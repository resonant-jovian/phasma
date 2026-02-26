use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Block, Widget},
};

/// Custom widget that renders a 2D scalar field as a character-art heatmap.
///
/// Used for both the density projection ρ(x,y) and the phase-space slice f(x,vx).
/// Scales the data grid to the available terminal area via nearest-neighbour sampling,
/// then maps each value to a 6-step character gradient with a blue→cyan→yellow→white
/// colour ramp.
pub struct DensityMap<'a> {
    /// Flat row-major grid of values (length = nx * ny).
    data: &'a [f64],
    nx: usize,
    ny: usize,
    title: &'a str,
}

impl<'a> DensityMap<'a> {
    pub fn new(data: &'a [f64], nx: usize, ny: usize, title: &'a str) -> Self {
        Self { data, nx, ny, title }
    }
}

/// ASCII / Unicode density gradient (sparse → dense).
const GRADIENT: [&str; 6] = [" ", "·", "░", "▒", "▓", "█"];

impl Widget for DensityMap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(self.title);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 || self.data.is_empty() || self.nx == 0 || self.ny == 0 {
            return;
        }

        // Find min / max for normalisation (avoid division by zero)
        let min = self.data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = (max - min).max(1e-30);

        let w = inner.width as usize;
        let h = inner.height as usize;

        for row in 0..h {
            for col in 0..w {
                // Nearest-neighbour sampling from data grid to terminal cell
                let data_col = (col * self.nx / w).min(self.nx - 1);
                let data_row = (row * self.ny / h).min(self.ny - 1);
                let idx = data_row * self.nx + data_col;

                let val = self.data.get(idx).copied().unwrap_or(0.0);
                let normalized = ((val - min) / range).clamp(0.0, 1.0);

                let char_idx = ((normalized * 5.0) as usize).min(5);
                let sym = GRADIENT[char_idx];
                let color = lerp_color(normalized);

                let x = inner.x + col as u16;
                let y = inner.y + row as u16;
                if x < inner.x + inner.width && y < inner.y + inner.height {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_symbol(sym);
                        cell.set_fg(color);
                    }
                }
            }
        }
    }
}

/// Maps a normalised value t ∈ [0, 1] to a colour on the ramp:
///   0.00 → dark blue  (0,   0,  128)
///   0.33 → cyan       (0, 255,  255)
///   0.67 → yellow     (255, 255,   0)
///   1.00 → white      (255, 255, 255)
fn lerp_color(t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.33 {
        let s = t / 0.33;
        (0_u8, (255.0 * s) as u8, (128.0 + 127.0 * s) as u8)
    } else if t < 0.67 {
        let s = (t - 0.33) / 0.34;
        ((255.0 * s) as u8, 255_u8, (255.0 * (1.0 - s)) as u8)
    } else {
        let s = (t - 0.67) / 0.33;
        (255_u8, 255_u8, (255.0 * s) as u8)
    };
    Color::Rgb(r, g, b)
}
