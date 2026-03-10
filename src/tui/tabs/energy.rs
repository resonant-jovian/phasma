use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use crate::{data::live::LiveDataProvider, themes::ThemeColors, tui::action::Action};

type SeriesData<'a> = (&'a str, Vec<(f64, f64)>, Color);

/// Which traces are visible in the energy chart
#[derive(Default)]
struct TraceVisibility {
    total_energy: bool,
    kinetic_energy: bool,
    potential_energy: bool,
}

pub struct EnergyTab {
    traces: TraceVisibility,
    show_drift: bool,      // show fractional drift or absolute values
    selected_panel: usize, // 0=energy, 1=mass, 2=casimir, 3=entropy
}

impl Default for EnergyTab {
    fn default() -> Self {
        Self {
            traces: TraceVisibility {
                total_energy: true,
                kinetic_energy: true,
                potential_energy: true,
            },
            show_drift: false,
            selected_panel: 0,
        }
    }
}

impl EnergyTab {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('t') => {
                self.traces.total_energy = !self.traces.total_energy;
                None
            }
            KeyCode::Char('k') => {
                self.traces.kinetic_energy = !self.traces.kinetic_energy;
                None
            }
            KeyCode::Char('w') => {
                self.traces.potential_energy = !self.traces.potential_energy;
                None
            }
            KeyCode::Char('d') => {
                self.show_drift = !self.show_drift;
                None
            }
            KeyCode::Char('1') => {
                self.selected_panel = 0;
                None
            }
            KeyCode::Char('2') => {
                self.selected_panel = 1;
                None
            }
            KeyCode::Char('3') => {
                self.selected_panel = 2;
                None
            }
            KeyCode::Char('4') => {
                self.selected_panel = 3;
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, _action: &Action) -> Option<Action> {
        None
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
                    Span::styled(
                        "No diagnostics data yet — start a simulation on ",
                        Style::default().fg(theme.dim),
                    ),
                    Span::styled(
                        "[F2]",
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                area,
            );
            return;
        }

        // 2×2 grid layout
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        let [top_left, top_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top);

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom);

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
            let drift_data = diag.energy_drift_series();
            draw_single_series(frame, area, " ΔE/E₀ ", &drift_data, theme.chart[3], theme);
        } else {
            let mut datasets = Vec::new();
            let e_data = diag.total_energy.iter_chart_data();
            let k_data = diag.kinetic_energy.iter_chart_data();
            let w_data = diag.potential_energy.iter_chart_data();

            if self.traces.total_energy && !e_data.is_empty() {
                datasets.push(("E_tot", e_data, theme.chart[0]));
            }
            if self.traces.kinetic_energy && !k_data.is_empty() {
                datasets.push(("T", k_data, theme.chart[1]));
            }
            if self.traces.potential_energy && !w_data.is_empty() {
                datasets.push(("W", w_data, theme.chart[2]));
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
        let drift = data_provider.diagnostics.mass_drift_series();
        draw_single_series(frame, area, " ΔM/M₀ ", &drift, theme.chart[4], theme);
    }

    fn draw_casimir_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let drift = data_provider.diagnostics.c2_drift_series();
        draw_single_series(frame, area, " ΔC₂/C₂₀ ", &drift, theme.chart[5], theme);
    }

    fn draw_entropy_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let data = data_provider.diagnostics.entropy.iter_chart_data();
        draw_single_series(frame, area, " S(t) ", &data, theme.chart[6], theme);
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
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
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
        .block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
        )
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
    series: &[SeriesData<'_>],
    theme: &ThemeColors,
) {
    if series.is_empty() {
        frame.render_widget(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
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

    let datasets: Vec<Dataset> = series
        .iter()
        .map(|(name, data, color)| {
            Dataset::default()
                .name(*name)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(*color))
                .data(data)
        })
        .collect();

    let chart = Chart::new(datasets)
        .block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
        )
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
    // Add 5% padding
    let ypad = (y_max - y_min) * 0.05;
    (x_min, x_max, y_min - ypad, y_max + ypad)
}
