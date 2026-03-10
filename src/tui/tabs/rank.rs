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
    /// Last simulation step we recorded, to avoid duplicate pushes.
    last_step: u64,
}

impl Default for RankTab {
    fn default() -> Self {
        Self {
            rank_history: VecDeque::with_capacity(MAX_HISTORY),
            last_step: u64::MAX,
        }
    }
}

impl RankTab {
    pub fn handle_key_event(&mut self, _key: KeyEvent) -> Option<Action> {
        None
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
            self.last_step = state.step;
        }

        // Layout: top row (evolution chart + table) and bottom row (bar chart).
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);

        let [top_left, top_right] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(top);

        self.draw_rank_evolution(frame, top_left, theme);
        self.draw_per_node_table(frame, top_right, theme, state);
        self.draw_rank_bars(frame, bottom, theme, state);
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

        let datasets = vec![
            Dataset::default()
                .name("total rank")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(theme.chart[0]))
                .data(&dense),
        ];

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
        ])
        .height(1);

        let mut rows: Vec<Row> = Vec::with_capacity(14);
        for (i, label) in NODE_LABELS.iter().enumerate() {
            let rank_val = ranks.get(i).copied().unwrap_or(0);
            rows.push(Row::new(vec![
                Cell::from(format!("{i:>2}")),
                Cell::from(*label),
                Cell::from(format!("{rank_val}")),
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
            Constraint::Min(12),
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
