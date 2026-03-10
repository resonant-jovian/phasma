use std::collections::VecDeque;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use crate::data::DataProvider;
use crate::themes::ThemeColors;
use crate::tui::action::Action;

const MAX_HISTORY: usize = 500;

/// F9 Poisson Detail tab — displays P(k) power spectrum,
/// Poisson residual time series, and solver statistics.
pub struct PoissonDetailTab {
    residual_history: VecDeque<(f64, f64)>,
    potential_history: VecDeque<(f64, f64)>,
}

impl Default for PoissonDetailTab {
    fn default() -> Self {
        Self {
            residual_history: VecDeque::with_capacity(MAX_HISTORY),
            potential_history: VecDeque::with_capacity(MAX_HISTORY),
        }
    }
}

impl PoissonDetailTab {
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
        // Update history from current state
        if let Some(state) = data_provider.current_state() {
            let t = state.t;

            // Residual history
            if let Some(residual) = state.poisson_residual_l2 {
                let should_push = self
                    .residual_history
                    .back()
                    .is_none_or(|(last_t, _)| t > *last_t);
                if should_push {
                    if self.residual_history.len() >= MAX_HISTORY {
                        self.residual_history.pop_front();
                    }
                    self.residual_history.push_back((t, residual));
                }
            }

            // Potential energy history (kept for solver stats W(t) display)
            let w = state.potential_energy;
            let should_push_w = self
                .potential_history
                .back()
                .is_none_or(|(last_t, _)| t > *last_t);
            if should_push_w {
                if self.potential_history.len() >= MAX_HISTORY {
                    self.potential_history.pop_front();
                }
                self.potential_history.push_back((t, w));
            }
        }

        // Layout: top half = P(k) spectrum, bottom half = residual chart + solver stats
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(bottom);

