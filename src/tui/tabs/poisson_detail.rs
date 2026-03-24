use crossterm::event::KeyEvent;
use ratatui::style::Color;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use ratatui_plt::prelude::{
    Axis as PltAxis, Bounds, LinePlot, LineStyle, MarkerShape, Scale, Series,
};
use ratatui_plt::statistics::poly_fit;

use crate::data::DataProvider;
use crate::themes::ThemeColors;
use crate::tui::action::Action;
use crate::tui::plt_bridge::phasma_theme_to_plt;

/// F9 Poisson Detail tab — displays P(k) power spectrum,
/// Poisson residual time series, near-field correction, and solver statistics.
#[derive(Default)]
pub struct PoissonDetailTab {}

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
        // Compact mode: show only P(k) spectrum
        if area.width < 76 {
            self.draw_power_spectrum(frame, area, theme, data_provider);
            return;
        }

        // Layout: top    = P_Φ(k) (40%) + P_ρ(k) (30%) + E(k) (30%)
        //         middle = Poisson Residual L₂ (50%) + Near-field Correction (50%)
        //         bottom = Solver Stats (50%) + Green's Fn Rank (50%)
        let [top, middle, bottom] = Layout::vertical([
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
        ])
        .areas(area);

        let [top_left, top_mid, top_right] = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .areas(top);

        let [mid_left, mid_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(middle);

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom);

        self.draw_power_spectrum(frame, top_left, theme, data_provider);
        Self::draw_density_spectrum(frame, top_mid, theme, data_provider);
        Self::draw_field_energy_spectrum(frame, top_right, theme, data_provider);
        Self::draw_poisson_residual_chart(frame, mid_left, theme, data_provider);
        Self::draw_near_field_correction_chart(frame, mid_right, theme, data_provider);
        Self::draw_solver_stats(frame, bottom_left, theme, data_provider);
        Self::draw_green_rank_panel(frame, bottom_right, theme, data_provider);
    }

    fn draw_power_spectrum(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();

        let spec_data: Option<Vec<(f64, f64)>> = state.and_then(|s| {
            s.potential_power_spectrum.as_ref().map(|spec| {
                spec.iter()
                    .filter(|&&(k, p)| k > 0.0 && p > 0.0)
                    .map(|&(k, p)| (k, p))
                    .collect()
            })
        });

        match spec_data {
            Some(ref data) if data.len() >= 2 => {
                let plt_theme = phasma_theme_to_plt(theme);
                let mut plot = LinePlot::new()
                    .series(
                        Series::new("|\u{03a6}\u{0302}(k)|\u{00b2}")
                            .data(data.clone())
                            .color(theme.chart[0])
                            .marker(MarkerShape::Circle),
                    )
                    .x_axis(PltAxis::new().label("k").scale(Scale::Log(10.0)))
                    .y_axis(PltAxis::new().label("P(k)").scale(Scale::Log(10.0)))
                    .title(" P(k) Potential Power Spectrum ")
                    .show_legend(true)
                    .theme(plt_theme);

                if let Some((_slope, fit_series)) = spectral_slope_fit(data, Color::Gray) {
                    plot = plot.series(fit_series);
                }

                frame.render_widget(&plot, area);
            }
            _ => {
                let block = Block::bordered()
                    .title(" P(k) Potential Power Spectrum ")
                    .border_style(Style::default().fg(theme.border));
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

    fn draw_density_spectrum(
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();
        let spec_data: Option<Vec<(f64, f64)>> = state.and_then(|s| {
            s.density_power_spectrum.as_ref().map(|spec| {
                spec.iter()
                    .filter(|&&(k, p)| k > 0.0 && p > 0.0)
                    .copied()
                    .collect()
            })
        });

        match spec_data {
            Some(ref data) if data.len() >= 2 => {
                let plt_theme = phasma_theme_to_plt(theme);
                let mut plot = LinePlot::new()
                    .series(
                        Series::new("|\u{03c1}\u{0302}(k)|\u{00b2}")
                            .data(data.clone())
                            .color(theme.chart[1])
                            .marker(MarkerShape::Circle),
                    )
                    .x_axis(PltAxis::new().label("k").scale(Scale::Log(10.0)))
                    .y_axis(PltAxis::new().label("P\u{03c1}(k)").scale(Scale::Log(10.0)))
                    .title(" P\u{03c1}(k) Density Spectrum ")
                    .show_legend(true)
                    .theme(plt_theme);

                if let Some((_slope, fit_series)) = spectral_slope_fit(data, Color::Gray) {
                    plot = plot.series(fit_series);
                }

                frame.render_widget(&plot, area);
            }
            _ => {
                let block = Block::bordered()
                    .title(" P\u{03c1}(k) Density Spectrum ")
                    .border_style(Style::default().fg(theme.border));
                let inner = block.inner(area);
                frame.render_widget(block, area);
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "  Waiting for data...",
                        Style::default().fg(theme.dim),
                    ))),
                    inner,
                );
            }
        }
    }

    fn draw_field_energy_spectrum(
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let state = data_provider.current_state();
        let spec_data: Option<Vec<(f64, f64)>> = state.and_then(|s| {
            s.field_energy_spectrum.as_ref().map(|spec| {
                spec.iter()
                    .filter(|&&(k, e)| k > 0.0 && e > 0.0)
                    .copied()
                    .collect()
            })
        });

        match spec_data {
            Some(ref data) if data.len() >= 2 => {
                let plt_theme = phasma_theme_to_plt(theme);
                let mut plot = LinePlot::new()
                    .series(
                        Series::new("E(k)")
                            .data(data.clone())
                            .color(theme.chart[2])
                            .marker(MarkerShape::Circle),
                    )
                    .x_axis(PltAxis::new().label("k").scale(Scale::Log(10.0)))
                    .y_axis(PltAxis::new().label("E(k)").scale(Scale::Log(10.0)))
                    .title(" E(k) Field Energy Spectrum ")
                    .show_legend(true)
                    .theme(plt_theme);

                if let Some((_slope, fit_series)) = spectral_slope_fit(data, Color::Gray) {
                    plot = plot.series(fit_series);
                }

                frame.render_widget(&plot, area);
            }
            _ => {
                let block = Block::bordered()
                    .title(" E(k) Field Energy Spectrum ")
                    .border_style(Style::default().fg(theme.border));
                let inner = block.inner(area);
                frame.render_widget(block, area);
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "  Waiting for data...",
                        Style::default().fg(theme.dim),
                    ))),
                    inner,
                );
            }
        }
    }

    fn draw_poisson_residual_chart(
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let data = data_provider
            .diagnostics()
            .poisson_residual
            .iter_chart_data();

        if data.len() < 2 {
            let block = Block::bordered()
                .title(" Poisson Residual L\u{2082} ")
                .border_style(Style::default().fg(theme.border));
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

        let plt_theme = phasma_theme_to_plt(theme);
        let plot = LinePlot::new()
            .series(
                Series::new("residual")
                    .data(data)
                    .color(theme.chart[1])
                    .marker(MarkerShape::Circle),
            )
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new().label("L\u{2082}").scale(Scale::Log(10.0)))
            .title(" Poisson Residual L\u{2082} ")
            .theme(plt_theme);

        frame.render_widget(&plot, area);
    }

    fn draw_near_field_correction_chart(
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let data = data_provider
            .diagnostics()
            .near_field_correction_l2
            .iter_chart_data();

        if data.len() < 2 {
            let block = Block::bordered()
                .title(" Near-field Correction ")
                .border_style(Style::default().fg(theme.border));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled(
                    "  Waiting for data...",
                    Style::default().fg(theme.dim),
                )])),
                inner,
            );
            return;
        }

        let plt_theme = phasma_theme_to_plt(theme);
        let plot = LinePlot::new()
            .series(
                Series::new("near-field")
                    .data(data)
                    .color(theme.chart[3])
                    .marker(MarkerShape::Circle),
            )
            .x_axis(PltAxis::new().label("t"))
            .y_axis(PltAxis::new().label("L\u{2082}").scale(Scale::Log(10.0)))
            .title(" Near-field Correction ")
            .theme(plt_theme);

        frame.render_widget(&plot, area);
    }

    fn draw_solver_stats(
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
    fn draw_green_rank_panel(
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let block = Block::bordered()
            .title(" Green\u{2019}s Fn Rank (Braess-Hackbusch) ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let state = data_provider.current_state();
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  1/|x| \u{2248} \u{03a3} c\u{2096} exp(-\u{03b1}\u{2096}|x|\u{00b2})",
                Style::default().fg(theme.accent),
            )),
            Line::from(""),
        ];

        if let Some(s) = state {
            if let Some(rg) = s.green_function_rank {
                lines.push(Line::from(vec![
                    Span::styled("  R\u{1d33} terms: ", Style::default().fg(theme.dim)),
                    Span::styled(
                        format!("{rg}"),
                        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    "  R\u{1d33} terms: \u{2014}",
                    Style::default().fg(theme.dim),
                )));
            }
            if let Some(terms) = s.exp_sum_terms {
                lines.push(Line::from(vec![
                    Span::styled("  Exp-sum terms: ", Style::default().fg(theme.dim)),
                    Span::styled(format!("{terms}"), Style::default().fg(theme.fg)),
                ]));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "  R\u{1d33} terms: \u{2014}",
                Style::default().fg(theme.dim),
            )));
            lines.push(Line::from(Span::styled(
                "  (no sim data)",
                Style::default().fg(theme.dim),
            )));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Fit a power law P(k) ~ k^alpha to log-log spectrum data.
/// Returns a (slope, fit_series) pair for overlay, or None if insufficient data.
fn spectral_slope_fit(data: &[(f64, f64)], color: Color) -> Option<(f64, Series)> {
    if data.len() < 4 {
        return None;
    }
    let log_k: Vec<f64> = data.iter().map(|(k, _)| k.ln()).collect();
    let log_p: Vec<f64> = data.iter().map(|(_, p)| p.ln()).collect();
    let fit = poly_fit(&log_k, &log_p, 1)?;

    // coefficients: [a1, a0] where log(P) = a1*log(k) + a0
    let slope = fit.coefficients[0];

    // Generate fit line in original (k, P) space
    let k_min = data.first().map(|(k, _)| *k)?;
    let k_max = data.last().map(|(k, _)| *k)?;
    let fit_data: Vec<(f64, f64)> = vec![
        (k_min, (fit.eval(k_min.ln())).exp()),
        (k_max, (fit.eval(k_max.ln())).exp()),
    ];
    let label = format!("k^{slope:.1}");
    let series = Series::new(label)
        .data(fit_data)
        .color(color)
        .line_style(LineStyle::dashed());
    Some((slope, series))
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
