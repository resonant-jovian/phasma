use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};
use std::collections::VecDeque;

use crate::{data::DataProvider, themes::ThemeColors, tui::action::Action};

const RECENT_CAP: usize = 500;
const PERF_SUBSAMPLE: usize = 10;

/// F8 Performance Dashboard — step timing, adaptive dt, cumulative cost.
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
        self.subsample_count = 0;
        self.prev_t = 0.0;
        self.total_wall_sec = 0.0;
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
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        // 2×2 layout
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        let [stats_area, dt_area] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).areas(top);

        let [wall_area, cumul_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom);

        self.draw_stats(frame, stats_area, theme, data_provider);
        self.draw_dt_chart(frame, dt_area, theme);
        self.draw_wall_time_chart(frame, wall_area, theme);
        self.draw_cumulative_chart(frame, cumul_area, theme);
    }

    fn draw_stats(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();
        let step = state.map(|s| s.step).unwrap_or(0);
        let last_ms = state.map(|s| s.step_wall_ms).unwrap_or(0.0);
        let sim_t = state.map(|s| s.t).unwrap_or(0.0);
        let avg_ms = if self.wall_times.is_empty() {
            0.0
        } else {
            self.wall_times.iter().map(|(_, ms)| ms).sum::<f64>() / self.wall_times.len() as f64
        };
        let max_ms = self
            .wall_times
            .iter()
            .map(|(_, ms)| *ms)
            .fold(0.0f64, f64::max);
        let min_ms = self
            .wall_times
            .iter()
            .map(|(_, ms)| *ms)
            .fold(f64::INFINITY, f64::min);
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

        let val = |label: &str, value: String| -> Line {
            Line::from(vec![
                Span::styled(format!("  {label:<14}"), Style::default().fg(theme.dim)),
                Span::styled(value, Style::default().fg(theme.fg)),
            ])
        };

        let lines = vec![
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
            val("Steps/s", format!("{steps_per_sec:.1}")),
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

        let block = Block::bordered()
            .title(" Stats ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_dt_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let data = Self::merge_series(&self.dt_history_full, &self.dt_history);

        if data.is_empty() {
            frame.render_widget(
                Paragraph::new("Adaptive timestep will appear once the sim starts.")
                    .block(
                        Block::bordered()
                            .title(" dt(t) — adaptive timestep ")
                            .border_style(Style::default().fg(theme.border)),
                    )
                    .style(Style::default().fg(theme.dim)),
                area,
            );
            return;
        }

        let (x_min, x_max, y_min, y_max) = data_bounds(&data);
        let dense = densify(&data, area.width.saturating_sub(2) as usize * 2);

        let ds = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.chart[4]))
            .data(&dense);

        let chart = Chart::new(vec![ds])
            .block(
                Block::bordered()
                    .title(" dt(t) — adaptive timestep ")
                    .border_style(Style::default().fg(theme.border)),
            )
            .x_axis(
                Axis::default()
                    .title("t")
                    .bounds([x_min, x_max])
                    .labels(vec![format!("{x_min:.2}"), format!("{x_max:.2}")])
                    .style(Style::default().fg(theme.dim)),
            )
            .y_axis(
                Axis::default()
                    .bounds([y_min, y_max])
                    .labels(vec![format!("{y_min:.2e}"), format!("{y_max:.2e}")])
                    .style(Style::default().fg(theme.dim)),
            );

        frame.render_widget(chart, area);
    }

    fn draw_wall_time_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let data = Self::merge_series(&self.wall_times_full, &self.wall_times);

        if data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" ms/step ")
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let (x_min, x_max, y_min, y_max) = data_bounds(&data);
        let dense = densify(&data, area.width.saturating_sub(2) as usize * 2);

        let ds = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.chart[3]))
            .data(&dense);

        let chart = Chart::new(vec![ds])
            .block(
                Block::bordered()
                    .title(" ms/step ")
                    .border_style(Style::default().fg(theme.border)),
            )
            .x_axis(
                Axis::default()
                    .title("step")
                    .bounds([x_min, x_max])
                    .labels(vec![format!("{x_min:.0}"), format!("{x_max:.0}")])
                    .style(Style::default().fg(theme.dim)),
            )
            .y_axis(
                Axis::default()
                    .bounds([y_min, y_max])
                    .labels(vec![format!("{y_min:.1}"), format!("{y_max:.1}")])
                    .style(Style::default().fg(theme.dim)),
            );

        frame.render_widget(chart, area);
    }

    fn draw_cumulative_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let data = Self::merge_series(&self.cumulative_wall_full, &self.cumulative_wall);

        if data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(" Wall time vs sim time ")
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let (x_min, x_max, y_min, y_max) = data_bounds(&data);
        let dense = densify(&data, area.width.saturating_sub(2) as usize * 2);

        let ds = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.chart[1]))
            .data(&dense);

        let chart = Chart::new(vec![ds])
            .block(
                Block::bordered()
                    .title(" Wall time vs sim time ")
                    .border_style(Style::default().fg(theme.border)),
            )
            .x_axis(
                Axis::default()
                    .title("sim t")
                    .bounds([x_min, x_max])
                    .labels(vec![format!("{x_min:.2}"), format!("{x_max:.2}")])
                    .style(Style::default().fg(theme.dim)),
            )
            .y_axis(
                Axis::default()
                    .title("wall s")
                    .bounds([y_min, y_max])
                    .labels(vec![format!("{y_min:.1}"), format!("{y_max:.1}")])
                    .style(Style::default().fg(theme.dim)),
            );

        frame.render_widget(chart, area);
    }
}

fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else if secs < 3600.0 {
        format!("{}m{:02}s", secs as u64 / 60, secs as u64 % 60)
    } else {
        format!("{}h{:02}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
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
    (x_min, x_max, (y_min - ypad).max(0.0), y_max + ypad)
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
