use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use ratatui_plt::prelude::{
    AspectRatio, Axis as PltAxis, Bounds, Heatmap, Histogram as PltHistogram, Kde, LinePlot,
    Series, StairsDataset, StairsPlot,
};
use ratatui_plt::widgets::hexbin::HexbinPlot;

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    themes::ThemeColors,
    tui::{
        action::Action,
        plt_bridge::{NormMode, flat_to_grid_data, phasma_cmap_to_plt, phasma_theme_to_plt},
        widgets::data_cursor::DataCursor,
    },
};

pub struct PhaseSpaceTab {
    /// Which spatial dimension for x-axis (0=x₁, 1=x₂, 2=x₃)
    dim_x: usize,
    /// Which velocity dimension for y-axis (0=v₁, 1=v₂, 2=v₃)
    dim_v: usize,
    norm_mode: NormMode,
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
    /// Slice position offsets for the 4 hidden dimensions (-1.0 to 1.0 each).
    /// Order: first hidden, second hidden, third hidden, fourth hidden.
    slice_offsets: [f64; 4],
    /// When true, use physical extents for aspect ratio; when false, fill available area
    physical_aspect: bool,
    /// Toggle stream-count overlay (§2.2 F4 `[s]`)
    show_stream_count: bool,
    /// Toggle HexbinPlot rendering mode
    hexbin_mode: bool,
    /// Toggle velocity distribution histogram panel
    show_vel_histogram: bool,
    data_cursor: DataCursor,
    last_heatmap_area: Rect,
    last_data: Vec<f64>,
    last_nx: usize,
    last_ny: usize,
    last_state_step: u64,
}

impl Default for PhaseSpaceTab {
    fn default() -> Self {
        Self {
            dim_x: 0,
            dim_v: 0,
            norm_mode: NormMode::default(),
            colormap: Colormap::Viridis,
            show_info: true,
            zoom: 1.0,
            slice_offsets: [0.0; 4],
            physical_aspect: false,
            show_stream_count: false,
            hexbin_mode: false,
            show_vel_histogram: false,
            data_cursor: Default::default(),
            last_heatmap_area: Rect::default(),
            last_data: Vec::new(),
            last_nx: 0,
            last_ny: 0,
            last_state_step: u64::MAX,
        }
    }
}

