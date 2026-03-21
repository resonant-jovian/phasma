use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use crate::{
    data::DataProvider,
    themes::ThemeColors,
    tui::action::Action,
    tui::chart_utils::{data_bounds, densify},
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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    /// Show one profile at a time (original)
    Single,
    /// Show 4 stacked profiles sharing log-r axis
    #[default]
    Stacked,
}

/// F7 Radial Profiles tab — ρ(r), M(r), Φ(r), σ(r), β(r).
///
/// Computes spherically averaged profiles from the 3D density grid.
/// In stacked mode, shows ρ, M, Φ, σ simultaneously sharing the r axis.
/// Bin count presets for radial profiles.
const BIN_PRESETS: &[usize] = &[16, 32, 64, 128];

/// Lagrangian radii percentiles we track.
const LAGRANGIAN_PCTS: [f64; 5] = [0.10, 0.25, 0.50, 0.75, 0.90];
const LAGRANGIAN_CAP: usize = 500;

struct CachedProfiles {
    density: Vec<(f64, f64)>,
    mass: Vec<(f64, f64)>,
    potential: Vec<(f64, f64)>,
    sigma: Vec<(f64, f64)>,
    cached_step: u64,
    cached_bin_preset: usize,
}

impl Default for CachedProfiles {
    fn default() -> Self {
        Self {
            density: Vec::new(),
            mass: Vec::new(),
            potential: Vec::new(),
            sigma: Vec::new(),
            cached_step: u64::MAX,
            cached_bin_preset: usize::MAX,
        }
    }
}

pub struct ProfilesTab {
    kind: ProfileKind,
    log_scale: bool,
    show_analytic: bool,
    layout_mode: LayoutMode,
    /// Index into BIN_PRESETS.
    bin_preset_idx: usize,
    /// Lagrangian radii history: 5 series (L10, L25, L50, L75, L90), each (t, r).
    lagrangian_history: [VecDeque<(f64, f64)>; 5],
    /// Last step we recorded Lagrangian radii for (to avoid duplicates).
    lagrangian_last_step: u64,
    /// Cached radial profiles (recomputed only when step or bin count changes).
    cached_profiles: CachedProfiles,
    /// Cached lagrangian series (Vec copies of VecDeque history).
    cached_lagrangian: [Vec<(f64, f64)>; 5],
    cached_lagrangian_len: usize,
}

