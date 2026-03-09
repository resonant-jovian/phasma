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
    data::live::LiveDataProvider,
    themes::ThemeColors,
    tui::{
        action::Action,
        aspect::AspectCorrection,
        widgets::heatmap::HeatmapWidget,
    },
};

pub struct DensityTab {
    axis: usize,       // 0=yz, 1=xz, 2=xy (default)
    log_scale: bool,
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
    pan_x: f32,       // pan offset in fractional units [-1..1]
    pan_y: f32,
}

impl Default for DensityTab {
    fn default() -> Self {
        Self { axis: 2, log_scale: false, colormap: Colormap::Viridis, show_info: true, zoom: 1.0, pan_x: 0.0, pan_y: 0.0 }
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
                self.pan_x = 0.0;
                self.pan_y = 0.0;
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('x') => { self.axis = 0; None }
            KeyCode::Char('y') => { self.axis = 1; None }
            KeyCode::Char('z') => { self.axis = 2; None }
            KeyCode::Char('l') => { self.log_scale = !self.log_scale; None }
            KeyCode::Char('c') => { self.colormap = self.colormap.next(); None }
            KeyCode::Char('i') => { self.show_info = !self.show_info; None }
            KeyCode::Char('r') | KeyCode::Char('0') => {
                self.zoom = 1.0;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                None
            }
            KeyCode::Char('+') => { self.zoom = (self.zoom * 1.25).min(8.0); None }
            KeyCode::Char('-') => { self.zoom = (self.zoom / 1.25).max(0.25); None }
            KeyCode::Left  => { self.pan_x -= 0.1 / self.zoom; None }
            KeyCode::Right => { self.pan_x += 0.1 / self.zoom; None }
            KeyCode::Up    => { self.pan_y -= 0.1 / self.zoom; None }
            KeyCode::Down  => { self.pan_y += 0.1 / self.zoom; None }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::VizCycleColormap => { self.colormap = self.colormap.next(); }
            Action::VizToggleLog => { self.log_scale = !self.log_scale; }
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
        data_provider: &LiveDataProvider,
    ) {
        let effective_cmap = colormap;

        let Some((data, nx, ny)) = data_provider.density_projection(self.axis) else {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("No density data yet — start a simulation on ", Style::default().fg(theme.dim)),
                    Span::styled("[F2]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ])),
                area,
            );
            return;
        };

        let axis_names = ["ρ(y,z)  [x-projection]", "ρ(x,z)  [y-projection]", "ρ(x,y)  [z-projection]"];
        let title = axis_names[self.axis.min(2)];
        let log_tag = if self.log_scale { " [log]" } else { "" };
        let full_title = format!(" {title}{log_tag} ");

        let [heatmap_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            let a = area;
            [a, Rect::new(a.x, a.y, 0, 0)]
        };

        // Apply zoom+pan by extracting a sub-region of the data
        let (view_data, vnx, vny) = crop_data(&data, nx, ny, self.zoom, self.pan_x, self.pan_y);

        let asp = AspectCorrection::default();
        frame.render_widget(
            HeatmapWidget::new(&view_data, vnx, vny, &full_title)
                .colormap(effective_cmap)
                .log_scale(self.log_scale)
                .aspect(asp)
                .x_range(vnx as f64)
                .y_range(vny as f64),
            heatmap_area,
        );

        if self.show_info && info_area.width > 0 {
            let axis_hint = "[x/y/z] axis  [l] log  [c] cmap  [+/-] zoom  [arrows] pan  [r/0] reset  [i] hide";
            frame.render_widget(
                Paragraph::new(axis_hint).style(Style::default().fg(theme.dim)),
                info_area,
            );
        }
    }
}

/// Crop data to the visible window defined by zoom and pan offsets.
fn crop_data(data: &[f64], nx: usize, ny: usize, zoom: f32, pan_x: f32, pan_y: f32) -> (Vec<f64>, usize, usize) {
    if zoom <= 1.0 && pan_x == 0.0 && pan_y == 0.0 {
        return (data.to_vec(), nx, ny);
    }
    let zoom = zoom.max(1.0); // pan without zoom doesn't crop
    let view_w = (nx as f32 / zoom).ceil() as usize;
    let view_h = (ny as f32 / zoom).ceil() as usize;
    let view_w = view_w.max(1).min(nx);
    let view_h = view_h.max(1).min(ny);

    // Center + pan
    let cx = (nx as f32 / 2.0 + pan_x * nx as f32).clamp(view_w as f32 / 2.0, nx as f32 - view_w as f32 / 2.0);
    let cy = (ny as f32 / 2.0 + pan_y * ny as f32).clamp(view_h as f32 / 2.0, ny as f32 - view_h as f32 / 2.0);

    let x0 = (cx - view_w as f32 / 2.0).max(0.0) as usize;
    let y0 = (cy - view_h as f32 / 2.0).max(0.0) as usize;

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
