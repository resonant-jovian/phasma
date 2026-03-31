use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use ratatui_plt::prelude::{
    Axis as PltAxis, Bounds, Histogram as PltHistogram, LegendPosition, LinePlot, Scale, Series,
    StackedArea, StemPlot,
};
use ratatui_plt::widgets::bar_chart::{BarChart, BarDataset, Orientation};
use ratatui_plt::widgets::box_plot::{BoxData, BoxPlot};
use ratatui_plt::widgets::histogram::HistNorm;
use ratatui_plt::widgets::violin_plot::{ViolinData, ViolinPlot};
use std::collections::VecDeque;

use ratatui_plt::prelude::Theme;

use crate::{
    data::DataProvider,
    tui::action::Action,
    tui::plt_bridge::{PhasmaThemeExt, format_duration, format_size},
};

const RECENT_CAP: usize = 500;
const PERF_SUBSAMPLE: usize = 10;

/// Cached merged series (rebuilt when wall_times length changes).
struct CachedMerged {
    wall_data: Vec<(f64, f64)>,
    dt_data: Vec<(f64, f64)>,
    cumulative_data: Vec<(f64, f64)>,
    at_len: usize,
}

impl Default for CachedMerged {
    fn default() -> Self {
        Self {
            wall_data: Vec::new(),
            dt_data: Vec::new(),
            cumulative_data: Vec::new(),
            at_len: usize::MAX,
        }
    }
}

/// F8 Performance Dashboard — step timing, adaptive dt, cumulative cost, timing breakdown.
pub struct PerformanceTab {
    /// (step, wall_ms) per step — recent high-resolution window
    wall_times: VecDeque<(f64, f64)>,
    /// (sim_time, dt) — adaptive timestep evolution (recent)
    dt_history: VecDeque<(f64, f64)>,
    /// (sim_time, cumulative_wall_sec) — total wall time spent (recent)
    cumulative_wall: VecDeque<(f64, f64)>,
    /// Downsampled full history for each series (never dropped)
    wall_times_full: Vec<(f64, f64)>,
    dt_history_full: Vec<(f64, f64)>,
    cumulative_wall_full: Vec<(f64, f64)>,
    /// Subsample counter for full history
    subsample_count: usize,
    /// Previous sim time for computing dt
    prev_t: f64,
    /// Running total wall seconds
    total_wall_sec: f64,
    /// Rolling steps/second (last 100 steps)
    steps_per_sec_history: VecDeque<(f64, f64)>,
    /// Cached merged series for chart rendering.
    cached_merged: CachedMerged,
    /// Histogram normalization mode (count / density / probability).
    hist_norm: HistNorm,
    /// Distribution view mode: 0=histogram, 1=violin, 2=boxplot.
    dist_mode: u8,
}

impl Default for PerformanceTab {
    fn default() -> Self {
        Self {
            wall_times: VecDeque::with_capacity(RECENT_CAP),
            dt_history: VecDeque::with_capacity(RECENT_CAP),
            cumulative_wall: VecDeque::with_capacity(RECENT_CAP),
            wall_times_full: Vec::new(),
            dt_history_full: Vec::new(),
            cumulative_wall_full: Vec::new(),
            subsample_count: 0,
            prev_t: 0.0,
            total_wall_sec: 0.0,
            steps_per_sec_history: VecDeque::with_capacity(RECENT_CAP),
            cached_merged: CachedMerged::default(),
            hist_norm: HistNorm::Count,
            dist_mode: 0,
        }
    }
}