impl Default for ProfilesTab {
    fn default() -> Self {
        Self {
            kind: ProfileKind::Density,
            log_scale: true,
            show_analytic: true,
            layout_mode: LayoutMode::Stacked,
            bin_preset_idx: 2, // default 64 bins
            lagrangian_history: std::array::from_fn(|_| VecDeque::with_capacity(LAGRANGIAN_CAP)),
            lagrangian_last_step: u64::MAX,
            cached_profiles: CachedProfiles::default(),
            cached_lagrangian: std::array::from_fn(|_| Vec::new()),
            cached_lagrangian_len: 0,
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
            KeyCode::Char('s') => {
                self.layout_mode = match self.layout_mode {
                    LayoutMode::Single => LayoutMode::Stacked,
                    LayoutMode::Stacked => LayoutMode::Single,
                };
                None
            }
            KeyCode::Char('b') => {
                // Cycle bin count preset
                self.bin_preset_idx = (self.bin_preset_idx + 1) % BIN_PRESETS.len();
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        if let Action::SimUpdate(state) = action
            && state.step != self.lagrangian_last_step
            && !state.density_xy.is_empty()
        {
            self.lagrangian_last_step = state.step;
            let l_box = state.spatial_extent * 2.0;
            let nx = state.density_nx;
            let ny = state.density_ny;
            let dx = if nx > 0 { l_box / nx as f64 } else { 1.0 };
            let profile = compute_radial_profile(&state.density_xy, nx, ny, dx, 64);
            let mass = compute_mass_profile(&profile, dx);
            if let Some(&(_, m_total)) = mass.last()
                && m_total > 0.0
            {
                for (k, &pct) in LAGRANGIAN_PCTS.iter().enumerate() {
                    let target = pct * m_total;
                    // Find first bin where cumulative mass >= target
                    let r = mass
                        .iter()
                        .find(|&&(_, m)| m >= target)
                        .map(|&(r, _)| r)
                        .unwrap_or(0.0);
                    if self.lagrangian_history[k].len() >= LAGRANGIAN_CAP {
                        self.lagrangian_history[k].pop_front();
                    }
                    self.lagrangian_history[k].push_back((state.t, r));
                }
            }
        }
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
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                area,
            );
            return;
        }
        let Some(state) = state else { return };

        // Recompute profiles only when step or bin count changes
        let n_bins = BIN_PRESETS[self.bin_preset_idx];
        if state.step != self.cached_profiles.cached_step
            || self.bin_preset_idx != self.cached_profiles.cached_bin_preset
        {
            let l_box = state.spatial_extent * 2.0;
            let nx = state.density_nx;
            let ny = state.density_ny;
            let dx = if nx > 0 { l_box / nx as f64 } else { 1.0 };
            let g = state.gravitational_constant;
            let density_profile = compute_radial_profile(&state.density_xy, nx, ny, dx, n_bins);
            let mass_profile = compute_mass_profile(&density_profile, dx);
            let potential_profile = compute_potential_profile(&mass_profile, g);
            let sigma_profile = compute_sigma_profile(&density_profile, &mass_profile, g);
            self.cached_profiles = CachedProfiles {
                density: density_profile,
                mass: mass_profile,
                potential: potential_profile,
                sigma: sigma_profile,
                cached_step: state.step,
                cached_bin_preset: self.bin_preset_idx,
            };
        }

        // Rebuild lagrangian series cache when history grows
        let total_lag_len: usize = self.lagrangian_history.iter().map(|h| h.len()).sum();
        if total_lag_len != self.cached_lagrangian_len {
            for i in 0..5 {
                self.cached_lagrangian[i] = self.lagrangian_history[i].iter().copied().collect();
            }
            self.cached_lagrangian_len = total_lag_len;
        }

        // Footer area
        let [main_area, info_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);

        // Compact mode: auto-switch to single panel when area is too narrow/short
        let effective_mode = if area.width < 76 || area.height < 20 {
            LayoutMode::Single
        } else {
            self.layout_mode
        };

        match effective_mode {
            LayoutMode::Stacked => {
                self.draw_stacked(frame, main_area, theme, data_provider);
            }
            LayoutMode::Single => {
                self.draw_single(frame, main_area, theme, data_provider);
            }
        }

        // Footer hint
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
        let mode_tag = match self.layout_mode {
            LayoutMode::Single => "single",
            LayoutMode::Stacked => "stacked",
        };
        let bins = BIN_PRESETS[self.bin_preset_idx];
        // Check if analytic is available for the current model
        let model_type = data_provider
            .config()
            .map(|c| c.model.model_type.as_str())
            .unwrap_or("");
        let analytic_supported = matches!(model_type, "plummer" | "hernquist" | "nfw");
        let analytic_tag = if self.show_analytic && analytic_supported {
            "*"
        } else if self.show_analytic {
            " (n/a)"
        } else {
            ""
        };
        hint_parts.push(format!(
            "  [l] log{}  [a] analytic{}  [s] {}  [b] bins:{}",
            if self.log_scale { "*" } else { "" },
            analytic_tag,
            mode_tag,
            bins,
        ));
        let hint = hint_parts.join("  ");
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(theme.dim)),
            info_area,
        );
    }

    fn draw_stacked(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        // 4 profile panels + Lagrangian radii stub, sharing the r axis
        let panels = Layout::vertical([
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(12),
        ])
        .split(area);

        let profiles: [(ProfileKind, &str, &[(f64, f64)], usize); 4] = [
            (
                ProfileKind::Density,
                " ρ(r) ",
                &self.cached_profiles.density,
                0,
            ),
            (ProfileKind::Mass, " M(<r) ", &self.cached_profiles.mass, 1),
            (
                ProfileKind::Potential,
                " Φ(r) ",
                &self.cached_profiles.potential,
                2,
            ),
            (
                ProfileKind::Velocity,
                " σ(r) ",
                &self.cached_profiles.sigma,
                4,
            ),
        ];

        for (i, &(kind, title, profile_data, color_idx)) in profiles.iter().enumerate() {
            self.draw_profile_panel(
                frame,
                panels[i],
                theme,
                data_provider,
                kind,
                title,
                profile_data,
                theme.chart[color_idx],
            );
        }

        // Lagrangian radii panel — L10/L25/L50/L75/L90 vs time
        self.draw_lagrangian_panel(frame, panels[4], theme);
    }

    fn draw_single(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let (kind, title, color, profile_data): (ProfileKind, &str, _, &[(f64, f64)]) =
            match self.kind {
                ProfileKind::Density => (
                    ProfileKind::Density,
                    " ρ(r) ",
                    theme.chart[0],
                    &self.cached_profiles.density,
                ),
                ProfileKind::Mass => (
                    ProfileKind::Mass,
                    " M(<r) ",
                    theme.chart[1],
                    &self.cached_profiles.mass,
                ),
                ProfileKind::Potential => (
                    ProfileKind::Potential,
                    " Φ(r) ",
                    theme.chart[2],
                    &self.cached_profiles.potential,
                ),
                ProfileKind::Velocity => (
                    ProfileKind::Velocity,
                    " σ(r) ",
                    theme.chart[4],
                    &self.cached_profiles.sigma,
                ),
                ProfileKind::Anisotropy => (
                    ProfileKind::Anisotropy,
                    " β(r) ",
                    theme.chart[3],
                    &self.cached_profiles.density,
                ),
            };

        // For anisotropy, show β=0 (isotropic) placeholder
        let aniso_data: Vec<(f64, f64)>;
        let chart_source = if self.kind == ProfileKind::Anisotropy {
            aniso_data = self
                .cached_profiles
                .density
                .iter()
                .map(|&(r, _)| (r, 0.0))
                .collect();
            &aniso_data
        } else {
            profile_data
        };

        self.draw_profile_panel(
            frame,
            area,
            theme,
            data_provider,
            kind,
            title,
            chart_source,
            color,
        );
    }

    fn draw_profile_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
        kind: ProfileKind,
        title: &str,
        profile_data: &[(f64, f64)],
        color: ratatui::style::Color,
    ) {
        let density_profile = &self.cached_profiles.density;
        let log_tag = if self.log_scale { " [log]" } else { "" };
        let full_title = format!("{title}{log_tag}");

        // Apply log scaling
        let chart_data: Vec<(f64, f64)> = if self.log_scale && kind == ProfileKind::Potential {
            profile_data
                .iter()
                .filter(|&&(r, v)| r > 0.0 && v < 0.0)
                .map(|&(r, v)| (r.ln(), (-v).ln()))
                .collect()
        } else if self.log_scale && kind != ProfileKind::Anisotropy {
            profile_data
                .iter()
                .filter(|&&(r, v)| r > 0.0 && v > 0.0)
                .map(|&(r, v)| (r.ln(), v.ln()))
                .collect()
        } else {
            profile_data.to_vec()
        };

        // Analytic overlay (only for plummer/hernquist/nfw models)
        let analytic_data: Vec<(f64, f64)> = if self.show_analytic {
            let cfg = data_provider.config();
            compute_analytic_profile(cfg, kind, self.log_scale, density_profile)
        } else {
            Vec::new()
        };
        let analytic_available = self.show_analytic && !analytic_data.is_empty();

        if chart_data.is_empty() {
            frame.render_widget(
                Block::bordered()
                    .title(full_title.as_str())
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let (x_min, x_max, y_min, y_max) = combined_bounds(&chart_data, &analytic_data);
        let x_label = if self.log_scale { "ln(r)" } else { "r" };

        let chart_width = area.width.saturating_sub(2) as usize;
        let target = chart_width * 2;
        let dense_chart = densify(&chart_data, target);
        let dense_analytic = densify(&analytic_data, target);

        let mut datasets = vec![
            Dataset::default()
                .name("sim")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(&dense_chart),
        ];

        if analytic_available && !dense_analytic.is_empty() {
            datasets.push(
                Dataset::default()
                    .name("analytic")
                    .marker(symbols::Marker::Dot)
                    .graph_type(GraphType::Line)
                    .style(
                        Style::default()
                            .fg(theme.chart[5 % theme.chart.len()])
                            .add_modifier(Modifier::DIM),
                    )
                    .data(&dense_analytic),
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

        frame.render_widget(chart, area);
    }

    fn draw_lagrangian_panel(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Lagrangian Radii ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Check if we have any data
        let has_data = self.lagrangian_history[0].len() >= 2;
        if !has_data {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Lagrangian radii appear once simulation runs",
                    Style::default().fg(theme.dim),
                )),
                inner,
            );
            return;
        }

        const LABELS: [&str; 5] = ["L10", "L25", "L50", "L75", "L90"];

        // Use cached series data (rebuilt in draw() when history grows)
        let series_data = &self.cached_lagrangian;

        // Find global bounds
        let (mut t_min, mut t_max, mut r_min, mut r_max) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for series in series_data {
            for &(t, r) in series {
                t_min = t_min.min(t);
                t_max = t_max.max(t);
                r_min = r_min.min(r);
                r_max = r_max.max(r);
            }
        }
        if t_min >= t_max {
            t_max = t_min + 1.0;
        }
        if r_min >= r_max {
            r_max = r_min + 1.0;
        }
        let rpad = (r_max - r_min) * 0.05;

        let target = inner.width.saturating_sub(2) as usize * 2;
        let dense: Vec<std::borrow::Cow<'_, [(f64, f64)]>> =
            series_data.iter().map(|s| densify(s, target)).collect();

        let datasets: Vec<Dataset> = dense
            .iter()
            .enumerate()
            .map(|(i, d)| {
                Dataset::default()
                    .name(LABELS[i])
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(theme.chart[i % theme.chart.len()]))
                    .data(d)
            })
            .collect();

        let chart = Chart::new(datasets)
            .x_axis(
                Axis::default()
                    .title("t")
                    .bounds([t_min, t_max])
                    .labels(vec![format!("{t_min:.2}"), format!("{t_max:.2}")])
                    .style(Style::default().fg(theme.dim)),
            )
            .y_axis(
                Axis::default()
                    .title("r")
                    .bounds([r_min - rpad, r_max + rpad])
                    .labels(vec![format!("{r_min:.2}"), format!("{r_max:.2}")])
                    .style(Style::default().fg(theme.dim)),
            );

        frame.render_widget(chart, inner);
    }
}

