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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn square_data_matching_area() {
        // 100×50 area, 10×10 data, cell_ar=0.5
        // desired_pixel_ar = (10/10)/0.5 = 2.0
        // avail_ar = 100/50 = 2.0 → exact match
        let ac = AspectCorrection::default();
        let available = Rect::new(0, 0, 100, 50);
        let result = ac.letterbox(available, 10.0, 10.0);
        assert_eq!(result.rect.width, 100);
        assert_eq!(result.rect.height, 50);
    }

    #[test]
    fn wide_data_tall_area() {
        // Wide data in tall area → pillarbox (width-constrained)
        let ac = AspectCorrection::new(0.5);
        let available = Rect::new(0, 0, 40, 100);
        let result = ac.letterbox(available, 10.0, 10.0);
        // desired_pixel_ar = (10/10)/0.5 = 2.0
        // avail_ar = 40/100 = 0.4 < 2.0 → letterbox (constrained by width)
        // h = (40/2.0).round() = 20, capped at 100
        assert_eq!(result.rect.width, 40);
        assert!(result.rect.height <= available.height);
    }

    #[test]
    fn tall_data_wide_area() {
        // Tall data (y >> x) in wide area → letterbox (height-constrained when data is narrow)
        let ac = AspectCorrection::new(0.5);
        let available = Rect::new(0, 0, 200, 20);
        let result = ac.letterbox(available, 1.0, 10.0);
        // desired_pixel_ar = (1/10)/0.5 = 0.2
        // avail_ar = 200/20 = 10.0 > 0.2 → pillarbox (constrained by height)
        assert_eq!(result.rect.height, 20);
        assert!(result.rect.width <= available.width);
    }

    #[test]
    fn zero_width_area() {
        let ac = AspectCorrection::default();
        let available = Rect::new(0, 0, 0, 50);
        let result = ac.letterbox(available, 10.0, 10.0);
        assert_eq!(result.rect, available);
    }

    #[test]
    fn zero_data_range() {
        let ac = AspectCorrection::default();
        let available = Rect::new(0, 0, 100, 50);
        let result = ac.letterbox(available, 0.0, 10.0);
        assert_eq!(result.rect, available);
    }

    #[test]
    fn result_contained() {
        let ac = AspectCorrection::new(0.5);
        let available = Rect::new(5, 10, 80, 40);
        let result = ac.letterbox(available, 3.0, 7.0);
        assert!(result.rect.x >= available.x);
        assert!(result.rect.y >= available.y);
        assert!(result.rect.x + result.rect.width <= available.x + available.width);
        assert!(result.rect.y + result.rect.height <= available.y + available.height);
    }

    #[test]
    fn cell_aspect_doubles_width() {
        // With cell_aspect_ratio=0.5, square data needs ~2× cols vs rows
        let ac = AspectCorrection::new(0.5);
        let available = Rect::new(0, 0, 200, 200);
        let result = ac.letterbox(available, 10.0, 10.0);
        // desired_pixel_ar = (10/10)/0.5 = 2.0 → w should be ~2× h
        let ratio = result.rect.width as f64 / result.rect.height as f64;
        assert!((ratio - 2.0).abs() < 0.1, "ratio was {ratio}");
    }
}
