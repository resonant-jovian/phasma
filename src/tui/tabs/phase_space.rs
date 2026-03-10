use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    themes::ThemeColors,
    tui::{
        action::Action,
        aspect::AspectCorrection,
        widgets::{data_cursor::DataCursor, heatmap::HeatmapWidget},
    },
};

pub struct PhaseSpaceTab {
    /// Which spatial dimension for x-axis (0=x₁, 1=x₂, 2=x₃)
    dim_x: usize,
    /// Which velocity dimension for y-axis (0=v₁, 1=v₂, 2=v₃)
    dim_v: usize,
    log_scale: bool,
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
    /// Slice position offset in normalised coordinates (-1.0 to 1.0)
    slice_offset: f64,
    /// When true, use physical extents for aspect ratio; when false, fill available area
    physical_aspect: bool,
    data_cursor: DataCursor,
    last_heatmap_area: Rect,
    last_data: Vec<f64>,
    last_nx: usize,
    last_ny: usize,
}

impl Default for PhaseSpaceTab {
    fn default() -> Self {
        Self {
            dim_x: 0,
            dim_v: 0,
            log_scale: false,
            colormap: Colormap::Viridis,
            show_info: true,
            zoom: 1.0,
            slice_offset: 0.0,
            physical_aspect: false,
            data_cursor: Default::default(),
            last_heatmap_area: Rect::default(),
            last_data: Vec::new(),
            last_nx: 0,
            last_ny: 0,
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
                self.log_scale = !self.log_scale;
                None
            }
            KeyCode::Char('c') => {
                self.colormap = self.colormap.next();
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
            KeyCode::Char('{') => {
                self.slice_offset = (self.slice_offset - 0.1).max(-1.0);
                None
            }
            KeyCode::Char('}') => {
                self.slice_offset = (self.slice_offset + 0.1).min(1.0);
                None
            }
            KeyCode::Char('p') => {
                self.physical_aspect = !self.physical_aspect;
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

        let Some((data, nx, nv)) = data_provider.phase_slice(self.dim_x, self.dim_v, &[]) else {
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
        let slice_info = if self.slice_offset.abs() > 0.01 {
            format!(" slice={:+.1}", self.slice_offset)
        } else {
            String::new()
        };
        let title = format!(
            " f({}, {}) {}{}",
            dim_labels[self.dim_x],
            vel_labels[self.dim_v],
            if self.log_scale { "[log]" } else { "" },
            slice_info,
        );

        let [heatmap_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            [area, Rect::new(area.x, area.y, 0, 0)]
        };

        let (view_data, vnx, vnv) = crop_data(&data, nx, nv, self.zoom);

        // Use physical extents for aspect ratio when enabled
        let state = data_provider.current_state();
        let cfg = data_provider.config();
        let cell_ar = cfg.map(|c| c.appearance.cell_aspect_ratio).unwrap_or(0.5);
        let asp = AspectCorrection::new(cell_ar);

        let mut widget = HeatmapWidget::new(&view_data, vnx, vnv, &title)
            .colormap(effective_cmap)
            .log_scale(self.log_scale)
            .aspect(asp);

        if self.physical_aspect {
            let x_extent = state.map(|s| s.spatial_extent * 2.0).unwrap_or(vnx as f64);
            let v_extent = cfg
                .map(|c| c.domain.velocity_extent * 2.0)
                .unwrap_or(vnv as f64);
            widget = widget.x_range(x_extent).y_range(v_extent);
        }

        frame.render_widget(widget, heatmap_area);

        // Cache data for mouse cursor lookups
        self.last_heatmap_area = heatmap_area;
        self.last_data = view_data;
        self.last_nx = vnx;
        self.last_ny = vnv;

        if self.show_info && info_area.width > 0 {
            let scrub_hint = if let Some((idx, total)) = data_provider.scrub_position() {
                format!("  SCRUB {}/{total}", idx + 1)
            } else {
                String::new()
            };
            let hint = format!(
                "[1-3] x={}  [4-6] v={}  [+/-/scroll] zoom  [r/0] reset  [l] log  [{{/}}] slice  [p] aspect  [i] hide{scrub_hint}",
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

fn crop_data(data: &[f64], nx: usize, ny: usize, zoom: f32) -> (Vec<f64>, usize, usize) {
    if zoom <= 1.0 {
        return (data.to_vec(), nx, ny);
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
    (out, view_w, view_h)
}