/// Compute azimuthally averaged radial density profile from a 2D density projection.
/// Returns (physical_radius, density_value) pairs.
fn compute_radial_profile(
    data: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    requested_bins: usize,
) -> Vec<(f64, f64)> {
    if data.is_empty() || nx == 0 || ny == 0 {
        return Vec::new();
    }

    let cx = nx as f64 / 2.0;
    let cy = ny as f64 / 2.0;
    let max_r = cx.min(cy);
    let n_bins = requested_bins.clamp(16, 256);
    let bin_width = max_r / n_bins as f64;

    let mut bin_sum = vec![0.0f64; n_bins];
    let mut bin_count = vec![0u32; n_bins];

    for iy in 0..ny {
        for ix in 0..nx {
            let ddx = ix as f64 + 0.5 - cx;
            let ddy = iy as f64 + 0.5 - cy;
            let r = (ddx * ddx + ddy * ddy).sqrt();
            let bin = ((r / bin_width) as usize).min(n_bins - 1);
            bin_sum[bin] += data[iy * nx + ix];
            bin_count[bin] += 1;
        }
    }

    bin_sum
        .iter()
        .zip(bin_count.iter())
        .enumerate()
        .filter(|&(_, (&_, &c))| c > 0)
        .map(|(i, (&s, &c))| ((i as f64 + 0.5) * bin_width * dx, s / c as f64))
        .collect()
}

