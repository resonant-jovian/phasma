use std::collections::VecDeque;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Cell, Chart, Dataset, Paragraph, Row, Table},
};

use crate::data::DataProvider;
use crate::themes::ThemeColors;
use crate::tui::action::Action;

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
        }
    }
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

        let state = state.unwrap(); // safe: is_ht checked above

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
        let block = Block::bordered()
            .title(" Rank Evolution ")
            .border_style(Style::default().fg(theme.border));

        if self.rank_history.len() < 2 {
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

        let chart_data: Vec<(f64, f64)> = self
            .rank_history
            .iter()
            .map(|&(t, r)| (t, r as f64))
            .collect();

        let (x_min, x_max, y_min, y_max) = data_bounds(&chart_data);

        let dense = densify(&chart_data, area.width.saturating_sub(2) as usize * 2);

        let trunc_data: Vec<(f64, f64)> = self
            .trunc_error_history
            .iter()
            .filter(|&&(_, e)| e > 0.0)
            .map(|&(t, e)| (t, e.log10() * 10.0 + y_max)) // scale log error into rank y-range
            .collect();
        let dense_trunc = densify(&trunc_data, area.width.saturating_sub(2) as usize * 2);

        let mut datasets = vec![
            Dataset::default()
                .name("total rank")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(theme.chart[0]))
                .data(&dense),
        ];

        if dense_trunc.len() >= 2 {
            datasets.push(
                Dataset::default()
                    .name("ε_trunc")
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(theme.chart[2]))
                    .data(&dense_trunc),
            );
        }

        // Poisson rank amplification (scaled into rank y-range)
        let poisson_amp_data: Vec<(f64, f64)> = self
            .poisson_amp_history
            .iter()
            .map(|&(t, a)| (t, a * y_max / 2.0)) // scale amplification into chart range
            .collect();
        let dense_poisson_amp = densify(&poisson_amp_data, area.width.saturating_sub(2) as usize * 2);

        if dense_poisson_amp.len() >= 2 {
            datasets.push(
                Dataset::default()
                    .name("Poisson amp.")
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(theme.chart[3 % theme.chart.len()]))
                    .data(&dense_poisson_amp),
            );
        }

        // Advection rank amplification
        let advection_amp_data: Vec<(f64, f64)> = self
            .advection_amp_history
            .iter()
            .map(|&(t, a)| (t, a * y_max / 2.0))
            .collect();
        let dense_advection_amp = densify(&advection_amp_data, area.width.saturating_sub(2) as usize * 2);

        if dense_advection_amp.len() >= 2 {
            datasets.push(
                Dataset::default()
                    .name("Advect. amp.")
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(theme.chart[4 % theme.chart.len()]))
                    .data(&dense_advection_amp),
            );
        }

        let chart = Chart::new(datasets)
            .block(block)
            .x_axis(
                Axis::default()
                    .bounds([x_min, x_max])
                    .labels(vec![format!("{x_min:.3}"), format!("{x_max:.3}")])
                    .style(Style::default().fg(theme.dim)),
            )
            .y_axis(
                Axis::default()
                    .bounds([y_min, y_max])
                    .labels(vec![format!("{:.0}", y_min), format!("{:.0}", y_max)])
                    .style(Style::default().fg(theme.dim)),
            );

        frame.render_widget(chart, area);
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
        let block = Block::bordered()
            .title(" Rank Bar Chart ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let ranks = match &state.rank_per_node {
            Some(r) => r,
            None => return,
        };

        let max_rank = ranks.iter().copied().max().unwrap_or(1).max(1);

        // Available width for the bar (minus label and value text)
        let label_width: u16 = 14;
        let value_width: u16 = 6;
        let bar_max_width = inner.width.saturating_sub(label_width + value_width + 3);

        // One row per node; if area is too short, truncate.
        let rows_available = inner.height as usize;

        for (i, label) in NODE_LABELS.iter().enumerate() {
            if i >= rows_available {
                break;
            }
            let rank_val = ranks.get(i).copied().unwrap_or(0);
            let fraction = rank_val as f64 / max_rank as f64;
            let bar_len =
                ((fraction * bar_max_width as f64) as u16).max(if rank_val > 0 { 1 } else { 0 });

            // Color by budget fraction: green < 50%, yellow 50-80%, red > 80%
            let bar_color = if fraction < 0.5 {
                theme.ok
            } else if fraction < 0.8 {
                theme.warn
            } else {
                theme.error
            };

            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }

            // Render label
            let label_span =
                Span::styled(format!("{:>12} ", label), Style::default().fg(theme.dim));
            frame.render_widget(
                Paragraph::new(Line::from(label_span)),
                Rect::new(inner.x, y, label_width, 1),
            );

            // Render bar
            if bar_len > 0 {
                let bar_str: String = "\u{2588}".repeat(bar_len as usize);
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        bar_str,
                        Style::default().fg(bar_color),
                    ))),
                    Rect::new(inner.x + label_width, y, bar_len, 1),
                );
            }

            // Render value
            let val_str = format!(" {rank_val}");
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    val_str,
                    Style::default().fg(theme.fg),
                ))),
                Rect::new(
                    inner.x + label_width + bar_len,
                    y,
                    value_width + (bar_max_width - bar_len),
                    1,
                ),
            );
        }
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
        let block = Block::bordered()
            .title(title.as_str())
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let ranks = state.rank_per_node.as_ref();
        let rank_val = ranks.and_then(|r| r.get(node)).copied().unwrap_or(0);
        let trunc_err = state
            .truncation_errors
            .as_ref()
            .and_then(|e| e.get(node))
            .copied();
        let budget = 100usize; // TODO: from config

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

        let node_type = if node < 6 { "leaf" } else { "transfer" };
        lines.push(Line::from(vec![
            Span::styled("  Type: ", Style::default().fg(theme.dim)),
            Span::styled(node_type, Style::default().fg(theme.fg)),
        ]));

        lines.push(Line::from(""));
        // Compute SV decay slope if singular values available
        if let Some(ref svs) = state.singular_values {
            if let Some(sv_vec) = svs.get(node) {
                if sv_vec.len() >= 2 {
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
                        "  Decay slope: (too few SVs)",
                        Style::default().fg(theme.dim),
                    )));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "  Decay slope: \u{2014}",
                    Style::default().fg(theme.dim),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "  Decay slope: \u{2014}",
                Style::default().fg(theme.dim),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Moment-carrying: \u{2014}/k",
            Style::default().fg(theme.dim),
        )));
        lines.push(Line::from(Span::styled(
            "  (LoMaC conservation)",
            Style::default().fg(theme.dim),
        )));

        frame.render_widget(Paragraph::new(lines), inner);
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

/// Linearly interpolate sparse data so braille line charts look solid.
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