impl PhaseSpaceTab {
    pub fn handle_scroll(&mut self, delta: i32) {
        if delta < 0 {
            self.zoom = (self.zoom * 1.15).min(8.0);
        } else {
            self.zoom = (self.zoom / 1.15).max(1.0);
            if self.zoom <= 1.01 {
                self.zoom = 1.0;
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            // Select spatial dimension with 1-3
            KeyCode::Char('1') => {
                self.dim_x = 0;
                None
            }
            KeyCode::Char('2') => {
                self.dim_x = 1;
                None
            }
            KeyCode::Char('3') => {
                self.dim_x = 2;
                None
            }
            // Select velocity dimension with 4-6
            KeyCode::Char('4') => {
                self.dim_v = 0;
                None
            }
            KeyCode::Char('5') => {
                self.dim_v = 1;
                None
            }
            KeyCode::Char('6') => {
                self.dim_v = 2;
                None
            }
            KeyCode::Char('l') => {
                self.norm_mode = self.norm_mode.next();
                None
            }
            KeyCode::Char('i') => {
                self.show_info = !self.show_info;
                None
            }
            KeyCode::Char('+') => {
                self.zoom = (self.zoom * 1.25).min(8.0);
                None
            }
            KeyCode::Char('-') => {
                self.zoom = (self.zoom / 1.25).max(0.25);
                None
            }
            KeyCode::Char('r') | KeyCode::Char('0') => {
                self.zoom = 1.0;
                None
            }
            // Slice position: ,/. = 1st hidden dim, (/) = 2nd, {/} = 3rd, </> = 4th
            KeyCode::Char(',') => {
                self.slice_offsets[0] = (self.slice_offsets[0] - 0.1).max(-1.0);
                None
            }
            KeyCode::Char('.') => {
                self.slice_offsets[0] = (self.slice_offsets[0] + 0.1).min(1.0);
                None
            }
            KeyCode::Char('(') => {
                self.slice_offsets[1] = (self.slice_offsets[1] - 0.1).max(-1.0);
                None
            }
            KeyCode::Char(')') => {
                self.slice_offsets[1] = (self.slice_offsets[1] + 0.1).min(1.0);
                None
            }
            // Slice position: {/} = 3rd hidden dim, </> = 4th hidden dim
            KeyCode::Char('{') => {
                self.slice_offsets[2] = (self.slice_offsets[2] - 0.1).max(-1.0);
                None
            }
            KeyCode::Char('}') => {
                self.slice_offsets[2] = (self.slice_offsets[2] + 0.1).min(1.0);
                None
            }
            KeyCode::Char('<') => {
                self.slice_offsets[3] = (self.slice_offsets[3] - 0.1).max(-1.0);
                None
            }
            KeyCode::Char('>') => {
                self.slice_offsets[3] = (self.slice_offsets[3] + 0.1).min(1.0);
                None
            }
            KeyCode::Char('p') => {
                self.physical_aspect = !self.physical_aspect;
                None
            }
            // §2.2 F4 overlays (stubs — data not yet available from caustic)
            KeyCode::Char('s') => {
                self.show_stream_count = !self.show_stream_count;
                None
            }
            KeyCode::Char('v') => {
                self.show_vel_histogram = !self.show_vel_histogram;
                None
            }
            KeyCode::Char('x') => {
                self.hexbin_mode = !self.hexbin_mode;
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::VizCycleColormap => {
                self.colormap = self.colormap.next();
            }
            Action::VizToggleLog => {
                self.norm_mode = self.norm_mode.next();
            }
            _ => {}
        }
        None
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        colormap: Colormap,
        data_provider: &dyn DataProvider,
    ) {
        let effective_cmap = colormap;

        // Build fixed-dim slice specifications from the 4 hidden dimension offsets.
        // The 6 dimensions are [x1, x2, x3, v1, v2, v3]; the visible pair is
        // (dim_x, 3+dim_v). The remaining 4 are "hidden" and can be sliced.
        let visible_x = self.dim_x;
        let visible_v = 3 + self.dim_v;
        let mut hidden = [0usize; 4];
        let mut n_hidden = 0;
        for d in 0..6 {
            if d != visible_x && d != visible_v {
                hidden[n_hidden] = d;
                n_hidden += 1;
            }
        }
        let mut fixed = [(0usize, 0.0f64); 4];
        let mut n_fixed = 0;
        for (i, &dim) in hidden[..n_hidden].iter().enumerate() {
            let off = self.slice_offsets[i];
            if off.abs() > 0.001 {
                fixed[n_fixed] = (dim, off);
                n_fixed += 1;
            }
        }
        let Some((data, nx, nv)) =
            data_provider.phase_slice(self.dim_x, self.dim_v, &fixed[..n_fixed])
        else {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "No phase-space data yet — start a simulation on ",
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
        };

        let dim_labels = ["x", "y", "z"];
        let vel_labels = ["vx", "vy", "vz"];
        let slice_info = {
            let mut parts = Vec::new();
            for (i, &off) in self.slice_offsets.iter().enumerate() {
                if off.abs() > 0.01 {
                    parts.push(format!("s{}={:+.1}", i + 1, off));
                }
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!(" [{}]", parts.join(","))
            }
        };
        let title = format!(
            " f({}, {}){}{}",
            dim_labels[self.dim_x],
            vel_labels[self.dim_v],
            self.norm_mode.tag(),
            slice_info,
        );

        let [main_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            [area, Rect::new(area.x, area.y, 0, 0)]
        };

        // Split main area for optional marginal histograms (JointPlot-like layout)
        let (heatmap_area, hist_area, spatial_marginal_area) =
            if self.show_vel_histogram && main_area.width >= 50 && main_area.height >= 15 {
                // Top: spatial marginal rho(x), Center+Right: heatmap + velocity marginal
                let [top_marginal, center] =
                    Layout::vertical([Constraint::Length(6), Constraint::Min(8)]).areas(main_area);
                let [hm, hi] =
                    Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                        .areas(center);
                (hm, Some(hi), Some(top_marginal))
            } else if self.show_vel_histogram && main_area.width >= 50 {
                let [hm, hi] =
                    Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                        .areas(main_area);
                (hm, Some(hi), None)
            } else {
                (main_area, None, None)
            };

        let (view_data, vnx, vnv) = crop_data(&data, nx, nv, self.zoom);

        // Use physical extents for aspect ratio when enabled
        let state = data_provider.current_state();
        let cfg = data_provider.config();
        let x_extent = state.map(|s| s.spatial_extent).unwrap_or(vnx as f64 / 2.0);
        let v_extent = cfg
            .map(|c| {
                use rust_decimal::prelude::ToPrimitive;
                c.domain.velocity_extent.to_f64().unwrap_or(5.0)
            })
            .unwrap_or(vnv as f64 / 2.0);

        let aspect = if self.physical_aspect {
            // Convert float ratio to integer pair (2 decimal places of precision)
            let ratio = x_extent / v_extent;
            let w = (ratio * 100.0).round() as u16;
            AspectRatio::Ratio(w, 100)
        } else {
            AspectRatio::Auto
        };

        let grid = flat_to_grid_data(
            &view_data,
            vnx,
            vnv,
            (-x_extent, x_extent),
            (-v_extent, v_extent),
        );

        let (vmin, vmax) = grid.value_bounds();
        let plt_theme = phasma_theme_to_plt(theme);

        if self.hexbin_mode {
            // Convert grid to scatter points for HexbinPlot
            let dx = 2.0 * x_extent / vnx.max(1) as f64;
            let dv = 2.0 * v_extent / vnv.max(1) as f64;
            let mut points = Vec::new();
            for iv in 0..vnv {
                for ix in 0..vnx {
                    let val = view_data.get(iv * vnx + ix).copied().unwrap_or(0.0);
                    if val > 0.0 {
                        let x = -x_extent + (ix as f64 + 0.5) * dx;
                        let v = -v_extent + (iv as f64 + 0.5) * dv;
                        // Weight by value: repeat point proportionally
                        let reps = (val / vmax * 10.0).ceil().max(1.0) as usize;
                        for _ in 0..reps.min(20) {
                            points.push((x, v));
                        }
                    }
                }
            }
            let hexbin = HexbinPlot::new(points)
                .gridsize(vnx.min(30).max(5))
                .colormap(phasma_cmap_to_plt(effective_cmap))
                .title(format!("{title} [hexbin]"))
                .x_axis(PltAxis::new().bounds(Bounds::Manual(-x_extent, x_extent)))
                .y_axis(PltAxis::new().bounds(Bounds::Manual(-v_extent, v_extent)))
                .theme(plt_theme);
            frame.render_widget(&hexbin, heatmap_area);
        } else {
            let mut hm = Heatmap::new(grid)
                .colormap(phasma_cmap_to_plt(effective_cmap))
                .title(title.clone())
                .aspect_ratio(aspect)
                .show_colorbar(true)
                .theme(plt_theme);

            hm = self.norm_mode.apply_to_heatmap(hm, vmin, vmax);

            frame.render_widget(&hm, heatmap_area);
        }

        // Cache data for mouse cursor lookups — only copy when data actually changed
        self.last_heatmap_area = heatmap_area;
        if let Some(s) = state
            && (s.step != self.last_state_step || vnx != self.last_nx || vnv != self.last_ny)
        {
            self.last_data = view_data.into_owned();
            self.last_state_step = s.step;
            self.last_nx = vnx;
            self.last_ny = vnv;
        }

        // Velocity histogram panel — marginal velocity distribution as StairsPlot + KDE overlay
        if let Some(ha) = hist_area {
            if !self.last_data.is_empty() && self.last_nx > 0 && self.last_ny > 0 {
                // Sum columns to get velocity marginal (sum over x for each v bin)
                let vel_marginal: Vec<f64> = (0..self.last_ny)
                    .map(|iv| {
                        (0..self.last_nx)
                            .map(|ix| {
                                self.last_data
                                    .get(iv * self.last_nx + ix)
                                    .copied()
                                    .unwrap_or(0.0)
                            })
                            .sum()
                    })
                    .collect();

                let plt_theme = phasma_theme_to_plt(theme);

                // Build bin edges (n+1 edges for n bins)
                let n_bins = self.last_ny;
                let dv = if n_bins > 0 {
                    2.0 * v_extent / n_bins as f64
                } else {
                    1.0
                };
                let edges: Vec<f64> = (0..=n_bins).map(|i| -v_extent + dv * i as f64).collect();

                let stairs = StairsPlot::new()
                    .dataset(StairsDataset::new(
                        "f(v)",
                        edges,
                        vel_marginal.clone(),
                        theme.chart[0],
                    ))
                    .x_axis(PltAxis::new().label("v"))
                    .y_axis(PltAxis::new().label("f"))
                    .title(" Velocity Distribution ")
                    .show_legend(false)
                    .baseline(0.0)
                    .theme(plt_theme.clone());

                frame.render_widget(&stairs, ha);

                // KDE overlay — expand binned marginal into weighted samples for KDE
                let marginal_sum: f64 = vel_marginal.iter().sum();
                if marginal_sum > 0.0 && n_bins >= 2 {
                    // Build bin centers
                    let bin_centers: Vec<f64> = (0..n_bins)
                        .map(|i| -v_extent + dv * (i as f64 + 0.5))
                        .collect();

                    // Create weighted sample: replicate each bin center proportionally
                    // to its marginal value (normalized to ~200 total samples for KDE)
                    let target_samples = 200usize;
                    let mut raw_velocity_data = Vec::with_capacity(target_samples + n_bins);
                    for (i, &count) in vel_marginal.iter().enumerate() {
                        let n_reps =
                            ((count / marginal_sum) * target_samples as f64).round() as usize;
                        for _ in 0..n_reps {
                            raw_velocity_data.push(bin_centers[i]);
                        }
                    }

                    if raw_velocity_data.len() >= 2 {
                        let kde = Kde::default();
                        let (eval_points, densities) = kde.fit(&raw_velocity_data);

                        // Scale KDE densities to match histogram magnitude
                        let kde_max = densities.iter().cloned().fold(0.0_f64, f64::max);
                        let hist_max = vel_marginal.iter().cloned().fold(0.0_f64, f64::max);

                        if kde_max > 0.0 {
                            let scale = hist_max / kde_max;
                            let kde_data: Vec<(f64, f64)> = eval_points
                                .iter()
                                .zip(densities.iter())
                                .map(|(&x, &y)| (x, y * scale))
                                .collect();

                            let kde_color = if theme.chart.len() > 2 {
                                theme.chart[2]
                            } else {
                                theme.chart[theme.chart.len() - 1]
                            };
                            let kde_series = Series::new("KDE").data(kde_data).color(kde_color);

                            let kde_plot = LinePlot::new()
                                .series(kde_series)
                                .x_axis(
                                    PltAxis::new()
                                        .label("v")
                                        .bounds(Bounds::Manual(-v_extent, v_extent)),
                                )
                                .y_axis(
                                    PltAxis::new()
                                        .label("f")
                                        .bounds(Bounds::Manual(0.0, hist_max * 1.05)),
                                )
                                .title(" Velocity Distribution ")
                                .show_legend(false)
                                .theme(plt_theme);

                            frame.render_widget(&kde_plot, ha);
                        }
                    }
                }
            }
        }

        // Spatial marginal rho(x) panel (top strip, JointPlot-like)
        if let Some(sm_area) = spatial_marginal_area {
            if !self.last_data.is_empty() && self.last_nx > 0 && self.last_ny > 0 {
                // Sum rows to get spatial marginal (sum over v for each x bin)
                let spatial_marginal: Vec<f64> = (0..self.last_nx)
                    .map(|ix| {
                        (0..self.last_ny)
                            .map(|iv| {
                                self.last_data
                                    .get(iv * self.last_nx + ix)
                                    .copied()
                                    .unwrap_or(0.0)
                            })
                            .sum()
                    })
                    .collect();

                let plt_theme = phasma_theme_to_plt(theme);
                let n_bins = self.last_nx;
                let dx = if n_bins > 0 {
                    2.0 * x_extent / n_bins as f64
                } else {
                    1.0
                };
                let edges: Vec<f64> = (0..=n_bins).map(|i| -x_extent + dx * i as f64).collect();

                let stairs = StairsPlot::new()
                    .dataset(StairsDataset::new(
                        "ρ(x)",
                        edges,
                        spatial_marginal,
                        theme.chart[1],
                    ))
                    .x_axis(
                        PltAxis::new()
                            .label("x")
                            .bounds(Bounds::Manual(-x_extent, x_extent)),
                    )
                    .y_axis(PltAxis::new().label("ρ"))
                    .title(" Spatial Marginal ")
                    .show_legend(false)
                    .baseline(0.0)
                    .theme(plt_theme);

                frame.render_widget(&stairs, sm_area);
            }
        }

        if self.show_info && info_area.width > 0 {
            let scrub_hint = if let Some((idx, total)) = data_provider.scrub_position() {
                format!("  SCRUB {}/{total}", idx + 1)
            } else {
                String::new()
            };
            let stream_tag = if self.show_stream_count { " S" } else { "" };
            let hint = format!(
                "[1-3] x={}  [4-6] v={}  [+/-] zoom  [l] norm  [,/.] s1  [(/) s2  {{/}} s3  </> s4]  [p] aspect  [s] stream{stream_tag}  [i] hide{scrub_hint}",
                dim_labels[self.dim_x], vel_labels[self.dim_v],
            );
            frame.render_widget(
                Paragraph::new(hint).style(Style::default().fg(theme.dim)),
                info_area,
            );
        }

        // Data cursor tooltip (always drawn last, on top)
        self.data_cursor.draw(frame);
    }

    pub fn handle_mouse_move(&mut self, col: u16, row: u16) {
        let area = self.last_heatmap_area;
        if area.width == 0 || area.height == 0 || self.last_data.is_empty() {
            self.data_cursor.hide();
            return;
        }
        // Check if mouse is within the heatmap area (with 1-cell border for block)
        let inner_x = area.x + 1;
        let inner_y = area.y + 1;
        let inner_w = area.width.saturating_sub(2);
        let inner_h = area.height.saturating_sub(2);
        if col < inner_x || col >= inner_x + inner_w || row < inner_y || row >= inner_y + inner_h {
            self.data_cursor.hide();
            return;
        }
        let frac_x = (col - inner_x) as f64 / inner_w as f64;
        let frac_y = (row - inner_y) as f64 / inner_h as f64;
        let ix = ((frac_x * self.last_nx as f64) as usize).min(self.last_nx.saturating_sub(1));
        let iy = ((frac_y * self.last_ny as f64) as usize).min(self.last_ny.saturating_sub(1));
        let flat = iy * self.last_nx + ix;
        if let Some(&val) = self.last_data.get(flat) {
            self.data_cursor.show(
                col,
                row.saturating_sub(3),
                format!("[{ix},{iy}] = {val:.4e}"),
            );
        } else {
            self.data_cursor.hide();
        }
    }
}

fn crop_data<'a>(
    data: &'a [f64],
    nx: usize,
    ny: usize,
    zoom: f32,
) -> (Cow<'a, [f64]>, usize, usize) {
    if zoom <= 1.0 {
        return (Cow::Borrowed(data), nx, ny);
    }
    let view_w = (nx as f32 / zoom).ceil().max(1.0) as usize;
    let view_h = (ny as f32 / zoom).ceil().max(1.0) as usize;
    let view_w = view_w.min(nx);
    let view_h = view_h.min(ny);

    let x0 = (nx - view_w) / 2;
    let y0 = (ny - view_h) / 2;

    let mut out = Vec::with_capacity(view_w * view_h);
    for iy in y0..y0 + view_h {
        let iy = iy.min(ny - 1);
        for ix in x0..x0 + view_w {
            let ix = ix.min(nx - 1);
            out.push(data[iy * nx + ix]);
        }
    }
    (Cow::Owned(out), view_w, view_h)
}