/// Compute cumulative enclosed mass M(<r) from radial density profile.
/// Integrates ρ(r) * 4πr² dr using simple trapezoidal summation.
fn compute_mass_profile(density: &[(f64, f64)], dx: f64) -> Vec<(f64, f64)> {
    let mut mass = Vec::with_capacity(density.len());
    let mut cumulative = 0.0;
    for &(r, rho) in density {
        // Shell volume ~ 4πr² * dr (using bin width = dx)
        let shell_vol = 4.0 * std::f64::consts::PI * r * r * dx;
        cumulative += rho * shell_vol;
        mass.push((r, cumulative));
    }
    mass
}

/// Compute gravitational potential Φ(r) = -G * M(<r) / r.
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

    // Integrate from outside in: σ²(r) = (1/ρ(r)) * Σ_{r'=r}^{r_max} ρ(r') * G * M(r') / r'² * dr
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
    use rust_decimal::prelude::ToPrimitive;
    let m = cfg.model.total_mass.to_f64().unwrap_or(1.0);
    let a = cfg.model.scale_radius.to_f64().unwrap_or(1.0);
    let g = cfg.domain.gravitational_constant.to_f64().unwrap_or(1.0);

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
                            // Isotropic Plummer: σ²(r) = GM/(6a) * (1 + r²/a²)^(-1/2)
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
                .map(|n| n.concentration.to_f64().unwrap_or(10.0))
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
