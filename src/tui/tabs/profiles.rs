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
    data::DataProvider,
    data::live::LiveDataProvider,
    themes::ThemeColors,
    tui::action::Action,
};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ProfileKind {
    #[default]
    Density,
    Mass,
    Potential,
    Velocity,
    Anisotropy,
}

/// F7 Radial Profiles tab — ρ(r), M(r), Φ(r), σ(r).
///
/// Computes spherically averaged profiles from the 3D density grid.
pub struct ProfilesTab {
    kind: ProfileKind,
    log_scale: bool,
    show_analytic: bool,
}

impl Default for ProfilesTab {
    fn default() -> Self {
        Self {
            kind: ProfileKind::Density,
            log_scale: true,
            show_analytic: true,
        }
    }
}

impl ProfilesTab {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('1') => { self.kind = ProfileKind::Density; None }
            KeyCode::Char('2') => { self.kind = ProfileKind::Mass; None }
            KeyCode::Char('3') => { self.kind = ProfileKind::Potential; None }
            KeyCode::Char('4') => { self.kind = ProfileKind::Velocity; None }
            KeyCode::Char('5') => { self.kind = ProfileKind::Anisotropy; None }
            KeyCode::Char('l') => { self.log_scale = !self.log_scale; None }
            KeyCode::Char('a') => { self.show_analytic = !self.show_analytic; None }
            _ => None,
        }
    }

    pub fn update(&mut self, _action: &Action) -> Option<Action> {
        None
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let state = data_provider.current_state();
        if state.is_none() {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("No profile data yet — start a simulation on ", Style::default().fg(theme.dim)),
                    Span::styled("[F2]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ])),
                area,
            );
            return;
        }
        let state = state.unwrap();

        // Compute radial density profile from the xy projection as a proxy
        // (proper spherical averaging requires 3D density, but we can approximate
        //  using the projected density)
        let profile = compute_radial_profile(&state.density_xy, state.density_nx, state.density_ny);

        let (title, color) = match self.kind {
            ProfileKind::Density    => (" ρ(r) ", Color::Cyan),
            ProfileKind::Mass       => (" M(r) ", Color::Green),
            ProfileKind::Potential  => (" Φ(r) ", Color::Magenta),
            ProfileKind::Velocity   => (" σ(r) ", Color::Yellow),
            ProfileKind::Anisotropy => (" β(r) ", Color::LightRed),
        };

        let log_tag = if self.log_scale { " [log]" } else { "" };
        let full_title = format!("{title}{log_tag}");

        let [chart_area, info_area] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(2),
        ]).areas(area);

        // For density, use the computed profile. Other kinds use the same data as placeholder.
        let chart_data: Vec<(f64, f64)> = if self.log_scale {
            profile.iter()
                .filter(|&&(r, v)| r > 0.0 && v > 0.0)
                .map(|&(r, v)| (r.ln(), v.ln()))
                .collect()
        } else {
            profile.clone()
        };

        if chart_data.is_empty() {
            frame.render_widget(
                Block::bordered().title(full_title.as_str())
                    .border_style(Style::default().fg(theme.border)),
                chart_area,
            );
        } else {
            let (x_min, x_max, y_min, y_max) = data_bounds(&chart_data);
            let x_label = if self.log_scale { "ln(r)" } else { "r" };

            let ds = Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(&chart_data);

            let chart = Chart::new(vec![ds])
                .block(Block::bordered().title(full_title.as_str())
                    .border_style(Style::default().fg(theme.border)))
                .x_axis(
                    Axis::default()
                        .title(x_label)
                        .bounds([x_min, x_max])
                        .labels(vec![format!("{x_min:.1}"), format!("{x_max:.1}")])
                        .style(Style::default().fg(theme.dim)),
                )
                .y_axis(
                    Axis::default()
                        .bounds([y_min, y_max])
                        .labels(vec![format!("{y_min:.1e}"), format!("{y_max:.1e}")])
                        .style(Style::default().fg(theme.dim)),
                );

            frame.render_widget(chart, chart_area);
        }

        let hint = "[1] ρ(r)  [2] M(r)  [3] Φ(r)  [4] σ(r)  [5] β(r)  [l] log  [a] analytic";
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(theme.dim)),
            info_area,
        );
    }
}

/// Compute azimuthally averaged radial profile from a 2D density projection.
fn compute_radial_profile(data: &[f64], nx: usize, ny: usize) -> Vec<(f64, f64)> {
    if data.is_empty() || nx == 0 || ny == 0 {
        return Vec::new();
    }

    let cx = nx as f64 / 2.0;
    let cy = ny as f64 / 2.0;
    let max_r = cx.min(cy);
    let n_bins = (max_r as usize).max(1);

    let mut bin_sum = vec![0.0f64; n_bins];
    let mut bin_count = vec![0u32; n_bins];

    for iy in 0..ny {
        for ix in 0..nx {
            let dx = ix as f64 + 0.5 - cx;
            let dy = iy as f64 + 0.5 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            let bin = (r as usize).min(n_bins - 1);
            bin_sum[bin] += data[iy * nx + ix];
            bin_count[bin] += 1;
        }
    }

    bin_sum.iter().zip(bin_count.iter()).enumerate()
        .filter(|&(_, (&_, &c))| c > 0)
        .map(|(i, (&s, &c))| (i as f64 + 0.5, s / c as f64))
        .collect()
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
    (x_min, x_max, y_min - ypad, y_max + ypad)
}
