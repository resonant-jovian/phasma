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
    data::DataProvider, data::live::LiveDataProvider, themes::ThemeColors, tui::action::Action,
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

/// F7 Radial Profiles tab — ρ(r), M(r), Φ(r), σ(r), β(r).
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
            KeyCode::Char('1') => {
                self.kind = ProfileKind::Density;
                None
            }
            KeyCode::Char('2') => {
                self.kind = ProfileKind::Mass;
                None
            }
            KeyCode::Char('3') => {
                self.kind = ProfileKind::Potential;
                None
            }
            KeyCode::Char('4') => {
                self.kind = ProfileKind::Velocity;
                None
            }
            KeyCode::Char('5') => {
                self.kind = ProfileKind::Anisotropy;
                None
            }
            KeyCode::Char('l') => {
                self.log_scale = !self.log_scale;
                None
            }
            KeyCode::Char('a') => {
                self.show_analytic = !self.show_analytic;
                None
            }
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
                    Span::styled(
                        "No profile data yet — start a simulation on ",
                        Style::default().fg(theme.dim),
                    ),
                    Span::styled(
                        "[F2]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                area,
            );
            return;
        }
        let state = state.unwrap();

        // Compute radial density profile from the 3D density (using xy projection)
        let l_box = state.spatial_extent * 2.0; // full box size
        let nx = state.density_nx;
        let ny = state.density_ny;
        let dx = if nx > 0 { l_box / nx as f64 } else { 1.0 };
        let density_profile = compute_radial_profile(&state.density_xy, nx, ny, dx);

        // Derive other quantities from the density profile
        let g = state.gravitational_constant;
        let mass_profile = compute_mass_profile(&density_profile, dx);
        let potential_profile = compute_potential_profile(&mass_profile, g);
        let sigma_profile = compute_sigma_profile(&density_profile, &mass_profile, g);

        // Select the appropriate data
        let (title, color, profile_data) = match self.kind {
            ProfileKind::Density => (" ρ(r) ", Color::Cyan, &density_profile),
            ProfileKind::Mass => (" M(<r) ", Color::Green, &mass_profile),
            ProfileKind::Potential => (" Φ(r) ", Color::Magenta, &potential_profile),
            ProfileKind::Velocity => (" σ(r) ", Color::Yellow, &sigma_profile),
            ProfileKind::Anisotropy => (" β(r) ", Color::LightRed, &density_profile), // placeholder
        };

        // For anisotropy, show β=0 (isotropic) placeholder
        let aniso_data: Vec<(f64, f64)>;
        let chart_source = if self.kind == ProfileKind::Anisotropy {
            aniso_data = density_profile.iter().map(|&(r, _)| (r, 0.0)).collect();
            &aniso_data
        } else {
            profile_data
        };

        let log_tag = if self.log_scale { " [log]" } else { "" };
        let analytic_tag = if self.show_analytic { " +analytic" } else { "" };
        let full_title = format!("{title}{log_tag}{analytic_tag}");

        let [chart_area, info_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);

        // Apply log scaling
        let chart_data: Vec<(f64, f64)> = if self.log_scale && self.kind == ProfileKind::Potential {
            // Potential is negative, use log(|Φ|)
            chart_source
                .iter()
                .filter(|&&(r, v)| r > 0.0 && v < 0.0)
                .map(|&(r, v)| (r.ln(), (-v).ln()))
                .collect()
        } else if self.log_scale && self.kind != ProfileKind::Anisotropy {
            chart_source
                .iter()
                .filter(|&&(r, v)| r > 0.0 && v > 0.0)
                .map(|&(r, v)| (r.ln(), v.ln()))
                .collect()
        } else {
            chart_source.clone()
        };

        // Analytic overlay
        let analytic_data: Vec<(f64, f64)> = if self.show_analytic {
            let cfg = data_provider.config();
            compute_analytic_profile(cfg, self.kind, self.log_scale, &density_profile)
        } else {
            Vec::new()
        };

        if chart_data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(full_title.as_str())
                    .border_style(Style::default().fg(theme.border)),
                chart_area,
            );
        } else {
            let (x_min, x_max, y_min, y_max) = combined_bounds(&chart_data, &analytic_data);
            let x_label = if self.log_scale { "ln(r)" } else { "r" };

            let mut datasets = vec![
                Dataset::default()
                    .name("sim")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(color))
                    .data(&chart_data),
            ];

            if !analytic_data.is_empty() {
                datasets.push(
                    Dataset::default()
                        .name("analytic")
                        .marker(symbols::Marker::Braille)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::White))
                        .data(&analytic_data),
                );
            }

            let chart = Chart::new(datasets)
                .block(
                    Block::bordered()
                        .title(full_title.as_str())
                        .border_style(Style::default().fg(theme.border)),
                )
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

        let kind_labels = ["[1]ρ", "[2]M", "[3]Φ", "[4]σ", "[5]β"];
        let active = match self.kind {
            ProfileKind::Density => 0,
            ProfileKind::Mass => 1,
            ProfileKind::Potential => 2,
            ProfileKind::Velocity => 3,
            ProfileKind::Anisotropy => 4,
        };
        let mut hint_parts: Vec<String> = kind_labels
            .iter()
            .enumerate()
            .map(|(i, &label)| {
                if i == active {
                    format!("{label}*")
                } else {
                    label.to_string()
                }
            })
            .collect();
        hint_parts.push(format!(
            "  [l] log{}  [a] analytic{}",
            if self.log_scale { "*" } else { "" },
            if self.show_analytic { "*" } else { "" },
        ));
        let hint = hint_parts.join("  ");
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(theme.dim)),
            info_area,
        );
    }
}

