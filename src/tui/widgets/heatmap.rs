use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Widget},
};

use crate::{
    colormaps::{lookup, Colormap},
    tui::aspect::AspectCorrection,
    tui::widgets::colorbar::render_colorbar,
};

pub struct HeatmapWidget<'a> {
    pub data: &'a [f64],
    pub nx: usize,
    pub ny: usize,
    pub title: &'a str,
    pub colormap: Colormap,
    pub log_scale: bool,
    pub show_colorbar: bool,
    pub aspect: Option<AspectCorrection>,
    pub x_range: f64,
    pub y_range: f64,
}

impl<'a> HeatmapWidget<'a> {
    pub fn new(data: &'a [f64], nx: usize, ny: usize, title: &'a str) -> Self {
        Self {
            data,
            nx,
            ny,
            title,
            colormap: Colormap::Viridis,
            log_scale: false,
            show_colorbar: true,
            aspect: None,
            x_range: 1.0,
            y_range: 1.0,
        }
    }

    pub fn colormap(mut self, c: Colormap) -> Self { self.colormap = c; self }
    pub fn log_scale(mut self, v: bool) -> Self { self.log_scale = v; self }
    pub fn show_colorbar(mut self, v: bool) -> Self { self.show_colorbar = v; self }
    pub fn aspect(mut self, a: AspectCorrection) -> Self { self.aspect = Some(a); self }
    pub fn x_range(mut self, r: f64) -> Self { self.x_range = r; self }
    pub fn y_range(mut self, r: f64) -> Self { self.y_range = r; self }
}

impl Widget for HeatmapWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(self.title);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0
            || inner.height == 0
            || self.data.is_empty()
            || self.nx == 0
            || self.ny == 0
        {
            return;
        }

        // Split off colorbar column
        let (cb_area, draw_area) = if self.show_colorbar && inner.width > 8 {
            let [hm, cb] = Layout::horizontal([
                Constraint::Min(0),
                Constraint::Length(4),
            ])
            .areas(inner);
            (Some(cb), hm)
        } else {
            (None, inner)
        };

        // Optionally apply aspect-ratio letterbox
        let draw_area = if let Some(ref asp) = self.aspect {
            asp.letterbox(draw_area, self.x_range, self.y_range).rect
        } else {
            draw_area
        };

        // Normalise data
        let (min_val, max_val) = data_range(self.data, self.log_scale);

        // Render using HalfBlock characters (2× vertical resolution)
        // Each terminal row = 2 "half-rows": upper (bg) and lower (fg, "▄")
        let cols = draw_area.width as usize;
        let half_rows = (draw_area.height as usize) * 2;

        for col in 0..cols {
            for row in 0..draw_area.height as usize {
                let hrow_upper = row * 2;
                let hrow_lower = row * 2 + 1;

                let data_col = (col * self.nx) / cols.max(1);
                let data_col = data_col.min(self.nx - 1);

                let dc_upper = (hrow_upper * self.ny) / half_rows.max(1);
                let dc_upper = dc_upper.min(self.ny - 1);

                let dc_lower = (hrow_lower * self.ny) / half_rows.max(1);
                let dc_lower = dc_lower.min(self.ny - 1);

                let idx_upper = dc_upper * self.nx + data_col;
                let idx_lower = dc_lower * self.nx + data_col;

                let v_upper = self.data.get(idx_upper).copied().unwrap_or(0.0);
                let v_lower = self.data.get(idx_lower).copied().unwrap_or(0.0);

                let t_upper = normalize(v_upper, min_val, max_val, self.log_scale);
                let t_lower = normalize(v_lower, min_val, max_val, self.log_scale);

                let c_upper = lookup(self.colormap, t_upper);
                let c_lower = lookup(self.colormap, t_lower);

                let x = draw_area.x + col as u16;
                let y = draw_area.y + row as u16;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol("▄");
                    cell.set_fg(c_lower);
                    cell.set_bg(c_upper);
                }
            }
        }

        if let Some(cb) = cb_area {
            render_colorbar(buf, cb, min_val, max_val, self.colormap, self.log_scale);
        }
    }
}

fn data_range(data: &[f64], log_scale: bool) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &v in data {
        if log_scale && v <= 0.0 { continue; }
        if v < min { min = v; }
        if v > max { max = v; }
    }
    if min == f64::INFINITY { min = 0.0; }
    if max == f64::NEG_INFINITY { max = 1.0; }
    if min == max { max = min + 1.0; }
    (min, max)
}

fn normalize(v: f64, min: f64, max: f64, log_scale: bool) -> f64 {
    if log_scale {
        if v <= 0.0 || min <= 0.0 { return 0.0; }
        let lv = v.ln();
        let lmin = min.ln();
        let lmax = max.ln();
        if lmax == lmin { return 0.0; }
        ((lv - lmin) / (lmax - lmin)).clamp(0.0, 1.0)
    } else {
        let range = max - min;
        if range == 0.0 { return 0.0; }
        ((v - min) / range).clamp(0.0, 1.0)
    }
}
