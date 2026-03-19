use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, Gauge, GraphType, Paragraph},
};
use std::collections::VecDeque;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    sim::SimState,
    themes::ThemeColors,
    tui::{
        action::Action,
        aspect::AspectCorrection,
        config::Config,
        widgets::{
            heatmap::HeatmapWidget,
            sparkline_table::{SparklineRow, SparklineTable},
        },
    },
};

const MAX_ENERGY_RECENT: usize = 500;
const RC_SUBSAMPLE: usize = 10;

pub struct RunControlTab {
    sim_state: Option<SimState>,
    initial_energy: f64,
    initial_mass: f64,
    initial_c2: f64,
    /// Recent high-resolution energy ratio history
    energy_recent: VecDeque<(f64, f64)>,
    /// Downsampled full history (never dropped)
    energy_full: Vec<(f64, f64)>,
    rc_subsample_count: usize,
    log_stream: VecDeque<(Level, String)>,
    log_filter: LogFilter,
    paused: bool,
    command_tx: Option<UnboundedSender<Action>>,
    sim_start_time: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFilter {
    All,
    WarnPlus,
    ErrorOnly,
}

impl Default for RunControlTab {
    fn default() -> Self {
        Self {
            sim_state: None,
            initial_energy: 0.0,
            initial_mass: 0.0,
            initial_c2: 0.0,
            energy_recent: VecDeque::with_capacity(MAX_ENERGY_RECENT),
            energy_full: Vec::new(),
            rc_subsample_count: 0,
            log_stream: VecDeque::with_capacity(200),
            log_filter: LogFilter::All,
            paused: false,
            command_tx: None,
            sim_start_time: None,
        }
    }
}

impl RunControlTab {
    pub fn register_action_handler(&mut self, tx: UnboundedSender<Action>) {
        self.command_tx = Some(tx);
    }

    pub fn register_config_handler(&mut self, _config: Config) {}

    fn push_log(&mut self, level: Level, msg: impl Into<String>) {
        if self.log_stream.len() >= 200 {
            self.log_stream.pop_front();
        }
        self.log_stream.push_back((level, msg.into()));
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('p') | KeyCode::Char(' ') => Some(if self.paused {
                Action::SimResume
            } else {
                Action::SimPause
            }),
            KeyCode::Char('s') => Some(Action::SimStop),
            KeyCode::Char('r') => Some(Action::SimRestart),
            KeyCode::Char('1') => {
                self.log_filter = LogFilter::All;
                None
            }
            KeyCode::Char('2') => {
                self.log_filter = LogFilter::WarnPlus;
                None
            }
            KeyCode::Char('3') => {
                self.log_filter = LogFilter::ErrorOnly;
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::SimUpdate(state) => {
                // Ingest verbose log messages from sim thread
                for msg in &state.log_messages {
                    self.push_log(Level::Info, msg.clone());
                }

                // Track initial values
                if self.initial_energy == 0.0 && state.total_energy != 0.0 {
                    self.initial_energy = state.total_energy;
                    self.initial_mass = state.total_mass;
                    self.initial_c2 = state.casimir_c2;
                    self.push_log(
                        Level::Info,
                        format!(
                            "Sim started: E₀={:.3e}, M₀={:.4}",
                            state.total_energy, state.total_mass
                        ),
                    );
                }

                // Energy history
                if self.initial_energy != 0.0 {
                    let e_ratio = state.total_energy / self.initial_energy;
                    if self.energy_recent.len() >= MAX_ENERGY_RECENT {
                        self.energy_recent.pop_front();
                    }
                    self.energy_recent.push_back((state.t, e_ratio));

                    self.rc_subsample_count += 1;
                    if self.rc_subsample_count >= RC_SUBSAMPLE {
                        self.energy_full.push((state.t, e_ratio));
                        self.rc_subsample_count = 0;
                    }
                }

                if let Some(reason) = state.exit_reason {
                    self.sim_start_time = None;
                    self.push_log(Level::Info, format!("Exit: {reason}"));
                }

                self.sim_state = Some((**state).clone());
            }
            Action::SimPause => {
                self.paused = true;
                self.push_log(Level::Info, "Simulation paused".to_string());
            }
            Action::SimResume => {
                self.paused = false;
                self.push_log(Level::Info, "Simulation resumed".to_string());
            }
            Action::SimStop => {
                self.paused = false;
                self.sim_start_time = None;
                self.push_log(Level::Warn, "Simulation stopped by user".to_string());
            }
            Action::SimStart | Action::SimRestart => {
                self.energy_recent.clear();
                self.energy_full.clear();
                self.rc_subsample_count = 0;
                self.initial_energy = 0.0;
                self.initial_mass = 0.0;
                self.initial_c2 = 0.0;
                self.sim_state = None;
                self.sim_start_time = Some(std::time::Instant::now());
                self.paused = false;
                if matches!(action, Action::SimRestart) {
                    self.push_log(Level::Info, "Restarting simulation…".to_string());
                } else {
                    self.push_log(Level::Info, "Starting simulation…".to_string());
                }
            }
            Action::StatusMsg(msg) => {
                self.push_log(Level::Info, msg.clone());
            }
            _ => {}
        }
        None
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        colormap: Colormap,
        data_provider: &dyn DataProvider,
    ) {
        // Compact mode: controls + bottom only (no maps)
        if area.width < 76 {
            let [top_area, bottom_area] =
                Layout::vertical([Constraint::Length(4), Constraint::Min(6)]).areas(area);
            self.draw_controls(frame, top_area, theme);
            self.draw_bottom(frame, bottom_area, theme, data_provider);
            return;
        }

        let ideal_map_h = (area.width / 4).clamp(8, area.height.saturating_sub(16));

        let [top_area, maps_area, bottom_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(ideal_map_h),
            Constraint::Min(12),
        ])
        .areas(area);