/// Compute azimuthally averaged radial density profile from a 2D density projection.
/// Returns (physical_radius, density_value) pairs.
fn compute_radial_profile(data: &[f64], nx: usize, ny: usize, dx: f64) -> Vec<(f64, f64)> {
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
            let ddx = ix as f64 + 0.5 - cx;
            let ddy = iy as f64 + 0.5 - cy;
            let r = (ddx * ddx + ddy * ddy).sqrt();
            let bin = (r as usize).min(n_bins - 1);
            bin_sum[bin] += data[iy * nx + ix];
            bin_count[bin] += 1;
        }
    }

    bin_sum
        .iter()
        .zip(bin_count.iter())
        .enumerate()
        .filter(|&(_, (&_, &c))| c > 0)
        .map(|(i, (&s, &c))| ((i as f64 + 0.5) * dx, s / c as f64))
        .collect()
}

/// Compute cumulative enclosed mass M(<r) from radial density profile.
/// Integrates ρ(r) × 4πr² dr using simple trapezoidal summation.
fn compute_mass_profile(density: &[(f64, f64)], dx: f64) -> Vec<(f64, f64)> {
    let mut mass = Vec::with_capacity(density.len());
    let mut cumulative = 0.0;
    for &(r, rho) in density {
        // Shell volume ≈ 4πr² × dr (using bin width = dx)
        let shell_vol = 4.0 * std::f64::consts::PI * r * r * dx;
        cumulative += rho * shell_vol;
        mass.push((r, cumulative));
    }
    mass
}

/// Compute gravitational potential Φ(r) = -G × M(<r) / r.
fn compute_potential_profile(mass: &[(f64, f64)], g: f64) -> Vec<(f64, f64)> {
    mass.iter()
        .filter(|&&(r, _)| r > 0.0)
        .map(|&(r, m)| (r, -g * m / r))
        .collect()
}

/// Compute velocity dispersion σ(r) from Jeans equation (isotropic case).
/// σ²(r) = (1/ρ) ∫_r^∞ ρ(r') G M(r') / r'² dr'
fn compute_sigma_profile(density: &[(f64, f64)], mass: &[(f64, f64)], g: f64) -> Vec<(f64, f64)> {
    if density.len() != mass.len() || density.is_empty() {
        return Vec::new();
    }
    let n = density.len();
    let dr = if n > 1 {
        density[1].0 - density[0].0
    } else {
        1.0
    };

    // Integrate from outside in: σ²(r) = (1/ρ(r)) × Σ_{r'=r}^{r_max} ρ(r') × G × M(r') / r'² × dr
    let mut integral = vec![0.0f64; n];
    let mut running = 0.0;
    for i in (0..n).rev() {
        let r = density[i].0;
        let rho = density[i].1;
        let m = mass[i].1;
        if r > 0.0 {
            running += rho * g * m / (r * r) * dr;
        }
        integral[i] = running;
    }

    density
        .iter()
        .enumerate()
        .filter(|&(_, &(_, rho))| rho > 1e-30)
        .map(|(i, &(r, rho))| (r, (integral[i] / rho).sqrt()))
        .collect()
}

