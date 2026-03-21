use ratatui::style::Color;
use ratatui_plt::prelude::{
    Axis as PltAxis, Bounds, ColorCycle, GridData, LinearNorm, LogNorm, Normalize, Scale, Theme,
};

use crate::colormaps::Colormap;
use crate::themes::ThemeColors;

/// Convert a phasma `ThemeColors` to a ratatui-plt `Theme`.
pub fn phasma_theme_to_plt(theme: &ThemeColors) -> Theme {
    Theme {
        background: theme.bg,
        foreground: theme.fg,
        grid_color: theme.dim,
        minor_grid_color: theme.dim,
        axis_color: theme.border,
        color_cycle: ColorCycle::new(theme.chart.to_vec()),
        grid_visible: false,
        grid_pattern: ratatui_plt::prelude::DashPattern::Dotted,
        bold_title: true,
    }
}

/// Wrapper that implements `ratatui_plt::colormap::Colormap` via a boxed inner.
/// This allows passing the result directly to `.colormap()` on widgets.
pub struct PltColormap(Box<dyn ratatui_plt::colormap::Colormap>);

impl ratatui_plt::colormap::Colormap for PltColormap {
    fn color_at(&self, t: f64) -> Color {
        self.0.color_at(t)
    }
    fn name(&self) -> &str {
        self.0.name()
    }
}

/// Convert a phasma `Colormap` enum variant to a ratatui-plt `Colormap`.
pub fn phasma_cmap_to_plt(cmap: Colormap) -> PltColormap {
    use ratatui_plt::colormap::{
        Coolwarm as PltCoolwarm, Greys, Inferno as PltInferno, LinearSegmentedColormap,
        Magma as PltMagma, Plasma as PltPlasma, Viridis as PltViridis,
    };
    PltColormap(match cmap {
        Colormap::Viridis => Box::new(PltViridis),
        Colormap::Inferno => Box::new(PltInferno),
        Colormap::Plasma => Box::new(PltPlasma),
        Colormap::Magma => Box::new(PltMagma),
        Colormap::Grayscale => Box::new(Greys),
        Colormap::Coolwarm => Box::new(PltCoolwarm),
        Colormap::Cubehelix => {
            // Approximate phasma's cubehelix with a LinearSegmentedColormap
            // using the same 9-stop table from colormaps/mod.rs.
            let stops: [(f64, u8, u8, u8); 9] = [
                (0.000, 0, 0, 0),
                (0.125, 22, 17, 42),
                (0.250, 15, 56, 62),
                (0.375, 28, 98, 47),
                (0.500, 87, 117, 58),
                (0.625, 168, 115, 103),
                (0.750, 196, 130, 182),
                (0.875, 199, 180, 238),
                (1.000, 255, 255, 255),
            ];
            let r: Vec<(f64, f64, f64)> = stops
                .iter()
                .map(|&(t, r, _, _)| (t, r as f64 / 255.0, r as f64 / 255.0))
                .collect();
            let g: Vec<(f64, f64, f64)> = stops
                .iter()
                .map(|&(t, _, g, _)| (t, g as f64 / 255.0, g as f64 / 255.0))
                .collect();
            let b: Vec<(f64, f64, f64)> = stops
                .iter()
                .map(|&(t, _, _, b)| (t, b as f64 / 255.0, b as f64 / 255.0))
                .collect();
            Box::new(LinearSegmentedColormap::new("cubehelix", r, g, b))
        }
    })
}

/// Convert flat row-major `&[f64]` data to ratatui-plt `GridData` with coordinate vectors.
///
/// `data` is indexed as `data[row * nx + col]` (row-major). `GridData` expects
/// `values[row][col]` with `x` (length nx) as column coords and `y` (length ny)
/// as row coords.
pub fn flat_to_grid_data(
    data: &[f64],
    nx: usize,
    ny: usize,
    x_range: (f64, f64),
    y_range: (f64, f64),
) -> GridData {
    let x: Vec<f64> = if nx > 1 {
        (0..nx)
            .map(|i| x_range.0 + (x_range.1 - x_range.0) * i as f64 / (nx - 1) as f64)
            .collect()
    } else {
        vec![(x_range.0 + x_range.1) / 2.0]
    };

    let y: Vec<f64> = if ny > 1 {
        (0..ny)
            .map(|j| y_range.0 + (y_range.1 - y_range.0) * j as f64 / (ny - 1) as f64)
            .collect()
    } else {
        vec![(y_range.0 + y_range.1) / 2.0]
    };

    let values: Vec<Vec<f64>> = (0..ny)
        .map(|row| {
            let start = row * nx;
            let end = (start + nx).min(data.len());
            if start < data.len() {
                data[start..end].to_vec()
            } else {
                vec![0.0; nx]
            }
        })
        .collect();

    GridData::new(x, y, values)
}

/// Build a ratatui-plt `Axis` from manual bounds with sensible defaults.
pub fn make_axis(label: Option<&str>, lo: f64, hi: f64) -> PltAxis {
    let mut ax = PltAxis::new().bounds(Bounds::Manual(lo, hi));
    if let Some(l) = label {
        ax = ax.label(l);
    }
    ax
}

/// Build a ratatui-plt `Axis` with log scale.
pub fn make_log_axis(label: Option<&str>, base: f64) -> PltAxis {
    let mut ax = PltAxis::new().scale(Scale::Log(base));
    if let Some(l) = label {
        ax = ax.label(l);
    }
    ax
}
