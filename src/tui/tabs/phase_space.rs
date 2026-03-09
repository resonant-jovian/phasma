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

pub struct PhaseSpaceTab {
    /// Which spatial dimension for x-axis (0=x₁, 1=x₂, 2=x₃)
    dim_x: usize,
    /// Which velocity dimension for y-axis (0=v₁, 1=v₂, 2=v₃)
    dim_v: usize,
    /// Slice positions for fixed dimensions (dim_index, fractional_position 0..1)
    fixed_positions: [f64; 6],
    log_scale: bool,
    colormap: Colormap,
    show_info: bool,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
}

impl Default for PhaseSpaceTab {
    fn default() -> Self {
        Self {
            dim_x: 0,
            dim_v: 0,
            fixed_positions: [0.5; 6], // centre of each dimension
            log_scale: false,
            colormap: Colormap::Viridis,
            show_info: true,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
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
                self.pan_x = 0.0;
                self.pan_y = 0.0;
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            // Select spatial dimension with 1-3
            KeyCode::Char('1') => { self.dim_x = 0; None }
            KeyCode::Char('2') => { self.dim_x = 1; None }
            KeyCode::Char('3') => { self.dim_x = 2; None }
            // Select velocity dimension with 4-6
            KeyCode::Char('4') => { self.dim_v = 0; None }
            KeyCode::Char('5') => { self.dim_v = 1; None }
            KeyCode::Char('6') => { self.dim_v = 2; None }
            // Adjust slice position for non-displayed dims
            KeyCode::Char('{') => {
                self.nudge_slice(-0.05);
                None
            }
            KeyCode::Char('}') => {
                self.nudge_slice(0.05);
                None
            }
            KeyCode::Char('l') => { self.log_scale = !self.log_scale; None }
            KeyCode::Char('c') => { self.colormap = self.colormap.next(); None }
            KeyCode::Char('i') => { self.show_info = !self.show_info; None }
            KeyCode::Char('+') => { self.zoom = (self.zoom * 1.25).min(8.0); None }
            KeyCode::Char('-') => { self.zoom = (self.zoom / 1.25).max(0.25); None }
            KeyCode::Char('r') | KeyCode::Char('0') => {
                self.zoom = 1.0;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                None
            }
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

        // Currently uses the default x₁-v₁ slice from SimState
        let Some((data, nx, nv)) = data_provider.phase_slice(self.dim_x, self.dim_v, &[]) else {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("No phase-space data yet — start a simulation on ", Style::default().fg(theme.dim)),
                    Span::styled("[F2]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ])),
                area,
            );
            return;
        };

        let dim_labels = ["x", "y", "z"];
        let vel_labels = ["vx", "vy", "vz"];
        let title = format!(
            " f({}, {}) {}",
            dim_labels[self.dim_x],
            vel_labels[self.dim_v],
            if self.log_scale { "[log]" } else { "" },
        );

        let [heatmap_area, info_area] = if self.show_info && area.height > 4 {
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area)
        } else {
            [area, Rect::new(area.x, area.y, 0, 0)]
        };

        let (view_data, vnx, vnv) = crop_data(&data, nx, nv, self.zoom, self.pan_x, self.pan_y);

        let asp = AspectCorrection::default();
        frame.render_widget(
            HeatmapWidget::new(&view_data, vnx, vnv, &title)
                .colormap(effective_cmap)
                .log_scale(self.log_scale)
                .aspect(asp)
                .x_range(vnx as f64)
                .y_range(vnv as f64),
            heatmap_area,
        );

        if self.show_info && info_area.width > 0 {
            let hint = format!(
                "[1-3] x={}  [4-6] v={}  [{{/}}] slice  [+/-] zoom  [arrows] pan  [r/0] reset  [l] log  [i] hide",
                dim_labels[self.dim_x], vel_labels[self.dim_v],
            );
            frame.render_widget(
                Paragraph::new(hint).style(Style::default().fg(theme.dim)),
                info_area,
            );
        }
    }

    fn nudge_slice(&mut self, delta: f64) {
        // Nudge the first fixed dimension (not dim_x or dim_v in spatial set)
        for i in 0..6 {
            if i == self.dim_x || i == (self.dim_v + 3) {
                continue;
            }
            self.fixed_positions[i] = (self.fixed_positions[i] + delta).clamp(0.0, 1.0);
            return;
        }
    }
}

fn crop_data(data: &[f64], nx: usize, ny: usize, zoom: f32, pan_x: f32, pan_y: f32) -> (Vec<f64>, usize, usize) {
    if zoom <= 1.0 && pan_x == 0.0 && pan_y == 0.0 {
        return (data.to_vec(), nx, ny);
    }
    let zoom = zoom.max(1.0);
    let view_w = (nx as f32 / zoom).ceil() as usize;
    let view_h = (ny as f32 / zoom).ceil() as usize;
    let view_w = view_w.max(1).min(nx);
    let view_h = view_h.max(1).min(ny);

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
