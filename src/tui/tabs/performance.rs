use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};
use std::collections::VecDeque;

use crate::{
    data::DataProvider,
    data::live::LiveDataProvider,
    themes::ThemeColors,
};

const WALL_TIME_CAP: usize = 500;

/// F8 Performance Dashboard — step wall-clock time, throughput, memory.
pub struct PerformanceTab {
    wall_times: VecDeque<(f64, f64)>, // (step#, ms)
}

impl Default for PerformanceTab {
    fn default() -> Self {
        Self {
            wall_times: VecDeque::with_capacity(WALL_TIME_CAP),
        }
    }
}

impl PerformanceTab {
    /// Ingest performance data from the latest SimState.
    pub fn ingest(&mut self, step: u64, wall_ms: f64) {
        if self.wall_times.len() >= WALL_TIME_CAP {
            self.wall_times.pop_front();
        }
        self.wall_times.push_back((step as f64, wall_ms));
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        // Ingest from current state if available
        if let Some(state) = data_provider.current_state() {
            if self.wall_times.back().map_or(true, |&(s, _)| s != state.step as f64) {
                self.ingest(state.step, state.step_wall_ms);
            }
        }

        let [top, bottom] = Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]).areas(area);

        let [stats_area, throughput_area] = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ]).areas(top);

        // Stats panel
        self.draw_stats(frame, stats_area, theme, data_provider);

        // Throughput chart
        self.draw_throughput_chart(frame, throughput_area, theme);

        // Wall time chart
        self.draw_wall_time_chart(frame, bottom, theme);
    }

    fn draw_stats(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let state = data_provider.current_state();
        let step = state.map(|s| s.step).unwrap_or(0);
        let last_ms = state.map(|s| s.step_wall_ms).unwrap_or(0.0);
        let avg_ms = if self.wall_times.is_empty() {
            0.0
        } else {
            self.wall_times.iter().map(|(_, ms)| ms).sum::<f64>() / self.wall_times.len() as f64
        };
        let max_ms = self.wall_times.iter().map(|(_, ms)| *ms).fold(0.0f64, f64::max);

        let nx = state.map(|s| s.density_nx).unwrap_or(0);
        let nv = state.map(|s| s.phase_nv).unwrap_or(0);
        let grid_size = if nx > 0 && nv > 0 {
            format!("{}³×{}³ = {:.1e} cells", nx, nv, (nx * nx * nx * nv * nv * nv) as f64)
        } else {
            "—".to_string()
        };

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Performance Stats", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  Step:        {step}"), Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(format!("  Last step:   {last_ms:.1} ms"), Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(format!("  Avg step:    {avg_ms:.1} ms"), Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(format!("  Max step:    {max_ms:.1} ms"), Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(format!("  Grid:        {grid_size}"), Style::default().fg(theme.fg)),
            ]),
        ];

        let block = Block::bordered()
            .title(" Stats ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_throughput_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let data: Vec<(f64, f64)> = self.wall_times.iter()
            .map(|&(step, ms)| (step, if ms > 0.0 { 1000.0 / ms } else { 0.0 }))
            .collect();

        if data.is_empty() {
            frame.render_widget(
                Block::bordered().title(" Steps/s ").border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let (x_min, x_max, y_min, y_max) = data_bounds(&data);

        let ds = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&data);

        let chart = Chart::new(vec![ds])
            .block(Block::bordered().title(" Steps/s ").border_style(Style::default().fg(theme.border)))
            .x_axis(Axis::default().bounds([x_min, x_max]).style(Style::default().fg(theme.dim)))
            .y_axis(
                Axis::default()
                    .bounds([y_min, y_max])
                    .labels(vec![format!("{y_min:.0}"), format!("{y_max:.0}")])
                    .style(Style::default().fg(theme.dim)),
            );

        frame.render_widget(chart, area);
    }

    fn draw_wall_time_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let data: Vec<(f64, f64)> = self.wall_times.iter().copied().collect();

        if data.is_empty() {
            frame.render_widget(
                Block::bordered().title(" Wall time (ms/step) ").border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let (x_min, x_max, y_min, y_max) = data_bounds(&data);

        let ds = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightRed))
            .data(&data);

        let chart = Chart::new(vec![ds])
            .block(Block::bordered().title(" Wall time (ms/step) ").border_style(Style::default().fg(theme.border)))
            .x_axis(
                Axis::default()
                    .title("step")
                    .bounds([x_min, x_max])
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
}

fn data_bounds(data: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for &(x, y) in data {
        if x < x_min { x_min = x; }
        if x > x_max { x_max = x; }
        if y < y_min { y_min = y; }
        if y > y_max { y_max = y; }
    }
    if x_min >= x_max { x_max = x_min + 1.0; }
    if y_min >= y_max { y_max = y_min + 1.0; }
    let ypad = (y_max - y_min) * 0.05;
    (x_min, x_max, (y_min - ypad).max(0.0), y_max + ypad)
}