impl PerformanceTab {
    /// Ingest performance data from the latest SimState.
    pub fn ingest(&mut self, step: u64, t: f64, wall_ms: f64) {
        // Wall time per step (recent)
        if self.wall_times.len() >= RECENT_CAP {
            self.wall_times.pop_front();
        }
        self.wall_times.push_back((step as f64, wall_ms));

        // Adaptive dt
        let dt_val = if t > self.prev_t && self.prev_t > 0.0 {
            let dt = t - self.prev_t;
            if self.dt_history.len() >= RECENT_CAP {
                self.dt_history.pop_front();
            }
            self.dt_history.push_back((t, dt));
            Some((t, dt))
        } else {
            None
        };
        self.prev_t = t;

        // Cumulative wall time
        self.total_wall_sec += wall_ms / 1000.0;
        if self.cumulative_wall.len() >= RECENT_CAP {
            self.cumulative_wall.pop_front();
        }
        self.cumulative_wall.push_back((t, self.total_wall_sec));

        // Steps/second (rolling average over last 10 steps)
        let avg_ms = if self.wall_times.len() >= 2 {
            let n = self.wall_times.len().min(10);
            let sum: f64 = self.wall_times.iter().rev().take(n).map(|(_, ms)| ms).sum();
            sum / n as f64
        } else {
            wall_ms
        };
        let sps = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
        if self.steps_per_sec_history.len() >= RECENT_CAP {
            self.steps_per_sec_history.pop_front();
        }
        self.steps_per_sec_history.push_back((t, sps));

        // Downsample into full history
        self.subsample_count += 1;
        if self.subsample_count >= PERF_SUBSAMPLE {
            self.wall_times_full.push((step as f64, wall_ms));
            if let Some(dt_point) = dt_val {
                self.dt_history_full.push(dt_point);
            }
            self.cumulative_wall_full.push((t, self.total_wall_sec));
            self.subsample_count = 0;
        }
    }

    /// Reset for a new run.
    pub fn reset(&mut self) {
        self.wall_times.clear();
        self.dt_history.clear();
        self.cumulative_wall.clear();
        self.wall_times_full.clear();
        self.dt_history_full.clear();
        self.cumulative_wall_full.clear();
        self.steps_per_sec_history.clear();
        self.subsample_count = 0;
        self.prev_t = 0.0;
        self.total_wall_sec = 0.0;
        self.cached_merged = CachedMerged::default();
    }

