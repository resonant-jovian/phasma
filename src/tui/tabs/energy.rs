use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use crate::{
    data::live::LiveDataProvider,
    themes::ThemeColors,
    tui::action::Action,
};

/// Which traces are visible in the energy chart
#[derive(Default)]
struct TraceVisibility {
    total_energy: bool,
    kinetic_energy: bool,
    potential_energy: bool,
    virial_ratio: bool,
}

pub struct EnergyTab {
    traces: TraceVisibility,
    show_drift: bool,     // show fractional drift or absolute values
    selected_panel: usize, // 0=energy, 1=mass, 2=casimir, 3=entropy
    /// Time window: None = auto-fit (show all data), Some((start, end)) = fixed window
    time_window: Option<(f64, f64)>,
}

impl Default for EnergyTab {
    fn default() -> Self {
        Self {
            traces: TraceVisibility {
                total_energy: true,
                kinetic_energy: true,
                potential_energy: true,
                virial_ratio: false,
            },
            show_drift: false,
            selected_panel: 0,
            time_window: None,
        }
    }
}

impl EnergyTab {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('t') => { self.traces.total_energy = !self.traces.total_energy; None }
            KeyCode::Char('k') => { self.traces.kinetic_energy = !self.traces.kinetic_energy; None }
            KeyCode::Char('w') => { self.traces.potential_energy = !self.traces.potential_energy; None }
            KeyCode::Char('v') => { self.traces.virial_ratio = !self.traces.virial_ratio; None }
            KeyCode::Char('d') => { self.show_drift = !self.show_drift; None }
            KeyCode::Char('1') => { self.selected_panel = 0; None }
            KeyCode::Char('2') => { self.selected_panel = 1; None }
            KeyCode::Char('3') => { self.selected_panel = 2; None }
            KeyCode::Char('4') => { self.selected_panel = 3; None }
            // Time scroll: h = scroll left, l = scroll right
            KeyCode::Char('h') | KeyCode::Left => {
                self.scroll_time(-0.1);
                None
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.scroll_time(0.1);
                None
            }
            // Expand/contract time window
            KeyCode::Char('H') => {
                self.expand_time(1.5);
                None
            }
            KeyCode::Char('L') => {
                self.expand_time(1.0 / 1.5);
                None
            }
            // Fit all data
            KeyCode::Char('f') => {
                self.time_window = None;
                None
            }
            _ => None,
        }
    }

    fn scroll_time(&mut self, frac: f64) {
        // Auto-initialize time window if not set — can't scroll without bounds
        if self.time_window.is_none() {
            return; // Will be set on first data point
        }
        if let Some((start, end)) = &mut self.time_window {
            let span = *end - *start;
            let shift = span * frac;
            *start += shift;
            *end += shift;
        }
    }

    fn expand_time(&mut self, factor: f64) {
        if self.time_window.is_none() {
            return;
        }
        if let Some((start, end)) = &mut self.time_window {
            let mid = (*start + *end) / 2.0;
            let half = (*end - *start) / 2.0 * factor;
            *start = mid - half;
            *end = mid + half;
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        // Auto-initialize time_window from first data if user starts scrolling
        if let Action::SimUpdate(state) = action {
            if self.time_window.is_none() && state.t > 0.0 {
                // Keep auto-fit until user scrolls
            }
        }
        None
    }

    /// Initialize time window from data bounds on first scroll
    fn ensure_time_window(&mut self, data_start: f64, data_end: f64) {
        if self.time_window.is_none() {
            self.time_window = Some((data_start, data_end));
        }
    }

    fn clip_data(&self, data: &[(f64, f64)]) -> Vec<(f64, f64)> {
        match self.time_window {
            None => data.to_vec(),
            Some((start, end)) => {
                data.iter()
                    .filter(|(t, _)| *t >= start && *t <= end)
                    .copied()
                    .collect()
            }
        }
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        if data_provider.diagnostics.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("No diagnostics data yet — start a simulation on ", Style::default().fg(theme.dim)),
                    Span::styled("[F2]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ])),
                area,
            );
            return;
        }

        // 2×2 grid layout
        let [top, bottom] = Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]).areas(area);

        let [top_left, top_right] = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]).areas(top);

        let [bottom_left, bottom_right] = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]).areas(bottom);

        self.draw_energy_chart(frame, top_left, theme, data_provider);
        self.draw_mass_chart(frame, top_right, theme, data_provider);
        self.draw_casimir_chart(frame, bottom_left, theme, data_provider);
        self.draw_entropy_chart(frame, bottom_right, theme, data_provider);
    }

    fn draw_energy_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let diag = &data_provider.diagnostics;

        if self.show_drift {
            let drift_data = self.clip_data(&diag.energy_drift_series());
            draw_single_series(frame, area, " ΔE/E₀ ", &drift_data, Color::Red, theme);
        } else {
            let mut datasets = Vec::new();
            let e_data = self.clip_data(&diag.total_energy.iter_chart_data());
            let k_data = self.clip_data(&diag.kinetic_energy.iter_chart_data());
            let w_data = self.clip_data(&diag.potential_energy.iter_chart_data());

            if self.traces.total_energy && !e_data.is_empty() {
                datasets.push(("E_tot", e_data, Color::Cyan));
            }
            if self.traces.kinetic_energy && !k_data.is_empty() {
                datasets.push(("T", k_data, Color::Green));
            }
            if self.traces.potential_energy && !w_data.is_empty() {
                datasets.push(("W", w_data, Color::Magenta));
            }

            draw_multi_series(frame, area, " Energy ", &datasets, theme);
        }
    }

    fn draw_mass_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let drift = self.clip_data(&data_provider.diagnostics.mass_drift_series());
        draw_single_series(frame, area, " ΔM/M₀ ", &drift, Color::Yellow, theme);
    }

    fn draw_casimir_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let drift = self.clip_data(&data_provider.diagnostics.c2_drift_series());
        draw_single_series(frame, area, " ΔC₂/C₂₀ ", &drift, Color::LightBlue, theme);
    }

    fn draw_entropy_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let data = self.clip_data(&data_provider.diagnostics.entropy.iter_chart_data());
        draw_single_series(frame, area, " S(t) ", &data, Color::LightGreen, theme);
    }
}