/// Compute analytic profile for known model types.
fn compute_analytic_profile(
    cfg: Option<&crate::config::PhasmaConfig>,
    kind: ProfileKind,
    log_scale: bool,
    density_profile: &[(f64, f64)],
) -> Vec<(f64, f64)> {
    let cfg = match cfg {
        Some(c) => c,
        None => return Vec::new(),
    };

    let model = &cfg.model.model_type;
    let m = cfg.model.total_mass;
    let a = cfg.model.scale_radius;
    let g = cfg.domain.gravitational_constant;

    // Generate r values matching the simulation bins
    let r_values: Vec<f64> = density_profile.iter().map(|&(r, _)| r).collect();

    let raw: Vec<(f64, f64)> = match model.as_str() {
        "plummer" => {
            r_values
                .iter()
                .map(|&r| {
                    let r2a2 = r * r + a * a;
                    match kind {
                        ProfileKind::Density => {
                            let rho = 3.0 * m / (4.0 * std::f64::consts::PI * a * a * a)
                                * (1.0 + r * r / (a * a)).powf(-2.5);
                            (r, rho)
                        }
                        ProfileKind::Mass => {
                            let mr = m * r * r * r / r2a2.powf(1.5);
                            (r, mr)
                        }
                        ProfileKind::Potential => {
                            let phi = -g * m / r2a2.sqrt();
                            (r, phi)
                        }
                        ProfileKind::Velocity => {
                            // Isotropic Plummer: σ²(r) = GM/(6a) × (1 + r²/a²)^(-1/2)
                            let sigma = (g * m / (6.0 * a) / (1.0 + r * r / (a * a)).sqrt()).sqrt();
                            (r, sigma)
                        }
                        ProfileKind::Anisotropy => (r, 0.0),
                    }
                })
                .collect()
        }
        "hernquist" => {
            r_values
                .iter()
                .filter(|&&r| r > 0.0)
                .map(|&r| {
                    match kind {
                        ProfileKind::Density => {
                            let rho = m / (2.0 * std::f64::consts::PI) * a / (r * (r + a).powi(3));
                            (r, rho)
                        }
                        ProfileKind::Mass => {
                            let mr = m * r * r / ((r + a) * (r + a));
                            (r, mr)
                        }
                        ProfileKind::Potential => {
                            let phi = -g * m / (r + a);
                            (r, phi)
                        }
                        ProfileKind::Velocity => {
                            // Approximate using isotropic Jeans
                            let mr = m * r * r / ((r + a) * (r + a));
                            let rho = m / (2.0 * std::f64::consts::PI) * a / (r * (r + a).powi(3));
                            if rho > 0.0 {
                                let sigma = (g * mr / (2.0 * r)).sqrt();
                                (r, sigma)
                            } else {
                                (r, 0.0)
                            }
                        }
                        ProfileKind::Anisotropy => (r, 0.0),
                    }
                })
                .collect()
        }
        "nfw" => {
            let c = cfg
                .model
                .nfw
                .as_ref()
                .map(|n| n.concentration)
                .unwrap_or(10.0);
            let rs = a; // scale radius
            let ln_factor = (1.0 + c).ln() - c / (1.0 + c);
            r_values
                .iter()
                .filter(|&&r| r > 0.0)
                .map(|&r| {
                    let x = r / rs;
                    match kind {
                        ProfileKind::Density => {
                            let rho = m
                                / (4.0 * std::f64::consts::PI * rs * rs * rs * ln_factor)
                                / (x * (1.0 + x) * (1.0 + x));
                            (r, rho)
                        }
                        ProfileKind::Mass => {
                            let mr = m * ((1.0 + x).ln() - x / (1.0 + x)) / ln_factor;
                            (r, mr)
                        }
                        ProfileKind::Potential => {
                            let phi = -g * m / (rs * ln_factor) * (1.0 + x).ln() / x;
                            (r, phi)
                        }
                        ProfileKind::Velocity => {
                            let mr = m * ((1.0 + x).ln() - x / (1.0 + x)) / ln_factor;
                            let sigma = (g * mr / (2.0 * r)).sqrt();
                            (r, sigma)
                        }
                        ProfileKind::Anisotropy => (r, 0.0),
                    }
                })
                .collect()
        }
        _ => Vec::new(), // No analytic formula for king, zeldovich, merger, etc.
    };

    // Apply log scaling to match the simulation data
    if log_scale && kind != ProfileKind::Anisotropy && kind != ProfileKind::Potential {
        raw.iter()
            .filter(|&&(r, v)| r > 0.0 && v > 0.0)
            .map(|&(r, v)| (r.ln(), v.ln()))
            .collect()
    } else if log_scale && kind == ProfileKind::Potential {
        raw.iter()
            .filter(|&&(r, v)| r > 0.0 && v < 0.0)
            .map(|&(r, v)| (r.ln(), (-v).ln()))
            .collect()
    } else {
        raw
    }
}

fn combined_bounds(a: &[(f64, f64)], b: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let (mut x_min, mut x_max, mut y_min, mut y_max) = data_bounds(a);
    if !b.is_empty() {
        let (bx0, bx1, by0, by1) = data_bounds(b);
        x_min = x_min.min(bx0);
        x_max = x_max.max(bx1);
        y_min = y_min.min(by0);
        y_max = y_max.max(by1);
    }
    (x_min, x_max, y_min, y_max)
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
