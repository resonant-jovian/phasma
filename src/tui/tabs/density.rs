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
    tui::{action::Action, aspect::AspectCorrection, widgets::heatmap::HeatmapWidget},
};

pub struct DensityTab {
    axis: usize, // 0=yz, 1=xz, 2=xy (default)
    log_scale: bool,
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
}

impl Default for DensityTab {
    fn default() -> Self {
        Self {
            axis: 2,
            log_scale: false,
            colormap: Colormap::Viridis,
            show_info: true,
            zoom: 1.0,
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
            KeyCode::Char('x') => {
                self.axis = 0;
                None
            }
            KeyCode::Char('y') => {
                self.axis = 1;
                None
            }
            KeyCode::Char('z') => {
                self.axis = 2;
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
            KeyCode::Char('r') | KeyCode::Char('0') => {
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
        let full_title = format!(" {title}{log_tag} ");

        let [heatmap_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            let a = area;
            [a, Rect::new(a.x, a.y, 0, 0)]
        };

        // Apply zoom by extracting a sub-region of the data
        let (view_data, vnx, vny) = crop_data(&data, nx, ny, self.zoom);

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
            let scrub_hint = if let Some((idx, total)) = data_provider.scrub_position() {
                format!("  SCRUB {}/{total} [\\] live", idx + 1)
            } else {
                String::new()
            };
            let axis_hint = format!(
                "[x/y/z] axis  [l] log  [c] cmap  [+/-/scroll] zoom  [r/0] reset  [i] hide{scrub_hint}"
            );
            frame.render_widget(
                Paragraph::new(axis_hint).style(Style::default().fg(theme.dim)),
                info_area,
            );
        }
    }
}

/// Crop data to a centered sub-region defined by zoom level.
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