        self.draw_controls(frame, top_area, theme);
        self.draw_maps(frame, maps_area, theme, colormap, data_provider);
        self.draw_bottom(frame, bottom_area, theme, data_provider);
    }

    fn draw_controls(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let [prog_area, energy_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).areas(area);

        match &self.sim_state {
            None => {
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(Span::styled(
                            "No simulation running.",
                            Style::default().fg(theme.dim),
                        )),
                        Line::from(vec![
                            Span::styled("  → Go to Setup ", Style::default().fg(theme.dim)),
                            Span::styled(
                                "[F1]",
                                Style::default()
                                    .fg(theme.accent)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(" and press ", Style::default().fg(theme.dim)),
                            Span::styled(
                                "[r]",
                                Style::default()
                                    .fg(theme.accent)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(" to start", Style::default().fg(theme.dim)),
                        ]),
                    ]),
                    area,
                );
            }
            Some(state) => {
                let progress = state.progress();
                let paused_tag = if self.paused { " [PAUSED]" } else { "" };
                let eta = if let Some(start) = self.sim_start_time {
                    if progress > 0.01 && progress < 1.0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let remaining = elapsed * (1.0 - progress) / progress;
                        format!("  ETA {}", format_eta(remaining))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                frame.render_widget(
                    Gauge::default()
                        .gauge_style(Style::default().fg(theme.ok))
                        .ratio(progress)
                        .label(format!(
                            "t = {:.3}/{:.1}  step {}  {:.1}%{paused_tag}{eta}",
                            state.t,
                            state.t_final,
                            state.step,
                            progress * 100.0
                        )),
                    prog_area,
                );

                let e_drift = state.energy_drift().abs();
                let cons_ratio = (1.0 - (e_drift / 1e-3).min(1.0)).clamp(0.0, 1.0);
                let cons_color = if e_drift < 1e-5 {
                    theme.ok
                } else if e_drift < 1e-3 {
                    theme.warn
                } else {
                    theme.error
                };

                frame.render_widget(
                    Gauge::default()
                        .gauge_style(Style::default().fg(cons_color))
                        .ratio(cons_ratio)
                        .label(format!(
                            "|ΔE/E| = {e_drift:.2e}  dt={:.3e}  ρ_max={:.2e}",
                            state.step_wall_ms / 1000.0,
                            state.max_density
                        )),
                    energy_area,
                );
            }
        }
    }

    fn draw_maps(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        colormap: Colormap,
        data_provider: &dyn DataProvider,
    ) {
        let [density_area, phase_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);

        match data_provider.current_state() {
            None => {
                for (a, t) in [(density_area, " ρ(x,y) "), (phase_area, " f(x,vx) ")] {
                    frame.render_widget(
                        Paragraph::new("—")
                            .block(Block::bordered().title(t))
                            .style(Style::default().fg(theme.dim)),
                        a,
                    );
                }
            }
            Some(state) => {
                let cell_ar = data_provider
                    .config()
                    .map(|c| c.appearance.cell_aspect_ratio)
                    .unwrap_or(0.5);
                let asp = AspectCorrection::new(cell_ar);

                // Spatial domain is symmetric: extent covers half-width
                let x_extent = state.spatial_extent * 2.0;
                let y_extent = x_extent;

                frame.render_widget(
                    HeatmapWidget::new(
                        &state.density_xy,
                        state.density_nx,
                        state.density_ny,
                        " ρ(x,y) density ",
                    )
                    .colormap(colormap)
                    .aspect(asp)
                    .x_range(x_extent)
                    .y_range(y_extent),
                    density_area,
                );

                // Phase-space: use 1:1 aspect (equal ranges → visually square)
                frame.render_widget(
                    HeatmapWidget::new(
                        &state.phase_slice,
                        state.phase_nx,
                        state.phase_nv,
                        " f(x,vx) phase-space ",
                    )
                    .colormap(colormap)
                    .aspect(asp),
                    phase_area,
                );
            }
        }
    }

