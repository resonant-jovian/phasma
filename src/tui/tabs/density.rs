use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    themes::ThemeColors,
    tui::widgets::data_cursor::DataCursor,
    tui::{action::Action, aspect::AspectCorrection, widgets::heatmap::HeatmapWidget},
};

pub struct DensityTab {
    axis: usize, // 0=yz, 1=xz, 2=xy (default)
    log_scale: bool,
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
    show_contours: bool,
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
            log_scale: false,
            colormap: Colormap::Viridis,
            show_info: true,
            zoom: 1.0,
            show_contours: false,
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
                self.log_scale = !self.log_scale;
                None
            }
            KeyCode::Char('i') => {
                self.show_info = !self.show_info;
                None
            }
            KeyCode::Char('n') => {
                self.show_contours = !self.show_contours;
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
                self.log_scale = !self.log_scale;
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
        let log_tag = if self.log_scale { " [log]" } else { "" };
        let contour_tag = if self.show_contours { " [contour]" } else { "" };
        let full_title = format!(" {title}{log_tag}{contour_tag} ");

        let [heatmap_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            let a = area;
            [a, Rect::new(a.x, a.y, 0, 0)]
        };

        // Apply zoom by extracting a sub-region of the data
        let (view_data, vnx, vny) = crop_data(&data, nx, ny, self.zoom);

        // Use physical spatial extent for aspect ratio if available
        let state = data_provider.current_state();
        let x_extent = state.map(|s| s.spatial_extent * 2.0).unwrap_or(vnx as f64);
        let y_extent = x_extent; // spatial domain is symmetric

        let cell_ar = data_provider
            .config()
            .map(|c| c.appearance.cell_aspect_ratio)
            .unwrap_or(0.5);
        let asp = AspectCorrection::new(cell_ar);
        frame.render_widget(
            HeatmapWidget::new(&view_data, vnx, vny, &full_title)
                .colormap(effective_cmap)
                .log_scale(self.log_scale)
                .aspect(asp)
                .x_range(x_extent)
                .y_range(y_extent),
            heatmap_area,
        );

        // Compute the actual draw area used by the heatmap (replicate widget logic)
        let hm_inner = Block::bordered().inner(heatmap_area);
        let hm_draw = if hm_inner.width > 8 {
            let [hm, _cb] =
                Layout::horizontal([Constraint::Min(0), Constraint::Length(4)]).areas(hm_inner);
            hm
        } else {
            hm_inner
        };
        let hm_draw = asp.letterbox(hm_draw, x_extent, y_extent).rect;

        // Store data for mouse cursor lookups — only copy when data actually changed
        self.last_heatmap_area = hm_draw;
        if let Some(s) = state
            && (s.step != self.last_state_step || vnx != self.last_nx || vny != self.last_ny)
        {
            self.last_data = view_data.into_owned();
            self.last_state_step = s.step;
            self.last_nx = vnx;
            self.last_ny = vny;
        }

        // Contour overlay (uses cached data)
        if self.show_contours && !self.last_data.is_empty() && vnx > 0 && vny > 0 {
            overlay_contours(frame, hm_draw, &self.last_data, vnx, vny, self.log_scale);
        }

        if self.show_info && info_area.width > 0 {
            let scrub_hint = if let Some((idx, total)) = data_provider.scrub_position() {
                format!("  SCRUB {}/{total} [\\] live", idx + 1)
            } else {
                String::new()
            };
            let axis_hint = format!(
                "[1/2/3] axis  [l] log  [Shift+c] cmap  [+/-/scroll] zoom  [r/0] reset  [n] contour  [i] hide{scrub_hint}"
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

/// Overlay contour markers on the heatmap area.
///
/// Computes 5 evenly spaced contour levels between data min and max. For each
/// cell in the rendered area, maps screen position back to data coordinates and
/// checks whether the data value is within 10% of a level boundary (relative to
/// the spacing between levels). Matching cells get a dim `·` marker.
fn overlay_contours(
    frame: &mut Frame,
    draw_area: Rect,
    data: &[f64],
    nx: usize,
    ny: usize,
    log_scale: bool,
) {
    if draw_area.width == 0 || draw_area.height == 0 {
        return;
    }

    // Compute data range
    let (data_min, data_max) = data_range(data, log_scale);
    if data_max <= data_min {
        return;
    }

    let num_levels = 5usize;
    let levels: Vec<f64> = (1..=num_levels)
        .map(|i| {
            if log_scale && data_min > 0.0 {
                let lmin = data_min.ln();
                let lmax = data_max.ln();
                (lmin + (lmax - lmin) * i as f64 / (num_levels + 1) as f64).exp()
            } else {
                data_min + (data_max - data_min) * i as f64 / (num_levels + 1) as f64
            }
        })
        .collect();

    let level_spacing = if log_scale && data_min > 0.0 {
        let lmin = data_min.ln();
        let lmax = data_max.ln();
        ((lmax - lmin) / (num_levels + 1) as f64).exp() - 1.0
    } else {
        (data_max - data_min) / (num_levels + 1) as f64
    };

    // Threshold: 10% of spacing between levels
    let threshold_frac = 0.10;

    let cols = draw_area.width as usize;
    let rows = draw_area.height as usize;

    let buf = frame.buffer_mut();

    for row in 0..rows {
        for col in 0..cols {
            // Map screen position to data index
            let data_col = (col * nx) / cols.max(1);
            let data_col = data_col.min(nx.saturating_sub(1));
            let data_row = (row * ny) / rows.max(1);
            let data_row = data_row.min(ny.saturating_sub(1));

            let idx = data_row * nx + data_col;
            if idx >= data.len() {
                continue;
            }
            let val = data[idx];

            // Check if this value is near any contour level
            let near_contour = levels.iter().any(|&level| {
                let threshold = if log_scale && val > 0.0 && level > 0.0 {
                    level * level_spacing * threshold_frac
                } else {
                    level_spacing * threshold_frac
                };
                (val - level).abs() < threshold
            });

            if near_contour {
                let x = draw_area.x + col as u16;
                let y = draw_area.y + row as u16;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol("\u{00b7}"); // middle dot ·
                    cell.set_fg(Color::White);
                }
            }
        }
    }
}

/// Compute data range (min, max), handling log scale.
fn data_range(data: &[f64], log_scale: bool) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &v in data {
        if log_scale && v <= 0.0 {
            continue;
        }
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    if min == f64::INFINITY {
        min = 0.0;
    }
    if max == f64::NEG_INFINITY {
        max = 1.0;
    }
    if min == max {
        max = min + 1.0;
    }
    (min, max)
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
