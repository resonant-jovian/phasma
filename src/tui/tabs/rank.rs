use std::collections::VecDeque;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table},
};
use ratatui_plt::prelude::{Axis as PltAxis, LinePlot, Scale, Series, StemPlot, TwinAxes};
use ratatui_plt::widgets::bar_chart::{BarChart, BarDataset, Orientation};

use crate::data::DataProvider;
use crate::themes::ThemeColors;
use crate::tui::action::Action;
use crate::tui::plt_bridge::phasma_theme_to_plt;

const NODE_LABELS: [&str; 11] = [
    "x\u{2081}",
    "x\u{2082}",
    "x\u{2083}",
    "v\u{2081}",
    "v\u{2082}",
    "v\u{2083}",
    "{x\u{2082},x\u{2083}}",
    "{v\u{2082},v\u{2083}}",
    "{x\u{2081}..x\u{2083}}",
    "{v\u{2081}..v\u{2083}}",
    "root",
];

const MAX_HISTORY: usize = 500;

/// Cached chart data derived from VecDeque histories (rebuilt when length changes).
struct CachedRankData {
    chart_data: Vec<(f64, f64)>,
    trunc_data: Vec<(f64, f64)>,
    poisson_amp: Vec<(f64, f64)>,
    advection_amp: Vec<(f64, f64)>,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    at_len: usize,
}

impl Default for CachedRankData {
    fn default() -> Self {
        Self {
            chart_data: Vec::new(),
            trunc_data: Vec::new(),
            poisson_amp: Vec::new(),
            advection_amp: Vec::new(),
            x_min: 0.0,
            x_max: 1.0,
            y_min: 0.0,
            y_max: 1.0,
            at_len: usize::MAX,
        }
    }
}

/// F6 Rank Monitor — visualises HT tensor rank evolution and per-node ranks.
pub struct RankTab {
    /// Time-series of (sim_time, total_rank).
    rank_history: VecDeque<(f64, usize)>,
    /// Time-series of (sim_time, max truncation error) for secondary y-axis.
    trunc_error_history: VecDeque<(f64, f64)>,
    /// Time-series of (sim_time, poisson_rank_amplification).
    poisson_amp_history: VecDeque<(f64, f64)>,
    /// Time-series of (sim_time, advection_rank_amplification).
    advection_amp_history: VecDeque<(f64, f64)>,
    /// Last simulation step we recorded, to avoid duplicate pushes.
    last_step: u64,
    /// Selected node index for SV spectrum (0–10, cycles with j/k or n/N).
    selected_node: usize,
    /// Cached chart data (rebuilt when history length changes).
    cached_rank: CachedRankData,
}

impl Default for RankTab {
    fn default() -> Self {
        Self {
            rank_history: VecDeque::with_capacity(MAX_HISTORY),
            trunc_error_history: VecDeque::with_capacity(MAX_HISTORY),
            poisson_amp_history: VecDeque::with_capacity(MAX_HISTORY),
            advection_amp_history: VecDeque::with_capacity(MAX_HISTORY),
            last_step: u64::MAX,
            selected_node: 0,
            cached_rank: CachedRankData::default(),
        }
    }
}

/// Compute data bounds with 5% y-padding.
fn data_bounds(data: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for &(x, y) in data {
        if x < x_min {
            x_min = x;
        }
        if x > x_max {
            x_max = x;
        }
        if y < y_min {
            y_min = y;
        }
        if y > y_max {
            y_max = y;
        }
    }
    if x_min >= x_max {
        x_max = x_min + 1.0;
    }
    if y_min >= y_max {
        y_max = y_min + 1.0;
    }
    let ypad = (y_max - y_min) * 0.05;
    (x_min, x_max, y_min - ypad, y_max + ypad)
}