    fn draw_bottom(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Min(40), Constraint::Length(42)]).areas(area);

        let [chart_area, log_area] =
            Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                .areas(left_area);

        // Energy chart — merge full history + recent for complete view from t=0
        let energy_data = {
            let recent_start = self
                .energy_recent
                .front()
                .map(|(t, _)| *t)
                .unwrap_or(f64::INFINITY);
            let mut data: Vec<(f64, f64)> = self
                .energy_full
                .iter()
                .copied()
                .filter(|(t, _)| *t < recent_start)
                .collect();
            data.extend(self.energy_recent.iter().copied());
            data
        };
        if energy_data.is_empty() {
            frame.render_widget(
                Paragraph::new("Energy history will appear once the sim starts.")
                    .block(Block::bordered().title(" E(t)/E₀ "))
                    .style(Style::default().fg(theme.dim)),
                chart_area,
            );
        } else {
            let t_min = energy_data.first().map(|(t, _)| *t).unwrap_or(0.0);
            let t_max = energy_data
                .last()
                .map(|(t, _)| *t)
                .unwrap_or(1.0)
                .max(t_min + 0.001);
            let (e_min, e_max) = energy_data
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, e)| {
                    (lo.min(*e), hi.max(*e))
                });
            let e_lo = (e_min - 0.001).min(0.99);
            let e_hi = (e_max + 0.001).max(1.01);

            let chart_width = chart_area.width.saturating_sub(2) as usize;
            let dense = densify(&energy_data, chart_width * 2);

            let chart = Chart::new(vec![
                Dataset::default()
                    .name("E/E₀")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(theme.chart[0]))
                    .data(&dense),
            ])
            .block(Block::bordered().title(" E(t)/E₀ "))
            .x_axis(
                Axis::default()
                    .title("t")
                    .style(Style::default().fg(theme.dim))
                    .bounds([t_min, t_max])
                    .labels(vec![
                        format!("{t_min:.1}"),
                        format!("{:.1}", (t_min + t_max) / 2.0),
                        format!("{t_max:.1}"),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .title("E/E₀")
                    .style(Style::default().fg(theme.dim))
                    .bounds([e_lo, e_hi])
                    .labels(vec![
                        format!("{e_lo:.4}"),
                        format!("{:.4}", (e_lo + e_hi) / 2.0),
                        format!("{e_hi:.4}"),
                    ]),
            );

            frame.render_widget(chart, chart_area);
        }

        // Log stream
        let visible: Vec<_> = self
            .log_stream
            .iter()
            .filter(|(lvl, _)| match self.log_filter {
                LogFilter::All => true,
                LogFilter::WarnPlus => matches!(lvl, Level::Warn | Level::Error),
                LogFilter::ErrorOnly => matches!(lvl, Level::Error),
            })
            .rev()
            .take(log_area.height as usize)
            .collect();

        let lines: Vec<Line> = visible
            .iter()
            .rev()
            .map(|(lvl, msg)| {
                let color = match lvl {
                    Level::Info => theme.dim,
                    Level::Warn => theme.warn,
                    Level::Error => theme.error,
                };
                Line::from(Span::styled(msg.clone(), Style::default().fg(color)))
            })
            .collect();

        let filter_hint = match self.log_filter {
            LogFilter::All => "[1]All [2]Warn+ [3]Error",
            LogFilter::WarnPlus => "[1]All [2]Warn+✓ [3]Error",
            LogFilter::ErrorOnly => "[1]All [2]Warn+ [3]Error✓",
        };

        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title(format!(" Log  {filter_hint} "))),
            log_area,
        );

        // Right panel — split into diagnostics + config summary
        let [diag_area, summary_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(right_area);

        // Diagnostics sidebar (top-right) — uses scrub-aware state
        match data_provider.current_state() {
            None => {
                frame.render_widget(
                    Paragraph::new("—").block(Block::bordered().title(" Diagnostics ")),
                    diag_area,
                );
            }
            Some(state) => {
                let init_e = self.initial_energy;
                let init_m = self.initial_mass;
                let init_c = self.initial_c2;

                let mut rows: Vec<SparklineRow> = vec![
                    SparklineRow::new(
                        "Energy E",
                        state.total_energy,
                        if init_e != 0.0 {
                            (state.total_energy - init_e) / init_e.abs()
                        } else {
                            0.0
                        },
                    )
                    .thresholds(1e-4, 1e-2),
                    SparklineRow::new(
                        "Mass M",
                        state.total_mass,
                        if init_m != 0.0 {
                            (state.total_mass - init_m) / init_m.abs()
                        } else {
                            0.0
                        },
                    )
                    .thresholds(1e-6, 1e-3),
                    SparklineRow::new(
                        "Casimir C₂",
                        state.casimir_c2,
                        if init_c != 0.0 {
                            (state.casimir_c2 - init_c) / init_c.abs()
                        } else {
                            0.0
                        },
                    )
                    .thresholds(1e-5, 1e-2),
                    SparklineRow::new(
                        "Virial 2T/|W|",
                        state.virial_ratio,
                        state.virial_ratio - 1.0,
                    )
                    .thresholds(0.1, 0.5),
                    SparklineRow::new("Max density", state.max_density, 0.0),
                ];

                // HT rank rows (§2.2 spec: Avg rank, Peak rank)
                if let Some(ref ranks) = state.rank_per_node {
                    let budget = 100u32; // fallback budget
                    let avg = if ranks.is_empty() {
                        0.0
                    } else {
                        ranks.iter().sum::<usize>() as f64 / ranks.len() as f64
                    };
                    let peak = ranks.iter().copied().max().unwrap_or(0);
                    rows.push(
                        SparklineRow::new("Avg rank", avg, avg / budget as f64)
                            .thresholds(0.5, 0.8),
                    );
                    rows.push(
                        SparklineRow::new("Peak rank", peak as f64, peak as f64 / budget as f64)
                            .thresholds(0.5, 0.8),
                    );
                }

                // Memory row
                if let Some(mem) = state.rank_memory_bytes {
                    rows.push(SparklineRow::new(
                        "Memory",
                        mem as f64 / (1024.0 * 1024.0),
                        0.0,
                    ));
                }

                rows.push(SparklineRow::new("Entropy S", state.entropy, 0.0));

                SparklineTable::new(&rows, " Diagnostics ").draw(frame, diag_area, theme);
            }
        }

        // Config summary panel (bottom-right)
        self.draw_config_summary(frame, summary_area, theme, data_provider);
    }

    fn draw_config_summary(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let block = Block::bordered()
            .title(" Config ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let label_style = Style::default().fg(theme.dim);
        let value_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);

        let lines = if let Some(cfg) = data_provider.config() {
            let n = cfg.domain.spatial_resolution;
            let nv = cfg.domain.velocity_resolution;
            let dt_str = if let Some(state) = &self.sim_state {
                format!("{:.3e}", state.dt)
            } else {
                cfg.time.dt_mode.clone()
            };
            let steps_s = self
                .sim_state
                .as_ref()
                .filter(|s| s.step_wall_ms > 0.0)
                .map(|s| format!("{:.1}", 1000.0 / s.step_wall_ms))
                .unwrap_or_else(|| "—".to_string());

            vec![
                Line::from(vec![
                    Span::styled(" Model:  ", label_style),
                    Span::styled(&cfg.model.model_type, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" Grid:   ", label_style),
                    Span::styled(format!("{n}^3 x {nv}^3"), value_style),
                ]),
                Line::from(vec![
                    Span::styled(" Solver: ", label_style),
                    Span::styled(&cfg.solver.poisson, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" Split:  ", label_style),
                    Span::styled(&cfg.solver.integrator, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" dt:     ", label_style),
                    Span::styled(dt_str, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" step/s: ", label_style),
                    Span::styled(steps_s, value_style),
                ]),
            ]
        } else {
            vec![Line::from(vec![Span::styled(
                " No config loaded",
                label_style,
            )])]
        };

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn format_eta(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.0}s")
    } else if secs < 3600.0 {
        format!("{}m{:02}s", secs as u64 / 60, secs as u64 % 60)
    } else {
        format!("{}h{:02}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
    }
}

/// Linearly interpolate sparse data so there are at least `target` points.
fn densify(data: &[(f64, f64)], target: usize) -> Vec<(f64, f64)> {
    if data.len() >= target || data.len() < 2 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(target);
    let n_segments = data.len() - 1;
    let points_per_seg = (target / n_segments).max(2);
    for i in 0..n_segments {
        let (x0, y0) = data[i];
        let (x1, y1) = data[i + 1];
        let steps = if i < n_segments - 1 {
            points_per_seg
        } else {
            target.saturating_sub(out.len()).max(2)
        };
        for j in 0..steps {
            let frac = j as f64 / steps as f64;
            out.push((x0 + frac * (x1 - x0), y0 + frac * (y1 - y0)));
        }
    }
    if let Some(&last) = data.last() {
        out.push(last);
    }
    out
}
