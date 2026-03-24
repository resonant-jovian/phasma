use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};
use ratatui_plt::prelude::{
    AspectRatio, Axis as PltAxis, Bounds, ContourPlot, GridData, Heatmap, InsetAxes, LinePlot,
    Series, Spines, VectorField, VectorFieldData,
};
use ratatui_plt::widgets::streamplot::StreamPlot;

use ratatui_plt::prelude::Theme;

use crate::{
    data::DataProvider,
    tui::widgets::data_cursor::DataCursor,
    tui::widgets::zoom,
    tui::{
        action::Action,
        plt_bridge::{NormMode, PhasmaThemeExt, flat_to_grid_data},
    },
};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum ContourMode {
    #[default]
    HeatmapOnly,
    HeatmapContour,
    FilledContour,
}

impl ContourMode {
    fn next(self) -> Self {
        match self {
            ContourMode::HeatmapOnly => ContourMode::HeatmapContour,
            ContourMode::HeatmapContour => ContourMode::FilledContour,
            ContourMode::FilledContour => ContourMode::HeatmapOnly,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            ContourMode::HeatmapOnly => "",
            ContourMode::HeatmapContour => " [contour]",
            ContourMode::FilledContour => " [filled]",
        }
    }
}

pub struct DensityTab {
    axis: usize, // 0=yz, 1=xz, 2=xy (default)
    norm_mode: NormMode,
    show_marginals: bool,
    show_info: bool,
    zoom: f32,
    contour_mode: ContourMode,
    show_acceleration: bool,
    show_streamlines: bool,
    show_inset: bool,
    data_cursor: DataCursor,
    last_heatmap_area: Rect,
    last_data: Vec<f64>,
    last_nx: usize,
    last_ny: usize,
    last_state_step: u64,
    /// When true, the data cursor shows physical coordinates instead of grid indices.
    crosshair_mode: bool,
    /// Cached physical coordinates at the current cursor position.
    crosshair_data_pos: Option<(f64, f64)>,
    /// Cached spatial half-extent from the last draw call.
    last_extent: f64,
}

impl Default for DensityTab {
    fn default() -> Self {
        Self {
            axis: 2,
            norm_mode: NormMode::default(),
            show_marginals: false,
            show_info: true,
            zoom: 1.0,
            contour_mode: ContourMode::default(),
            show_acceleration: false,
            show_streamlines: false,
            show_inset: false,
            data_cursor: DataCursor::default(),
            last_heatmap_area: Rect::default(),
            last_data: Vec::new(),
            last_nx: 0,
            last_ny: 0,
            last_state_step: u64::MAX,
            crosshair_mode: false,
            crosshair_data_pos: None,
            last_extent: 0.0,
        }
    }
}

impl DensityTab {
    pub fn handle_scroll(&mut self, delta: i32) {
        zoom::apply_scroll_zoom(&mut self.zoom, delta);
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('1') => {
                self.axis = 0;
                None
            }
            KeyCode::Char('2') => {
                self.axis = 1;
                None
            }
            KeyCode::Char('3') => {
                self.axis = 2;
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
            KeyCode::Char('n') => {
                self.contour_mode = self.contour_mode.next();
                None
            }
            KeyCode::Char('m') => {
                self.show_marginals = !self.show_marginals;
                None
            }
            KeyCode::Char('c') => {
                self.crosshair_mode = !self.crosshair_mode;
                if !self.crosshair_mode {
                    self.crosshair_data_pos = None;
                }
                None
            }
            code @ (KeyCode::Char('+')
            | KeyCode::Char('-')
            | KeyCode::Char('r')
            | KeyCode::Char('0')) => {
                zoom::apply_key_zoom(&mut self.zoom, code);
                None
            }
            KeyCode::Char('g') => {
                self.show_acceleration = !self.show_acceleration;
                None
            }
            KeyCode::Char('f') => {
                self.show_streamlines = !self.show_streamlines;
                None
            }
            KeyCode::Char('I') => {
                self.show_inset = !self.show_inset;
                None
            }
            _ => None,
        }
    }

