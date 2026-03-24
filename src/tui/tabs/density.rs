use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use ratatui_plt::prelude::{
    AspectRatio, Axis as PltAxis, Bounds, ContourPlot, GridData, Heatmap, LinePlot, Series,
};

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    themes::ThemeColors,
    tui::widgets::data_cursor::DataCursor,
    tui::{
        action::Action,
        plt_bridge::{NormMode, flat_to_grid_data, phasma_cmap_to_plt, phasma_theme_to_plt},
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
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
    contour_mode: ContourMode,
    data_cursor: DataCursor,
    last_heatmap_area: Rect,
    last_data: Vec<f64>,
    last_nx: usize,
    last_ny: usize,
    last_state_step: u64,
}

impl Default for DensityTab {
    fn default() -> Self {
        Self {
            axis: 2,
            norm_mode: NormMode::default(),
            show_marginals: false,
            colormap: Colormap::Viridis,
            show_info: true,
            zoom: 1.0,
            contour_mode: ContourMode::default(),
            data_cursor: DataCursor::default(),
            last_heatmap_area: Rect::default(),
            last_data: Vec::new(),
            last_nx: 0,
            last_ny: 0,
            last_state_step: u64::MAX,
        }
    }
}

impl DensityTab {
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
            KeyCode::Char('r') => {
                self.zoom = 1.0;
                None
            }
            KeyCode::Char('0') => {
                // Auto-scale: fit data to view (currently same as reset)
                self.zoom = 1.0;
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
            _ => None,
        }
    }

    pub fn handle_mouse_move(&mut self, col: u16, row: u16) {
        let heatmap_area = self.last_heatmap_area;
        let nx = self.last_nx;
        let ny = self.last_ny;

        if nx == 0 || ny == 0 || heatmap_area.width == 0 || heatmap_area.height == 0 {
            self.data_cursor.hide();
            return;
        }

        if col >= heatmap_area.x
            && col < heatmap_area.x + heatmap_area.width
            && row >= heatmap_area.y
            && row < heatmap_area.y + heatmap_area.height
        {
            let dx = (col - heatmap_area.x) as f64 / heatmap_area.width as f64;
            let dy = (row - heatmap_area.y) as f64 / heatmap_area.height as f64;
            let ix = (dx * nx as f64).min((nx - 1) as f64) as usize;
            let iy = (dy * ny as f64).min((ny - 1) as f64) as usize;
            let idx = iy * nx + ix;
            if idx < self.last_data.len() {
                let val = self.last_data[idx];
                self.data_cursor
                    .show(col, row, format!("[{ix},{iy}] = {val:.4e}"));
            } else {
                self.data_cursor.hide();
            }
        } else {
            self.data_cursor.hide();
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

        let Some((data, nx, ny)) = data_provider.density_projection(self.axis) else {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "No density data yet — start a simulation on ",
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

        let axis_names = [
            "ρ(y,z)  [x-projection]",
            "ρ(x,z)  [y-projection]",
            "ρ(x,y)  [z-projection]",
        ];
        let title = axis_names[self.axis.min(2)];
        let norm_tag = self.norm_mode.tag();
        let contour_tag = self.contour_mode.tag();
        let full_title = format!(" {title}{norm_tag}{contour_tag} ");

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
        let (view_data, vnx, vny) = crop_data(&data, nx, ny, self.zoom);

        // Use physical spatial extent for aspect ratio if available
        let state = data_provider.current_state();
        let extent = state.map(|s| s.spatial_extent).unwrap_or(vnx as f64 / 2.0);

        // Build GridData and render via ratatui-plt Heatmap
        let grid = flat_to_grid_data(&view_data, vnx, vny, (-extent, extent), (-extent, extent));

        let (vmin, vmax) = grid.value_bounds();
        let plt_theme = phasma_theme_to_plt(theme);

        match self.contour_mode {
            ContourMode::HeatmapOnly | ContourMode::HeatmapContour => {
                let mut hm = Heatmap::new(grid.clone())
                    .colormap(phasma_cmap_to_plt(effective_cmap))
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
                        .theme(phasma_theme_to_plt(theme));
                    frame.render_widget(&contour, heatmap_area);
                }
            }
            ContourMode::FilledContour => {
                let contour = ContourPlot::new(grid)
                    .levels(10)
                    .filled(true)
                    .colormap(phasma_cmap_to_plt(effective_cmap))
                    .title(full_title.clone())
                    .aspect_ratio(AspectRatio::Equal)
                    .theme(plt_theme);
                let contour = self.norm_mode.apply_to_contour(contour, vmin, vmax);

                frame.render_widget(&contour, heatmap_area);
            }
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
            let plt_theme = phasma_theme_to_plt(theme);
            let plot = LinePlot::new()
                .series(Series::new("ρ(x)").data(col_sums).color(theme.chart[0]))
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
            let plt_theme = phasma_theme_to_plt(theme);
            let plot = LinePlot::new()
                .series(Series::new("ρ(y)").data(row_sums).color(theme.chart[1]))
                .x_axis(PltAxis::new().bounds(Bounds::Manual(-extent, extent)))
                .y_axis(PltAxis::new())
                .theme(plt_theme);
            frame.render_widget(&plot, right_area);
        }

        // Store heatmap area for mouse cursor lookups
        self.last_heatmap_area = heatmap_area;
        if let Some(s) = state
            && (s.step != self.last_state_step || vnx != self.last_nx || vny != self.last_ny)
        {
            self.last_data = view_data.into_owned();
            self.last_state_step = s.step;
            self.last_nx = vnx;
            self.last_ny = vny;
        }

        if self.show_info && info_area.width > 0 {
            let scrub_hint = if let Some((idx, total)) = data_provider.scrub_position() {
                format!("  SCRUB {}/{total} [\\] live", idx + 1)
            } else {
                String::new()
            };
            let axis_hint = format!(
                "[1/2/3] axis  [l] norm  [Shift+c] cmap  [+/-/scroll] zoom  [r/0] reset  [n] contour  [i] hide{scrub_hint}"
            );
            frame.render_widget(
                Paragraph::new(axis_hint).style(Style::default().fg(theme.dim)),
                info_area,
            );
        }

        // Data cursor tooltip (drawn last so it's on top)
        self.data_cursor.draw(frame);
    }
}

/// Crop data to a centered sub-region defined by zoom level.
/// Returns `Cow::Borrowed` at zoom ≤ 1.0 (zero-copy), `Cow::Owned` when zoomed.
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
