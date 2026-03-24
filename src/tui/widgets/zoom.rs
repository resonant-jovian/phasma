use std::borrow::Cow;

use crossterm::event::KeyCode;

/// Apply mouse scroll zoom. Negative delta = zoom in, positive = zoom out.
pub fn apply_scroll_zoom(zoom: &mut f32, delta: i32) {
    if delta < 0 {
        *zoom = (*zoom * 1.15).min(8.0);
    } else {
        *zoom = (*zoom / 1.15).max(1.0);
        if *zoom <= 1.01 {
            *zoom = 1.0;
        }
    }
}

/// Handle +/-/r/0 key zoom. Returns true if zoom was modified.
pub fn apply_key_zoom(zoom: &mut f32, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('+') => {
            *zoom = (*zoom * 1.25).min(8.0);
            true
        }
        KeyCode::Char('-') => {
            *zoom = (*zoom / 1.25).max(0.25);
            true
        }
        KeyCode::Char('r') | KeyCode::Char('0') => {
            *zoom = 1.0;
            true
        }
        _ => false,
    }
}

/// Crop 2D data grid to zoomed center region.
/// Returns `Cow::Borrowed` at zoom <= 1.0 (zero-copy), `Cow::Owned` when zoomed.
pub fn crop_data<'a>(
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
