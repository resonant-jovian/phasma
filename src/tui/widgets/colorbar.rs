use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
};

use crate::colormaps::{lookup, Colormap};

/// Render a vertical colorbar into the given area.
pub fn render_colorbar(
    buf: &mut Buffer,
    area: Rect,
    min_val: f64,
    max_val: f64,
    colormap: Colormap,
    log_scale: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let h = area.height as usize;
    let half_rows = h * 2;

    for row in 0..h {
        let hrow_upper = row * 2;
        let hrow_lower = row * 2 + 1;

        // Map rows top-to-bottom → t goes from 1 → 0
        let t_upper = 1.0 - hrow_upper as f64 / (half_rows - 1).max(1) as f64;
        let t_lower = 1.0 - hrow_lower as f64 / (half_rows - 1).max(1) as f64;

        let c_upper = lookup(colormap, t_upper.clamp(0.0, 1.0));
        let c_lower = lookup(colormap, t_lower.clamp(0.0, 1.0));

        let x = area.x;
        let y = area.y + row as u16;
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol("▄");
            cell.set_fg(c_lower);
            cell.set_bg(c_upper);
        }
        // Label column (2 chars wide)
        if area.width >= 3 {
            let label = if row == 0 {
                fmt_value(max_val, log_scale)
            } else if row == h - 1 {
                fmt_value(min_val, log_scale)
            } else if row == h / 2 {
                let mid = if log_scale && min_val > 0.0 && max_val > 0.0 {
                    (min_val.ln() + max_val.ln() / 2.0).exp()
                } else {
                    (min_val + max_val) / 2.0
                };
                fmt_value(mid, log_scale)
            } else {
                "".to_string()
            };

            let lx = area.x + 1;
            let ly = area.y + row as u16;
            for (i, ch) in label.chars().take((area.width - 1) as usize).enumerate() {
                if let Some(cell) = buf.cell_mut((lx + i as u16, ly)) {
                    cell.set_symbol(ch.encode_utf8(&mut [0u8; 4]));
                    cell.set_style(Style::default());
                }
            }
        }
    }
}

fn fmt_value(v: f64, log_scale: bool) -> String {
    if log_scale && v > 0.0 {
        format!("{:.0}", v.log10())
    } else if v.abs() >= 1000.0 || (v.abs() < 0.01 && v != 0.0) {
        format!("{v:.1e}")
    } else {
        format!("{v:.2}")
    }
}