    /// Merge full history + recent window for a complete chart from t=0.
    fn merge_series(full: &[(f64, f64)], recent: &VecDeque<(f64, f64)>) -> Vec<(f64, f64)> {
        let recent_start = recent.front().map(|(x, _)| *x).unwrap_or(f64::INFINITY);
        let mut data: Vec<(f64, f64)> = full
            .iter()
            .copied()
            .filter(|(x, _)| *x < recent_start)
            .collect();
        data.extend(recent.iter().copied());
        data
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('n') => {
                self.hist_norm = match self.hist_norm {
                    HistNorm::Count => HistNorm::Density,
                    HistNorm::Density => HistNorm::Probability,
                    HistNorm::Probability => HistNorm::Count,
                };
                None
            }
            KeyCode::Char('v') => {
                self.dist_mode = (self.dist_mode + 1) % 3;
                None
            }
            _ => None,
        }
    }

    /// Called on every SimUpdate to record performance data regardless of active tab.
    pub fn update(&mut self, action: &Action) {
        if let Action::SimUpdate(state) = action
            && self
                .wall_times
                .back()
                .is_none_or(|&(s, _)| s != state.step as f64)
        {
            self.ingest(state.step, state.t, state.step_wall_ms);
        }
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        // Rebuild merged series cache when data changes
        let current_len = self.wall_times.len();
        if current_len != self.cached_merged.at_len {
            self.cached_merged = CachedMerged {
                wall_data: Self::merge_series(&self.wall_times_full, &self.wall_times),
                dt_data: Self::merge_series(&self.dt_history_full, &self.dt_history),
                cumulative_data: Self::merge_series(
                    &self.cumulative_wall_full,
                    &self.cumulative_wall,
                ),
                at_len: current_len,
            };
        }

        // Compact mode: stats + wall time only
        if area.width < 76 {
            let [stats_area, chart_area] =
                Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .areas(area);
            self.draw_stats(frame, stats_area, theme, data_provider);
            self.draw_wall_time_chart(frame, chart_area, theme);
            return;
        }

        // Wide mode (160+): 4-row layout with extra charts
        if area.width >= 156 {
            let [top, mid, bottom, extra] = Layout::vertical([
                Constraint::Percentage(26),
                Constraint::Percentage(26),
                Constraint::Percentage(24),
                Constraint::Percentage(24),
            ])
            .areas(area);

            let [stats_area, timing_area, memory_area] = Layout::horizontal([
                Constraint::Percentage(28),
                Constraint::Percentage(36),
                Constraint::Percentage(36),
            ])
            .areas(top);

            let [wall_area, dt_area, cumul_area, hist_area] = Layout::horizontal([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .areas(mid);

            let [phase_area, adt_area, posv_area] = Layout::horizontal([
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ])
            .areas(bottom);

            let [op_timing_area, step_bkdn_area] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .areas(extra);

            self.draw_stats(frame, stats_area, theme, data_provider);
            Self::draw_timing_breakdown(frame, timing_area, theme, data_provider);
            Self::draw_memory_breakdown(frame, memory_area, theme, data_provider);
            self.draw_wall_time_chart(frame, wall_area, theme);
            self.draw_dt_chart(frame, dt_area, theme);
            self.draw_cumulative_chart(frame, cumul_area, theme);
            self.draw_step_time_histogram(frame, hist_area, theme);
            Self::draw_phase_timing_stacked(frame, phase_area, theme, data_provider);
            Self::draw_adaptive_dt_chart(frame, adt_area, theme, data_provider);
            Self::draw_positivity_violations(frame, posv_area, theme, data_provider);
            Self::draw_operation_timings(frame, op_timing_area, theme, data_provider);
            Self::draw_step_breakdown(frame, step_bkdn_area, theme, data_provider);
            return;
        }

        // Standard 4-row layout
        let [top, mid, bottom, extra] = Layout::vertical([
            Constraint::Percentage(26),
            Constraint::Percentage(26),
            Constraint::Percentage(24),
            Constraint::Percentage(24),
        ])
        .areas(area);

        let [stats_area, timing_area, memory_area, dt_area] = Layout::horizontal([
            Constraint::Percentage(28),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(28),
        ])
        .areas(top);

        let [wall_area, cumul_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(mid);

        let [phase_area, adt_area, posv_area] = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .areas(bottom);

        let [op_timing_area, step_bkdn_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(extra);

        self.draw_stats(frame, stats_area, theme, data_provider);
        Self::draw_timing_breakdown(frame, timing_area, theme, data_provider);
        Self::draw_memory_breakdown(frame, memory_area, theme, data_provider);
        self.draw_dt_chart(frame, dt_area, theme);
        self.draw_wall_time_chart(frame, wall_area, theme);
        self.draw_cumulative_chart(frame, cumul_area, theme);
        Self::draw_phase_timing_stacked(frame, phase_area, theme, data_provider);
        Self::draw_adaptive_dt_chart(frame, adt_area, theme, data_provider);
        Self::draw_positivity_violations(frame, posv_area, theme, data_provider);
        Self::draw_operation_timings(frame, op_timing_area, theme, data_provider);
        Self::draw_step_breakdown(frame, step_bkdn_area, theme, data_provider);
    }

    fn draw_memory_breakdown(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        // Split memory area: memory info + roofline indicators
        let [mem_area, roofline_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);

        let block = Block::bordered()
            .title(" Memory ")
            .border_style(Style::default().fg(theme.border_color()));
        let inner = block.inner(mem_area);
        frame.render_widget(block, mem_area);

        let state = data_provider.current_state();
        let iw = inner.width as usize;
        let lbl_w = if iw < 16 { 5 } else { 7 };
        let lines = if let Some(s) = state {
            let mut ls = Vec::new();
            ls.push(Line::from(vec![
                Span::styled(
                    format!(" {:<lbl_w$}", "Type"),
                    Style::default().fg(theme.dim()),
                ),
                Span::styled(
                    if s.repr_type.is_empty() {
                        "—".to_string()
                    } else {
                        s.repr_type.clone()
                    },
                    Style::default().fg(theme.foreground),
                ),
            ]));
            if let Some(mem) = s.rank_memory_bytes {
                ls.push(Line::from(vec![
                    Span::styled(
                        format!(" {:<lbl_w$}", "Repr"),
                        Style::default().fg(theme.dim()),
                    ),
                    Span::styled(
                        format_size(mem as f64),
                        Style::default().fg(theme.foreground),
                    ),
                ]));
            }
            if let Some(cr) = s.compression_ratio {
                ls.push(Line::from(vec![
                    Span::styled(
                        format!(" {:<lbl_w$}", "Comp"),
                        Style::default().fg(theme.dim()),
                    ),
                    Span::styled(
                        format!("{cr:.1}\u{00d7}"),
                        Style::default().fg(theme.chart_color(2)),
                    ),
                ]));
            }
            if s.svd_count > 0 {
                ls.push(Line::from(vec![
                    Span::styled(
                        format!(" {:<lbl_w$}", "SVDs"),
                        Style::default().fg(theme.dim()),
                    ),
                    Span::styled(
                        format!("{}/step", s.svd_count),
                        Style::default().fg(theme.foreground),
                    ),
                ]));
            }
            if s.htaca_evaluations > 0 {
                ls.push(Line::from(vec![
                    Span::styled(
                        format!(" {:<lbl_w$}", "HTACA"),
                        Style::default().fg(theme.dim()),
                    ),
                    Span::styled(
                        format!("{}", s.htaca_evaluations),
                        Style::default().fg(theme.foreground),
                    ),
                ]));
            }
            ls
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(" No data", Style::default().fg(theme.dim()))),
            ]
        };

        frame.render_widget(Paragraph::new(lines), inner);

        // Roofline indicators stub
        let rf_block = Block::bordered()
            .title(" Roofline ")
            .border_style(Style::default().fg(theme.border_color()));
        let rf_inner = rf_block.inner(roofline_area);
        frame.render_widget(rf_block, roofline_area);

        let rf_inner_w = rf_inner.width as usize;
        let rf_lines = if let Some(s) = state {
            // Estimate arithmetic intensity from step timing
            let total_ms = s.step_wall_ms;
            let nx = s.density_nx as f64;
            let nv = s.phase_nv as f64;
            let cells = nx * nx * nx * nv * nv * nv;
            let flops_est = cells * 20.0; // ~20 FLOP/cell estimate
            let gflops = if total_ms > 0.0 {
                flops_est / (total_ms * 1e6)
            } else {
                0.0
            };
            let mut ls = vec![Line::from(vec![
                Span::styled(" Est. ", Style::default().fg(theme.dim())),
                Span::styled(
                    format!("{gflops:.1} GF/s"),
                    Style::default().fg(theme.foreground),
                ),
            ])];
            let note = if rf_inner_w >= 30 {
                " (needs instrumentation)"
            } else {
                " (estimated)"
            };
            ls.push(Line::from(Span::styled(
                note,
                Style::default().fg(theme.dim()),
            )));
            ls
        } else {
            vec![Line::from(Span::styled(
                " No data",
                Style::default().fg(theme.dim()),
            ))]
        };
        frame.render_widget(Paragraph::new(rf_lines), rf_inner);
    }

    fn draw_timing_breakdown(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();
        let has_timings = state.map(|s| s.step_wall_ms > 0.0).unwrap_or(false);

        if !has_timings {
            let block = Block::bordered()
                .title(" Phase Timings ")
                .border_style(Style::default().fg(theme.border_color()));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Timing breakdown",
                        Style::default().fg(theme.dim()),
                    )),
                    Line::from(Span::styled(
                        "  not yet available",
                        Style::default().fg(theme.dim()),
                    )),
                ]),
                inner,
            );
            return;
        }

        let Some(s) = state else { return };
        let total = s.step_wall_ms;

        const PHASE_NAMES: [&str; 7] = ["Drift", "Poissn", "Kick", "Dens", "Diag", "I/O", "Other"];

        // Collect event-derived timing entries
        let mut event_categories: Vec<String> = Vec::new();
        let mut event_values: Vec<f64> = Vec::new();
        if let Some(us) = s.poisson_wall_us {
            let ms = us as f64 / 1000.0;
            event_categories.push("Poissn*".to_string());
            event_values.push(if total > 0.0 { ms / total * 100.0 } else { 0.0 });
        }
        if let Some(us) = s.advection_wall_us {
            let ms = us as f64 / 1000.0;
            event_categories.push("Advect*".to_string());
            event_values.push(if total > 0.0 { ms / total * 100.0 } else { 0.0 });
        }
        if let Some(us) = s.ht_slar_wall_us {
            let ms = us as f64 / 1000.0;
            event_categories.push("SLAR*".to_string());
            event_values.push(if total > 0.0 { ms / total * 100.0 } else { 0.0 });
        }

        if let Some(ref timings) = s.phase_timings {
            // Real phase timings → BarChart (augmented with event-derived timings)
            let plt_theme = theme.clone();
            let mut categories: Vec<String> = PHASE_NAMES
                .iter()
                .zip(timings.iter())
                .filter(|&(_, ms)| *ms > 0.0)
                .map(|(&name, _)| name.to_string())
                .collect();
            let mut values: Vec<f64> = timings
                .iter()
                .filter(|&&ms| ms > 0.0)
                .map(|&ms| if total > 0.0 { ms / total * 100.0 } else { 0.0 })
                .collect();

            // Append event-derived timing entries
            categories.extend(event_categories);
            values.extend(event_values);

            let chart = BarChart::new()
                .categories(categories)
                .dataset(BarDataset::new("% time", values, theme.chart_color(0)))
                .orientation(Orientation::Horizontal)
                .title(format!(" Phase Timings ({total:.1}ms) "))
                .theme(plt_theme);

            frame.render_widget(&chart, area);
        } else {
            // Estimated split (Strang) — text fallback, augmented with event timings
            let block = Block::bordered()
                .title(format!(" Phase Timings ({total:.1}ms) "))
                .border_style(Style::default().fg(theme.border_color()));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let mut lines = vec![
                Line::from(vec![
                    Span::styled(" Drift   ", Style::default().fg(theme.dim())),
                    Span::styled("~33%", Style::default().fg(theme.chart_color(0))),
                ]),
                Line::from(vec![
                    Span::styled(" Poissn  ", Style::default().fg(theme.dim())),
                    Span::styled("~34%", Style::default().fg(theme.chart_color(1))),
                ]),
                Line::from(vec![
                    Span::styled(" Kick    ", Style::default().fg(theme.dim())),
                    Span::styled("~33%", Style::default().fg(theme.chart_color(2))),
                ]),
            ];

            // Add event-derived timings when available
            if let Some(us) = s.poisson_wall_us {
                lines.push(Line::from(vec![
                    Span::styled(" Poissn  ", Style::default().fg(theme.dim())),
                    Span::styled(
                        format!("{:.0}\u{00b5}s", us),
                        Style::default().fg(theme.chart_color(3)),
                    ),
                ]));
            }
            if let Some(us) = s.advection_wall_us {
                lines.push(Line::from(vec![
                    Span::styled(" Advect  ", Style::default().fg(theme.dim())),
                    Span::styled(
                        format!("{:.0}\u{00b5}s", us),
                        Style::default().fg(theme.chart_color(4)),
                    ),
                ]));
            }
            if let Some(us) = s.ht_slar_wall_us {
                lines.push(Line::from(vec![
                    Span::styled(" SLAR    ", Style::default().fg(theme.dim())),
                    Span::styled(
                        format!("{:.0}\u{00b5}s", us),
                        Style::default().fg(theme.chart_color(5)),
                    ),
                ]));
            }

            if event_categories.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " (estimated)",
                    Style::default().fg(theme.dim()),
                )));
            }

            frame.render_widget(Paragraph::new(lines), inner);
        }
    }

    fn draw_stats(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();
        let step = state.map(|s| s.step).unwrap_or(0);
        let last_ms = state.map(|s| s.step_wall_ms).unwrap_or(0.0);
        let sim_t = state.map(|s| s.t).unwrap_or(0.0);
        let (sum, min_ms, max_ms) = self.wall_times.iter().fold(
            (0.0f64, f64::INFINITY, f64::NEG_INFINITY),
            |(s, mn, mx), &(_, ms)| (s + ms, mn.min(ms), mx.max(ms)),
        );
        let avg_ms = if self.wall_times.is_empty() {
            0.0
        } else {
            sum / self.wall_times.len() as f64
        };
        let steps_per_sec = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };

        let nx = state.map(|s| s.density_nx).unwrap_or(0);
        let nv = state.map(|s| s.phase_nv).unwrap_or(0);
        let total_cells = if nx > 0 && nv > 0 {
            (nx * nx * nx * nv * nv * nv) as f64
        } else {
            0.0
        };
        let grid_str = if nx > 0 && nv > 0 {
            format!("{}^3 x {}^3 = {:.1e}", nx, nv, total_cells)
        } else {
            "—".to_string()
        };

        let cells_per_sec = if avg_ms > 0.0 {
            total_cells * 1000.0 / avg_ms
        } else {
            0.0
        };

        // Current dt
        let current_dt = self.dt_history.back().map(|(_, dt)| *dt).unwrap_or(0.0);

        let inner_w = area.width.saturating_sub(2) as usize;
        let lw = if inner_w < 30 { 8 } else { 12 };
        let val = move |label: &str, value: String| -> Line {
            Line::from(vec![
                Span::styled(format!(" {label:<lw$}"), Style::default().fg(theme.dim())),
                Span::styled(value, Style::default().fg(theme.foreground)),
            ])
        };

        // Efficiency metric: throughput trend
        let throughput_trend = if self.steps_per_sec_history.len() >= 2 {
            let recent = self
                .steps_per_sec_history
                .back()
                .map(|(_, s)| *s)
                .unwrap_or(0.0);
            let old_idx = self.steps_per_sec_history.len().saturating_sub(20);
            let old = self
                .steps_per_sec_history
                .get(old_idx)
                .map(|(_, s)| *s)
                .unwrap_or(recent);
            if old > 0.0 {
                let pct = ((recent - old) / old * 100.0) as i64;
                if pct > 0 {
                    format!("+{pct}%")
                } else {
                    format!("{pct}%")
                }
            } else {
                "—".to_string()
            }
        } else {
            "—".to_string()
        };

        let mut lines = vec![
            Line::from(Span::styled(
                " Performance",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            val("Step", format!("{step}")),
            val("Sim time", format!("{sim_t:.4}")),
            val("Current dt", format!("{current_dt:.3e}")),
            val("Wall/step", format!("{last_ms:.1} ms")),
            val("Avg wall", format!("{avg_ms:.1} ms")),
            val("Min/max", format!("{min_ms:.1} / {max_ms:.1} ms")),
            val(
                "Steps/s",
                format!("{steps_per_sec:.1} ({throughput_trend})"),
            ),
            val("Total wall", format_duration(self.total_wall_sec)),
            Line::from(""),
            val("Grid", grid_str),
            val(
                "Cells/s",
                if cells_per_sec > 0.0 {
                    format!("{cells_per_sec:.2e}")
                } else {
                    "—".to_string()
                },
            ),
        ];

        // Event-derived component timings
        if let Some(s) = state {
            if let Some(us) = s.poisson_wall_us {
                lines.push(val("Poisson", format!("{us}\u{00b5}s")));
            }
            if let Some(us) = s.advection_wall_us {
                lines.push(val("Advect", format!("{us}\u{00b5}s")));
            }
            if let Some(us) = s.ht_slar_wall_us {
                lines.push(val("SLAR", format!("{us}\u{00b5}s")));
            }
        }

        let block = Block::bordered()
            .title(" Stats ")
            .border_style(Style::default().fg(theme.border_color()));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_dt_chart(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let data = &self.cached_merged.dt_data;

        if data.is_empty() {
            frame.render_widget(
                Paragraph::new("Adaptive timestep will appear once the sim starts.")
                    .block(
                        Block::bordered()
                            .title(" dt(t) — adaptive timestep ")
                            .border_style(Style::default().fg(theme.border_color())),
                    )
                    .style(Style::default().fg(theme.dim())),
                area,
            );
            return;
        }

        let plt_theme = theme.clone();
        let plot = LinePlot::new()
            .series(
                Series::new("dt")
                    .data(data.clone())
                    .color(theme.chart_color(4)),
            )
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new())
            .title(" dt(t) — adaptive timestep ")
            .theme(plt_theme);

        frame.render_widget(&plot, area);
    }

    fn draw_wall_time_chart(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let data = &self.cached_merged.wall_data;

        if data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" ms/step ")
                    .border_style(Style::default().fg(theme.border_color())),
                area,
            );
            return;
        }

        let plt_theme = theme.clone();
        let plot = LinePlot::new()
            .series(
                Series::new("ms/step")
                    .data(data.clone())
                    .color(theme.chart_color(3)),
            )
            .x_axis(PltAxis::new().label("step"))
            .y_axis(PltAxis::new())
            .title(" ms/step ")
            .theme(plt_theme);

        frame.render_widget(&plot, area);
    }

    fn draw_step_time_histogram(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let data = &self.cached_merged.wall_data;
        if data.len() < 10 {
            frame.render_widget(
                Block::bordered()
                    .title(" Step Time Distribution ")
                    .border_style(Style::default().fg(theme.border_color())),
                area,
            );
            return;
        }

        let times: Vec<f64> = data.iter().map(|(_, ms)| *ms).collect();
        let plt_theme = theme.clone();

        match self.dist_mode {
            1 => {
                // ViolinPlot
                let violin = ViolinPlot::new()
                    .dataset(ViolinData::new("wall time", times, theme.chart_color(3)))
                    .title(" Step Time Distribution [violin] ")
                    .show_box(true)
                    .theme(plt_theme);
                frame.render_widget(&violin, area);
            }
            2 => {
                // BoxPlot
                let boxplot = BoxPlot::new()
                    .box_data(BoxData::new("wall time", times, theme.chart_color(3)))
                    .show_outliers(true)
                    .show_means(true)
                    .title(" Step Time Distribution [boxplot] ")
                    .theme(plt_theme);
                frame.render_widget(&boxplot, area);
            }
            _ => {
                // Histogram (default)
                let norm_label = match &self.hist_norm {
                    HistNorm::Count => "count",
                    HistNorm::Density => "density",
                    HistNorm::Probability => "prob",
                };
                let hist = PltHistogram::new(times)
                    .bins(30)
                    .color(theme.chart_color(3))
                    .norm_mode(self.hist_norm.clone())
                    .title(format!(" Step Time Distribution [{norm_label}] "))
                    .x_axis(PltAxis::new().label("ms"))
                    .y_axis(PltAxis::new().label(norm_label))
                    .theme(plt_theme);
                frame.render_widget(&hist, area);
            }
        }
    }

    fn draw_cumulative_chart(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let data = &self.cached_merged.cumulative_data;

        if data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" Wall time vs sim time ")
                    .border_style(Style::default().fg(theme.border_color())),
                area,
            );
            return;
        }

        let plt_theme = theme.clone();
        let plot = LinePlot::new()
            .series(
                Series::new("wall")
                    .data(data.clone())
                    .color(theme.chart_color(1)),
            )
            .x_axis(PltAxis::new().label("sim t"))
            .y_axis(PltAxis::new().label("wall s"))
            .title(" Wall time vs sim time ")
            .theme(plt_theme);

        frame.render_widget(&plot, area);
    }

    fn draw_phase_timing_stacked(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let diag = data_provider.diagnostics();
        let drift_data = diag.phase_timing_drift.iter_chart_data();
        let poisson_data = diag.phase_timing_poisson.iter_chart_data();
        let kick_data = diag.phase_timing_kick.iter_chart_data();

        if drift_data.is_empty() && poisson_data.is_empty() && kick_data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" Phase Timing Breakdown ")
                    .border_style(Style::default().fg(theme.border_color())),
                area,
            );
            return;
        }

        let plt_theme = theme.clone();
        let stacked = StackedArea::new()
            .series(
                Series::new("Drift")
                    .data(drift_data)
                    .color(theme.chart_color(0)),
            )
            .series(
                Series::new("Poisson")
                    .data(poisson_data)
                    .color(theme.chart_color(1)),
            )
            .series(
                Series::new("Kick")
                    .data(kick_data)
                    .color(theme.chart_color(2)),
            )
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new().label("ms"))
            .title(" Phase Timing Breakdown ")
            .show_legend(true)
            .legend_position(LegendPosition::TopRight)
            .theme(plt_theme);

        frame.render_widget(&stacked, area);
    }

    fn draw_operation_timings(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let diag = data_provider.diagnostics();
        let poisson_data = diag.poisson_wall_us.iter_chart_data();
        let advection_data = diag.advection_wall_us.iter_chart_data();
        let slar_data = diag.ht_slar_wall_us.iter_chart_data();

        let has_data = poisson_data.len() >= 2 || advection_data.len() >= 2 || slar_data.len() >= 2;

        if !has_data {
            let block = Block::bordered()
                .title(" Operation Timings ")
                .border_style(Style::default().fg(theme.border_color()));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Waiting for timing events...",
                    Style::default().fg(theme.dim()),
                )),
            ];

            // Show current snapshot values if available
            if let Some(s) = data_provider.current_state() {
                lines.push(Line::from(""));
                if let Some(us) = s.poisson_wall_us {
                    lines.push(Line::from(vec![
                        Span::styled("  Poisson:   ", Style::default().fg(theme.dim())),
                        Span::styled(
                            format!("{us}\u{00b5}s"),
                            Style::default().fg(theme.foreground),
                        ),
                    ]));
                }
                if let Some(us) = s.advection_wall_us {
                    lines.push(Line::from(vec![
                        Span::styled("  Advection: ", Style::default().fg(theme.dim())),
                        Span::styled(
                            format!("{us}\u{00b5}s"),
                            Style::default().fg(theme.foreground),
                        ),
                    ]));
                }
                if let Some(us) = s.op_compute_density_us {
                    lines.push(Line::from(vec![
                        Span::styled("  Density:   ", Style::default().fg(theme.dim())),
                        Span::styled(
                            format!("{us}\u{00b5}s"),
                            Style::default().fg(theme.foreground),
                        ),
                    ]));
                }
                if let Some(us) = s.op_compute_accel_us {
                    lines.push(Line::from(vec![
                        Span::styled("  Accel:     ", Style::default().fg(theme.dim())),
                        Span::styled(
                            format!("{us}\u{00b5}s"),
                            Style::default().fg(theme.foreground),
                        ),
                    ]));
                }
            }

            frame.render_widget(Paragraph::new(lines), inner);
            return;
        }

        let plt_theme = theme.clone();
        let mut plot = LinePlot::new()
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new().label("\u{00b5}s"))
            .title(" Operation Timings ")
            .show_legend(true)
            .legend_position(LegendPosition::TopRight)
            .theme(plt_theme);

        if poisson_data.len() >= 2 {
            plot = plot.series(
                Series::new("Poisson")
                    .data(poisson_data)
                    .color(theme.chart_color(0)),
            );
        }
        if advection_data.len() >= 2 {
            plot = plot.series(
                Series::new("Advection")
                    .data(advection_data)
                    .color(theme.chart_color(1)),
            );
        }
        if slar_data.len() >= 2 {
            plot = plot.series(
                Series::new("SLAR")
                    .data(slar_data)
                    .color(theme.chart_color(2)),
            );
        }

        frame.render_widget(&plot, area);
    }

    fn draw_step_breakdown(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();
        let has_data = state.map(|s| s.step_timings.is_some()).unwrap_or(false);

        if !has_data {
            let block = Block::bordered()
                .title(" Step Breakdown ")
                .border_style(Style::default().fg(theme.border_color()));
            frame.render_widget(block, area);
            return;
        }

        let Some(s) = state else { return };
        let Some(ref timings) = s.step_timings else {
            return;
        };
        let total: f64 = timings.iter().sum();

        const LABELS: [&str; 7] = [
            "Drift", "Poisson", "Kick", "Density", "Diag", "I/O", "Other",
        ];

        let categories: Vec<String> = LABELS
            .iter()
            .zip(timings.iter())
            .filter(|&(_, &ms)| ms > 0.0)
            .map(|(&name, _)| name.to_string())
            .collect();
        let values: Vec<f64> = timings
            .iter()
            .filter(|&&ms| ms > 0.0)
            .map(|&ms| if total > 0.0 { ms / total * 100.0 } else { 0.0 })
            .collect();

        if categories.is_empty() {
            let block = Block::bordered()
                .title(" Step Breakdown ")
                .border_style(Style::default().fg(theme.border_color()));
            frame.render_widget(block, area);
            return;
        }

        let plt_theme = theme.clone();
        let chart = BarChart::new()
            .categories(categories)
            .dataset(BarDataset::new("% time", values, theme.chart_color(0)))
            .orientation(Orientation::Horizontal)
            .title(format!(" Step Breakdown ({total:.1}ms) "))
            .theme(plt_theme);

        frame.render_widget(&chart, area);
    }

    fn draw_adaptive_dt_chart(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let data = data_provider.diagnostics().adaptive_dt.iter_chart_data();

        if data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" Adaptive \u{0394}t ")
                    .border_style(Style::default().fg(theme.border_color())),
                area,
            );
            return;
        }

        let plt_theme = theme.clone();
        let plot = LinePlot::new()
            .series(
                Series::new("\u{0394}t")
                    .data(data)
                    .color(theme.chart_color(4)),
            )
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new().label("\u{0394}t").scale(Scale::Log(10.0)))
            .title(" Adaptive \u{0394}t ")
            .theme(plt_theme);

        frame.render_widget(&plot, area);
    }

    fn draw_positivity_violations(
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        data_provider: &dyn DataProvider,
    ) {
        let data = data_provider
            .diagnostics()
            .positivity_violations
            .iter_chart_data();

        if data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" Positivity Violations ")
                    .border_style(Style::default().fg(theme.border_color())),
                area,
            );
            return;
        }

        let plt_theme = theme.clone();
        let stem = StemPlot::new(data)
            .color(theme.chart_color(5))
            .title(" Positivity Violations ")
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new().label("count"))
            .theme(plt_theme);

        frame.render_widget(&stem, area);
    }
}