    pub fn handle_mouse_move(&mut self, col: u16, row: u16) {
        let heatmap_area = self.last_heatmap_area;
        let nx = self.last_nx;
        let ny = self.last_ny;

        if nx == 0 || ny == 0 || heatmap_area.width == 0 || heatmap_area.height == 0 {
            self.data_cursor.hide();
            self.crosshair_data_pos = None;
            return;
        }

        if col >= heatmap_area.x
            && col < heatmap_area.x + heatmap_area.width
            && row >= heatmap_area.y
            && row < heatmap_area.y + heatmap_area.height
        {
            let frac_x = (col - heatmap_area.x) as f64 / heatmap_area.width as f64;
            let frac_y = (row - heatmap_area.y) as f64 / heatmap_area.height as f64;
            let ix = (frac_x * nx as f64).min((nx - 1) as f64) as usize;
            let iy = (frac_y * ny as f64).min((ny - 1) as f64) as usize;
            let idx = iy * nx + ix;
            if idx < self.last_data.len() {
                let val = self.last_data[idx];
                let extent = self.last_extent;
                if self.crosshair_mode && extent > 0.0 {
                    let data_x = -extent + frac_x * 2.0 * extent;
                    let data_y = extent - frac_y * 2.0 * extent;
                    self.crosshair_data_pos = Some((data_x, data_y));
                    self.data_cursor.show(
                        col,
                        row,
                        format!("({data_x:.3}, {data_y:.3})  \u{03c1}={val:.4e}"),
                    );
                } else {
                    self.crosshair_data_pos = None;
                    self.data_cursor
                        .show(col, row, format!("[{ix},{iy}] = {val:.4e}"));
                }
            } else {
                self.data_cursor.hide();
                self.crosshair_data_pos = None;
            }
        } else {
            self.data_cursor.hide();
            self.crosshair_data_pos = None;
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::VizCycleColormap => {
                // Colormap cycling is now handled globally by ColormapState
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
        theme: &Theme,
        colormap_name: &str,
        data_provider: &dyn DataProvider,
    ) {
        let Some((data, nx, ny)) = data_provider.density_projection(self.axis) else {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "No density data yet — start a simulation on ",
                        Style::default().fg(theme.dim()),
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

        let axis_names = [
            "ρ(y,z)  [x-projection]",
            "ρ(x,z)  [y-projection]",
            "ρ(x,y)  [z-projection]",
        ];
        let title = axis_names[self.axis.min(2)];
        let norm_tag = self.norm_mode.tag();
        let contour_tag = self.contour_mode.tag();
        let accel_tag = if self.show_acceleration {
            " [accel]"
        } else {
            ""
        };
        let stream_tag = if self.show_streamlines {
            " [stream]"
        } else {
            ""
        };
        let inset_tag = if self.show_inset && self.zoom > 1.0 {
            " [inset]"
        } else {
            ""
        };
        let full_title =
            format!(" {title}{norm_tag}{contour_tag}{accel_tag}{stream_tag}{inset_tag} ");

        let [main_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            let a = area;
            [a, Rect::new(a.x, a.y, 0, 0)]
        };

        // Optional marginal strip layout
        let (heatmap_area, top_marginal, right_marginal) =
            if self.show_marginals && main_area.height >= 12 && main_area.width >= 40 {
                let [top_strip, center] =
                    Layout::vertical([Constraint::Length(5), Constraint::Min(5)]).areas(main_area);
                let [hm, right_strip] =
                    Layout::horizontal([Constraint::Min(10), Constraint::Length(20)]).areas(center);
                (hm, Some(top_strip), Some(right_strip))
            } else {
                (main_area, None, None)
            };

        // Apply zoom by extracting a sub-region of the data
        let (view_data, vnx, vny) = zoom::crop_data(&data, nx, ny, self.zoom);

        // Use physical spatial extent for aspect ratio if available
        let state = data_provider.current_state();
        let extent = state.map(|s| s.spatial_extent).unwrap_or(vnx as f64 / 2.0);

        // Build GridData and render via ratatui-plt Heatmap
        let grid = flat_to_grid_data(&view_data, vnx, vny, (-extent, extent), (-extent, extent));

        let (vmin, vmax) = grid.value_bounds();
        let plt_theme = theme.clone();

        match self.contour_mode {
            ContourMode::HeatmapOnly | ContourMode::HeatmapContour => {
                let mut hm = Heatmap::new(grid.clone())
                    .colormap(
                        ratatui_plt::colormap::get_colormap(colormap_name)
                            .unwrap_or_else(|| Box::new(ratatui_plt::colormap::Viridis)),
                    )
                    .title(full_title.clone())
                    .aspect_ratio(AspectRatio::Equal)
                    .show_colorbar(true)
                    .theme(plt_theme.clone());

                hm = self.norm_mode.apply_to_heatmap(hm, vmin, vmax);

                frame.render_widget(&hm, heatmap_area);

                // Overlay contours if requested
                if self.contour_mode == ContourMode::HeatmapContour {
                    let contour = ContourPlot::new(grid)
                        .levels(10)
                        .aspect_ratio(AspectRatio::Equal)
                        .theme(theme.clone());
                    frame.render_widget(&contour, heatmap_area);
                }
            }
            ContourMode::FilledContour => {
                let contour = ContourPlot::new(grid)
                    .levels(10)
                    .filled(true)
                    .colormap(
                        ratatui_plt::colormap::get_colormap(colormap_name)
                            .unwrap_or_else(|| Box::new(ratatui_plt::colormap::Viridis)),
                    )
                    .title(full_title.clone())
                    .aspect_ratio(AspectRatio::Equal)
                    .theme(plt_theme);
                let contour = self.norm_mode.apply_to_contour(contour, vmin, vmax);

                frame.render_widget(&contour, heatmap_area);
            }
        }

        // Acceleration vector field overlay
        if self.show_acceleration || self.show_streamlines {
            if let Some(accel) = state.and_then(|s| s.acceleration_xy.as_ref()) {
                if !accel.is_empty() {
                    let vfd = VectorFieldData::new(accel.clone());
                    if self.show_acceleration {
                        let vf = VectorField::new(vfd.clone())
                            .color(Color::White)
                            .color_by_magnitude(true)
                            .x_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                            .y_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                            .spines(Spines::all(false))
                            .theme(theme.clone());
                        frame.render_widget(&vf, heatmap_area);
                    }
                    if self.show_streamlines {
                        let sp = StreamPlot::new(vfd)
                            .color(Color::Cyan)
                            .color_by_magnitude(true)
                            .density(2)
                            .x_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                            .y_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                            .spines(Spines::all(false))
                            .theme(theme.clone());
                        frame.render_widget(&sp, heatmap_area);
                    }
                }
            }
        }

        // Inset overview when zoomed
        if self.show_inset && self.zoom > 1.0 && !data.is_empty() {
            let inset = InsetAxes::new(0.70, 0.02, 0.28, 0.28)
                .border(true)
                .border_color(theme.accent);
            let full_grid = flat_to_grid_data(&data, nx, ny, (-extent, extent), (-extent, extent));
            inset.render_with(heatmap_area, frame.buffer_mut(), |rect, buf| {
                let mini_hm = Heatmap::new(full_grid.clone())
                    .show_colorbar(false)
                    .theme(theme.clone());
                (&mini_hm).render(rect, buf);
            });
        }

        // Marginal density strips
        if let Some(top_area) = top_marginal {
            // Column sums → horizontal profile (x marginal)
            let col_sums: Vec<(f64, f64)> = (0..vnx)
                .map(|ix| {
                    let sum: f64 = (0..vny)
                        .map(|iy| view_data.get(iy * vnx + ix).copied().unwrap_or(0.0))
                        .sum();
                    let x = -extent + (ix as f64 + 0.5) * 2.0 * extent / vnx as f64;
                    (x, sum)
                })
                .collect();
            let plt_theme = theme.clone();
            let plot = LinePlot::new()
                .series(
                    Series::new("ρ(x)")
                        .data(col_sums)
                        .color(theme.chart_color(0)),
                )
                .x_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                .y_axis(PltAxis::new())
                .theme(plt_theme);
            frame.render_widget(&plot, top_area);
        }
        if let Some(right_area) = right_marginal {
            // Row sums → vertical profile (y marginal)
            let row_sums: Vec<(f64, f64)> = (0..vny)
                .map(|iy| {
                    let sum: f64 = (0..vnx)
                        .map(|ix| view_data.get(iy * vnx + ix).copied().unwrap_or(0.0))
                        .sum();
                    let y = -extent + (iy as f64 + 0.5) * 2.0 * extent / vny as f64;
                    (y, sum)
                })
                .collect();
            let plt_theme = theme.clone();
            let plot = LinePlot::new()
                .series(
                    Series::new("ρ(y)")
                        .data(row_sums)
                        .color(theme.chart_color(1)),
                )
                .x_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                .y_axis(PltAxis::new())
                .theme(plt_theme);
            frame.render_widget(&plot, right_area);
        }

        // Store heatmap area and extent for mouse cursor lookups
        self.last_heatmap_area = heatmap_area;
        self.last_extent = extent;
        if let Some(s) = state
            && (s.step != self.last_state_step || vnx != self.last_nx || vny != self.last_ny)
        {
            self.last_data = view_data.into_owned();
            self.last_state_step = s.step;
            self.last_nx = vnx;
            self.last_ny = vny;
        }

        if self.show_info && info_area.width > 0 {
            let mut status_parts = Vec::new();
            if self.crosshair_mode {
                status_parts.push("crosshair".to_string());
            }
            if self.show_acceleration {
                status_parts.push("accel".to_string());
            }
            if self.show_streamlines {
                status_parts.push("stream".to_string());
            }
            if self.show_inset && self.zoom > 1.0 {
                status_parts.push("inset".to_string());
            }
            if let Some((idx, total)) = data_provider.scrub_position() {
                status_parts.push(format!("SCRUB {}/{total}", idx + 1));
            }
            let status = status_parts.join("  ");
            if !status.is_empty() {
                frame.render_widget(
                    Paragraph::new(status).style(Style::default().fg(theme.dim())),
                    info_area,
                );
            }
        }

        // Data cursor tooltip (drawn last so it's on top)
        self.data_cursor.draw(frame);
    }
}
