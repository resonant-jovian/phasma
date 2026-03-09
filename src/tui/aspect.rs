use ratatui::layout::Rect;

/// Letterbox result — the sub-rect within the available area that maintains
/// the desired aspect ratio, centred with letter-box bars on the sides/top.
#[derive(Debug, Clone, Copy)]
pub struct LetterboxRect {
    pub rect: Rect,
}

/// Corrects for the fact that terminal cells are typically ~2× taller than
/// wide (cell_aspect_ratio ≈ 0.5 means cell width = 0.5 × cell height).
#[derive(Debug, Clone, Copy)]
pub struct AspectCorrection {
    /// Width/height ratio of a single terminal cell (default 0.5).
    pub cell_aspect_ratio: f64,
}

impl Default for AspectCorrection {
    fn default() -> Self {
        Self {
            cell_aspect_ratio: 0.5,
        }
    }
}

impl AspectCorrection {
    pub fn new(cell_aspect_ratio: f64) -> Self {
        Self { cell_aspect_ratio }
    }

    /// Compute letterbox rect so data_x_range / data_y_range is preserved,
    /// accounting for non-square terminal cells.
    ///
    /// Returns the largest centred sub-rect of `available` that has the
    /// correct aspect ratio.
    pub fn letterbox(
        &self,
        available: Rect,
        data_x_range: f64,
        data_y_range: f64,
    ) -> LetterboxRect {
        if available.width == 0
            || available.height == 0
            || data_x_range == 0.0
            || data_y_range == 0.0
        {
            return LetterboxRect { rect: available };
        }

        // Desired pixel aspect ratio (cols / rows) for the data
        // Each col = cell_aspect_ratio "units wide", each row = 1 "unit tall"
        // So desired cols/rows = (data_x / data_y) / cell_aspect_ratio
        let desired_pixel_ar = (data_x_range / data_y_range) / self.cell_aspect_ratio;

        let avail_ar = available.width as f64 / available.height as f64;

        let (w, h) = if avail_ar > desired_pixel_ar {
            // Pillarbox — constrained by height
            let h = available.height;
            let w = ((h as f64 * desired_pixel_ar).round() as u16).min(available.width);
            (w, h)
        } else {
            // Letterbox — constrained by width
            let w = available.width;
            let h = ((w as f64 / desired_pixel_ar).round() as u16).min(available.height);
            (w, h)
        };

        let x_off = (available.width.saturating_sub(w)) / 2;
        let y_off = (available.height.saturating_sub(h)) / 2;

        LetterboxRect {
            rect: Rect::new(available.x + x_off, available.y + y_off, w, h),
        }
    }
}