fn draw_single_series(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    data: &[(f64, f64)],
    color: Color,
    theme: &ThemeColors,
) {
    if data.is_empty() {
        frame.render_widget(
            Block::bordered().title(title).border_style(Style::default().fg(theme.border)),
            area,
        );
        return;
    }

    let (x_min, x_max, y_min, y_max) = data_bounds(data);

    let ds = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(color))
        .data(data);

    let chart = Chart::new(vec![ds])
        .block(Block::bordered().title(title).border_style(Style::default().fg(theme.border)))
        .x_axis(
            Axis::default()
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

fn draw_multi_series(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    series: &[(&str, Vec<(f64, f64)>, Color)],
    theme: &ThemeColors,
) {
    if series.is_empty() {
        frame.render_widget(
            Block::bordered().title(title).border_style(Style::default().fg(theme.border)),
            area,
        );
        return;
    }

    // Compute bounds over all series
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (_, data, _) in series {
        let (a, b, c, d) = data_bounds(data);
        x_min = x_min.min(a);
        x_max = x_max.max(b);
        y_min = y_min.min(c);
        y_max = y_max.max(d);
    }

    let datasets: Vec<Dataset> = series.iter().map(|(name, data, color)| {
        Dataset::default()
            .name(*name)
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(*color))
            .data(data)
    }).collect();

    let chart = Chart::new(datasets)
        .block(Block::bordered().title(title).border_style(Style::default().fg(theme.border)))
        .x_axis(
            Axis::default()
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
    // Add 5% padding
    let ypad = (y_max - y_min) * 0.05;
    (x_min, x_max, y_min - ypad, y_max + ypad)
}