impl RankTab {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        use crossterm::event::KeyCode;
        match key.code {
            // n/N to cycle selected node for SV spectrum
            KeyCode::Char('n') => {
                self.selected_node = (self.selected_node + 1) % NODE_LABELS.len();
                None
            }
            KeyCode::Char('N') => {
                self.selected_node =
                    (self.selected_node + NODE_LABELS.len() - 1) % NODE_LABELS.len();
                None
            }
            _ => None,
        }
    }

    pub fn handle_scroll(&mut self, _delta: i32) {}

    pub fn update(&mut self, _action: &Action) -> Option<Action> {
        None
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();

        // Check whether HT rank data is available.
        let is_ht = state
            .map(|s| s.repr_type == "ht" && s.rank_per_node.is_some())
            .unwrap_or(false);

        if !is_ht {
            self.draw_no_rank_message(frame, area, theme, state);
            return;
        }

        let Some(state) = state else { return };

        // Push new history entry if the step changed.
        if state.step != self.last_step
            && let Some(total) = state.rank_total
        {
            if self.rank_history.len() >= MAX_HISTORY {
                self.rank_history.pop_front();
            }
            self.rank_history.push_back((state.t, total));

            // Track truncation error history
            if let Some(ref errs) = state.truncation_errors {
                let max_err = errs.iter().copied().fold(0.0f64, f64::max);
                if max_err > 0.0 {
                    if self.trunc_error_history.len() >= MAX_HISTORY {
                        self.trunc_error_history.pop_front();
                    }
                    self.trunc_error_history.push_back((state.t, max_err));
                }
            }

            // Track rank amplification histories
            if let Some(amp) = state.poisson_rank_amplification {
                if self.poisson_amp_history.len() >= MAX_HISTORY {
                    self.poisson_amp_history.pop_front();
                }
                self.poisson_amp_history.push_back((state.t, amp));
            }
            if let Some(amp) = state.advection_rank_amplification {
                if self.advection_amp_history.len() >= MAX_HISTORY {
                    self.advection_amp_history.pop_front();
                }
                self.advection_amp_history.push_back((state.t, amp));
            }

            self.last_step = state.step;
        }

        // Rebuild cached chart data when history length changes
        let current_len = self.rank_history.len();
        if current_len != self.cached_rank.at_len {
            let chart_data: Vec<(f64, f64)> = self
                .rank_history
                .iter()
                .map(|&(t, r)| (t, r as f64))
                .collect();
            let (x_min, x_max, y_min, y_max) = if chart_data.len() >= 2 {
                data_bounds(&chart_data)
            } else {
                (0.0, 1.0, 0.0, 1.0)
            };
            // Store raw values — TwinAxes handles dual scaling
            let trunc_data: Vec<(f64, f64)> = self
                .trunc_error_history
                .iter()
                .filter(|&&(_, e)| e > 0.0)
                .copied()
                .collect();
            let poisson_amp: Vec<(f64, f64)> = self.poisson_amp_history.iter().copied().collect();
            let advection_amp: Vec<(f64, f64)> =
                self.advection_amp_history.iter().copied().collect();
            self.cached_rank = CachedRankData {
                chart_data,
                trunc_data,
                poisson_amp,
                advection_amp,
                x_min,
                x_max,
                y_min,
                y_max,
                at_len: current_len,
            };
        }

        // Compact mode: show only rank evolution chart
        if area.width < 76 {
            self.draw_rank_evolution(frame, area, theme);
            return;
        }

        // Layout: top row (evolution chart + table) and bottom row (bar chart + SV spectrum).
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);

        let [top_left, top_right] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(top);

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                .areas(bottom);

        self.draw_rank_evolution(frame, top_left, theme);
        self.draw_per_node_table(frame, top_right, theme, state);
        self.draw_rank_bars(frame, bottom_left, theme, state);
        self.draw_sv_spectrum(frame, bottom_right, theme, state);
    }

    // ── Panels ──────────────────────────────────────────────────────────

    fn draw_no_rank_message(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        state: Option<&crate::sim::SimState>,
    ) {
        let repr_name = state.map(|s| s.repr_type.as_str()).unwrap_or("unknown");
        let repr_detail = if repr_name == "uniform" {
            "uniform (full grid)"
        } else {
            repr_name
        };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Rank Monitor requires representation = \"ht\"",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Current representation: ", Style::default().fg(theme.dim)),
                Span::styled(
                    repr_detail.to_string(),
                    Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Switch to the Hierarchical Tucker representation in your config:",
                Style::default().fg(theme.dim),
            )),
            Line::from(Span::styled(
                "    [solver]",
                Style::default().fg(theme.warn),
            )),
            Line::from(Span::styled(
                "    representation = \"ht\"",
                Style::default().fg(theme.warn),
            )),
        ];

        let block = Block::bordered()
            .title(" Rank Monitor ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(text), inner);
    }

    fn draw_rank_evolution(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        if self.cached_rank.chart_data.len() < 2 {
            let block = Block::bordered()
                .title(" Rank Evolution ")
                .border_style(Style::default().fg(theme.border));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  Collecting data...",
                    Style::default().fg(theme.dim),
                ))),
                inner,
            );
            return;
        }

        let plt_theme = phasma_theme_to_plt(theme);

        // Use TwinAxes: primary (left) = total rank, secondary (right) = truncation error + amplifications
        let has_secondary = self.cached_rank.trunc_data.len() >= 2
            || self.cached_rank.poisson_amp.len() >= 2
            || self.cached_rank.advection_amp.len() >= 2;

        if has_secondary {
            let mut twin = TwinAxes::new()
                .primary(
                    Series::new("total rank")
                        .data(self.cached_rank.chart_data.clone())
                        .color(theme.chart[0]),
                )
                .x_axis(PltAxis::new().label("t"))
                .primary_y_axis(PltAxis::new().label("rank"))
                .secondary_y_axis(PltAxis::new().label("ε / amp").scale(Scale::Log(10.0)))
                .title(" Rank Evolution ")
                .theme(plt_theme);

            if self.cached_rank.trunc_data.len() >= 2 {
                twin = twin.secondary(
                    Series::new("ε_trunc")
                        .data(self.cached_rank.trunc_data.clone())
                        .color(theme.chart[2]),
                );
            }
            if self.cached_rank.poisson_amp.len() >= 2 {
                twin = twin.secondary(
                    Series::new("Poisson amp.")
                        .data(self.cached_rank.poisson_amp.clone())
                        .color(theme.chart[3 % theme.chart.len()]),
                );
            }
            if self.cached_rank.advection_amp.len() >= 2 {
                twin = twin.secondary(
                    Series::new("Advect. amp.")
                        .data(self.cached_rank.advection_amp.clone())
                        .color(theme.chart[4 % theme.chart.len()]),
                );
            }

            frame.render_widget(&twin, area);
        } else {
            // Fallback to simple LinePlot when no secondary data
            let plot = LinePlot::new()
                .series(
                    Series::new("total rank")
                        .data(self.cached_rank.chart_data.clone())
                        .color(theme.chart[0]),
                )
                .x_axis(PltAxis::new().label("t"))
                .y_axis(PltAxis::new().label("rank"))
                .title(" Rank Evolution ")
                .theme(plt_theme);

            frame.render_widget(&plot, area);
        }
    }

    fn draw_per_node_table(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        state: &crate::sim::SimState,
    ) {
        let block = Block::bordered()
            .title(" Per-Node Ranks ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let ranks = match &state.rank_per_node {
            Some(r) => r,
            None => return,
        };

        let header = Row::new(vec![
            Cell::from("Node").style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Dims").style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Rank").style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("ε_trunc").style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
        .height(1);

        let trunc_errors = state.truncation_errors.as_deref();
        let mut rows: Vec<Row> = Vec::with_capacity(14);
        for (i, label) in NODE_LABELS.iter().enumerate() {
            let rank_val = ranks.get(i).copied().unwrap_or(0);
            let err_str = trunc_errors
                .and_then(|e| e.get(i))
                .map(|&e| format!("{e:.1e}"))
                .unwrap_or_else(|| "—".to_string());
            rows.push(Row::new(vec![
                Cell::from(format!("{i:>2}")),
                Cell::from(*label),
                Cell::from(format!("{rank_val}")),
                Cell::from(err_str),
            ]));
        }

        // Separator + summary rows
        rows.push(Row::new(vec![
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ]));

        if let Some(total) = state.rank_total {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from("Total")
                    .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
                Cell::from(format!("{total}")).style(Style::default().fg(theme.ok)),
            ]));
        }

        if let Some(mem) = state.rank_memory_bytes {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from("Memory")
                    .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
                Cell::from(format_bytes(mem)).style(Style::default().fg(theme.chart[1])),
            ]));
        }

        if let Some(cr) = state.compression_ratio {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from("Compress.")
                    .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
                Cell::from(format!("{cr:.1}\u{00d7}")).style(Style::default().fg(theme.chart[2])),
            ]));
        }

        let widths = [
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(6),
            Constraint::Length(10),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(Style::default().bg(theme.highlight));

        frame.render_widget(table, inner);
    }

    fn draw_rank_bars(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        state: &crate::sim::SimState,
    ) {
        let ranks = match &state.rank_per_node {
            Some(r) => r,
            None => {
                let block = Block::bordered()
                    .title(" Rank Bar Chart ")
                    .border_style(Style::default().fg(theme.border));
                frame.render_widget(block, area);
                return;
            }
        };

        let plt_theme = phasma_theme_to_plt(theme);
        let categories: Vec<String> = NODE_LABELS.iter().map(|s| s.to_string()).collect();
        let values: Vec<f64> = (0..NODE_LABELS.len())
            .map(|i| ranks.get(i).copied().unwrap_or(0) as f64)
            .collect();

        let chart = BarChart::new()
            .categories(categories)
            .dataset(BarDataset::new("rank", values, theme.chart[0]))
            .orientation(Orientation::Horizontal)
            .title(" Rank Bar Chart ")
            .theme(plt_theme);

        frame.render_widget(&chart, area);
    }
    fn draw_sv_spectrum(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        state: &crate::sim::SimState,
    ) {
        let node = self.selected_node;
        let node_label = NODE_LABELS.get(node).unwrap_or(&"?");
        let title = format!(" SV Spectrum — node {node} ({node_label}) [n/N] ");

        let ranks = state.rank_per_node.as_ref();
        let rank_val = ranks.and_then(|r| r.get(node)).copied().unwrap_or(0);
        let trunc_err = state
            .truncation_errors
            .as_ref()
            .and_then(|e| e.get(node))
            .copied();
        let budget = 100usize;

        // Try to render StemPlot of singular values
        let has_sv_plot = if let Some(ref svs) = state.singular_values
            && let Some(sv_vec) = svs.get(node)
            && sv_vec.len() >= 2
        {
            let [plot_area, text_area] =
                Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .areas(area);

            let sv_data: Vec<(f64, f64)> = sv_vec
                .iter()
                .enumerate()
                .filter(|&(_, v)| *v > 0.0)
                .map(|(i, &v)| (i as f64, v))
                .collect();

            let plt_theme = phasma_theme_to_plt(theme);
            let stem = StemPlot::new(sv_data)
                .color(theme.chart[0])
                .title(title.clone())
                .x_axis(PltAxis::new().label("index"))
                .y_axis(PltAxis::new().scale(Scale::Log(10.0)))
                .theme(plt_theme);

            frame.render_widget(&stem, plot_area);
            Some(text_area)
        } else {
            None
        };

        // Text summary area (below stem plot, or full area if no SVs)
        let text_area = has_sv_plot.unwrap_or_else(|| {
            let block = Block::bordered()
                .title(title.as_str())
                .border_style(Style::default().fg(theme.border));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        });

        let node_type = if node < 6 { "leaf" } else { "transfer" };
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("  Rank: ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("{rank_val}/{budget}"),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({:.0}%)", rank_val as f64 / budget as f64 * 100.0),
                Style::default().fg(if rank_val * 100 > budget * 80 {
                    theme.warn
                } else {
                    theme.fg
                }),
            ),
        ]));
        if let Some(err) = trunc_err {
            lines.push(Line::from(vec![
                Span::styled("  \u{03b5}_trunc: ", Style::default().fg(theme.dim)),
                Span::styled(format!("{err:.2e}"), Style::default().fg(theme.fg)),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled("  Type: ", Style::default().fg(theme.dim)),
            Span::styled(node_type, Style::default().fg(theme.fg)),
        ]));

        // Decay slope
        if let Some(ref svs) = state.singular_values
            && let Some(sv_vec) = svs.get(node)
            && sv_vec.len() >= 2
        {
            let first = sv_vec[0].max(1e-300).ln();
            let last = sv_vec[sv_vec.len() - 1].max(1e-300).ln();
            let slope = (last - first) / (sv_vec.len() as f64 - 1.0);
            lines.push(Line::from(vec![
                Span::styled("  Decay slope: ", Style::default().fg(theme.dim)),
                Span::styled(
                    format!("{slope:.2}"),
                    Style::default().fg(if slope < -0.5 { theme.ok } else { theme.warn }),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "  Decay slope: \u{2014}",
                Style::default().fg(theme.dim),
            )));
        }

        frame.render_widget(Paragraph::new(lines), text_area);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