        self.draw_power_spectrum(frame, top, theme, data_provider);
        self.draw_residual_chart(frame, bottom_left, theme);
        self.draw_solver_stats(frame, bottom_right, theme, data_provider);
    }

    fn draw_power_spectrum(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let block = Block::bordered()
            .title(" P(k) Potential Power Spectrum ")
            .border_style(Style::default().fg(theme.border));

        let state = data_provider.current_state();

        let spec_data: Option<Vec<(f64, f64)>> = state.and_then(|s| {
            s.potential_power_spectrum.as_ref().map(|spec| {
                spec.iter()
                    .filter(|&&(k, p)| k > 0.0 && p > 0.0)
                    .map(|&(k, p)| (k.log10(), p.log10()))
                    .collect()
            })
        });

        match spec_data {
            Some(ref data) if data.len() >= 2 => {
                let (x_min, x_max, y_min, y_max) = data_bounds(data);

                let dataset = Dataset::default()
                    .name("|\u{03a6}\u{0302}(k)|\u{00b2}")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Scatter)
                    .style(Style::default().fg(theme.chart[0]))
                    .data(data);

                let chart = Chart::new(vec![dataset])
                    .block(block)
                    .x_axis(
                        Axis::default()
                            .title("log\u{2081}\u{2080}(k)")
                            .bounds([x_min, x_max])
                            .labels(vec![
                                format!("{x_min:.1}"),
                                format!("{:.1}", (x_min + x_max) / 2.0),
                                format!("{x_max:.1}"),
                            ])
                            .style(Style::default().fg(theme.dim)),
                    )
                    .y_axis(
                        Axis::default()
                            .title("log\u{2081}\u{2080}(P)")
                            .bounds([y_min, y_max])
                            .labels(vec![
                                format!("{y_min:.1}"),
                                format!("{:.1}", (y_min + y_max) / 2.0),
                                format!("{y_max:.1}"),
                            ])
                            .style(Style::default().fg(theme.dim)),
                    );

                frame.render_widget(chart, area);
            }
            _ => {
                let inner = block.inner(area);
                frame.render_widget(block, area);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled(
                        "  Waiting for spectrum data...",
                        Style::default().fg(theme.dim),
                    )])),
                    inner,
                );
            }
        }
    }

    fn draw_residual_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Poisson Residual ||\u{2207}\u{00b2}\u{03a6} \u{2212} 4\u{03c0}G\u{03c1}||\u{2082} ")
            .border_style(Style::default().fg(theme.border));

        if self.residual_history.len() < 2 {
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled(
                    "  Collecting data...",
                    Style::default().fg(theme.dim),
                )])),
                inner,
            );
            return;
        }

        let data: Vec<(f64, f64)> = self.residual_history.iter().copied().collect();
        let (x_min, x_max, y_min, y_max) = data_bounds(&data);

        // Densify for smooth braille rendering
        let chart_width = area.width.saturating_sub(2) as usize;
        let dense = densify(&data, chart_width * 2);

        let dataset = Dataset::default()
            .name("residual")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.chart[1]))
            .data(&dense);

        let chart = Chart::new(vec![dataset])
            .block(block)
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

    fn draw_solver_stats(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let block = Block::bordered()
            .title(" Solver Stats ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let label_style = Style::default().fg(theme.dim);
        let value_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);

        let lines = if let Some(state) = data_provider.current_state() {
            let poisson_label = if state.poisson_type.is_empty() {
                "unknown".to_string()
            } else {
                state.poisson_type.clone()
            };

            let bc_label = derive_bc_label(&state.poisson_type);

            let nx = state.density_nx;
            let ny = state.density_ny;
            let nz = state.density_nz;
            let total_cells = nx * ny * nz;
            let extent = state.spatial_extent;

            let residual_str = match state.poisson_residual_l2 {
                Some(r) => format!("{r:.2e}"),
                None => "N/A".to_string(),
            };

            let w_str = format!("{:.4e}", state.potential_energy);

            let vr = state.virial_ratio;
            let virial_color = if (vr - 1.0).abs() < 0.1 {
                theme.ok
            } else {
                theme.warn
            };

            vec![
                Line::from(vec![
                    Span::styled(" Type:      ", label_style),
                    Span::styled(poisson_label, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" Grid:      ", label_style),
                    Span::styled(
                        format!("{nx}\u{00d7}{ny}\u{00d7}{nz} ({total_cells})"),
                        value_style,
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" BC:        ", label_style),
                    Span::styled(bc_label, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" G:         ", label_style),
                    Span::styled(format!("{:.4}", state.gravitational_constant), value_style),
                ]),
                Line::from(vec![
                    Span::styled(" Domain:    ", label_style),
                    Span::styled(
                        format!("[{:.1}, {:.1}]\u{00b3}", -extent, extent),
                        value_style,
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Peak \u{03c1}:    ", label_style),
                    Span::styled(format!("{:.4e}", state.max_density), value_style),
                ]),
                Line::from(vec![
                    Span::styled(" W(t):      ", label_style),
                    Span::styled(w_str, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" dt:        ", label_style),
                    Span::styled(format!("{:.4e}", state.dt), value_style),
                ]),
                Line::from(vec![
                    Span::styled(" Residual:  ", label_style),
                    Span::styled(residual_str, value_style),
                ]),
                Line::from(vec![
                    Span::styled(" 2T/|W|:    ", label_style),
                    Span::styled(
                        format!("{vr:.4}"),
                        Style::default()
                            .fg(virial_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
            ]
        } else {
            vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    " No data \u{2014} start a run on [F2]",
                    label_style,
                )]),
            ]
        };

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Derive boundary condition label from the poisson_type string.
fn derive_bc_label(poisson_type: &str) -> String {
    match poisson_type {
        "fft_periodic" | "fft" => "Periodic".to_string(),
        "fft_isolated" => "Isolated (Hockney-Eastwood zero-padding)".to_string(),
        "multigrid" => "Configurable (multigrid)".to_string(),
        "spherical_harmonics" => "Spherical".to_string(),
        "tree" => "Open (tree)".to_string(),
        "" => "N/A".to_string(),
        other => other.to_string(),
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

/// Linearly interpolate sparse data to fill braille chart resolution.
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
