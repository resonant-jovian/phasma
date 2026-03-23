use ratatui::style::Color;
use ratatui_plt::prelude::{
    AsinhNorm, Axis as PltAxis, Bounds, ColorCycle, GridData, LinearNorm, LogNorm, Normalize,
    PowerNorm, RefLineDash, ReferenceLine, Scale, Series, SymLogNorm, Theme,
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

/// Build a ratatui-plt `Axis` with auto bounds and an optional label.
pub fn make_auto_axis(label: Option<&str>) -> PltAxis {
    let mut ax = PltAxis::new();
    if let Some(l) = label {
        ax = ax.label(l);
    }
    ax
}

/// Build a ratatui-plt `Axis` with symmetric log scale.
pub fn make_symlog_axis(label: Option<&str>, lin_thresh: f64) -> PltAxis {
    let mut ax = PltAxis::new().scale(Scale::SymLog {
        lin_thresh,
        lin_scale: 1.0,
        base: 10.0,
    });
    if let Some(l) = label {
        ax = ax.label(l);
    }
    ax
}

/// Build a `Series` from name, data, and color (reduces boilerplate).
pub fn make_series(name: &str, data: Vec<(f64, f64)>, color: Color) -> Series {
    Series::new(name).data(data).color(color)
}

/// Build a dashed horizontal reference line.
pub fn make_reference_hline(y: f64, color: Color) -> ReferenceLine {
    ReferenceLine::hline_dashed(y, color)
}

/// Build a dashed vertical reference line.
pub fn make_reference_vline(x: f64, color: Color) -> ReferenceLine {
    ReferenceLine::vline_dashed(x, color)
}

// ── Normalization helpers ──────────────────────────────────────────────

/// Heatmap normalization modes available across density and phase-space tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormMode {
    #[default]
    Linear,
    Log,
    Sqrt,
    Asinh,
}

impl NormMode {
    /// Cycle to the next normalization mode.
    pub fn next(self) -> Self {
        match self {
            Self::Linear => Self::Log,
            Self::Log => Self::Sqrt,
            Self::Sqrt => Self::Asinh,
            Self::Asinh => Self::Linear,
        }
    }

    /// Short tag for display in title bars.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Linear => "",
            Self::Log => " [log]",
            Self::Sqrt => " [sqrt]",
            Self::Asinh => " [asinh]",
        }
    }

    /// Apply this normalization mode to a ratatui-plt `Heatmap`.
    /// Falls back to `LinearNorm` when the data range is unsuitable
    /// (e.g., log with non-positive values).
    pub fn apply_to_heatmap(
        self,
        hm: ratatui_plt::prelude::Heatmap,
        vmin: f64,
        vmax: f64,
    ) -> ratatui_plt::prelude::Heatmap {
        match self {
            Self::Linear => hm.norm(LinearNorm::new(vmin, vmax)),
            Self::Log => {
                if vmin > 0.0 {
                    hm.norm(LogNorm::new(vmin, vmax))
                } else {
                    hm.norm(LinearNorm::new(vmin, vmax))
                }
            }
            Self::Sqrt => hm.norm(PowerNorm::new(0.5, vmin, vmax)),
            Self::Asinh => {
                let width = (vmax - vmin).abs() * 0.01;
                let width = if width > 0.0 { width } else { 1.0 };
                hm.norm(AsinhNorm::new(width, vmin, vmax))
            }
        }
    }

    /// Apply this normalization mode to a ratatui-plt `ContourPlot`.
    pub fn apply_to_contour(
        self,
        ct: ratatui_plt::prelude::ContourPlot,
        vmin: f64,
        vmax: f64,
    ) -> ratatui_plt::prelude::ContourPlot {
        match self {
            Self::Linear => ct.norm(LinearNorm::new(vmin, vmax)),
            Self::Log => {
                if vmin > 0.0 {
                    ct.norm(LogNorm::new(vmin, vmax))
                } else {
                    ct.norm(LinearNorm::new(vmin, vmax))
                }
            }
            Self::Sqrt => ct.norm(PowerNorm::new(0.5, vmin, vmax)),
            Self::Asinh => {
                let width = (vmax - vmin).abs() * 0.01;
                let width = if width > 0.0 { width } else { 1.0 };
                ct.norm(AsinhNorm::new(width, vmin, vmax))
            }
        }
    }
}

/// Build a `SymLogNorm` for signed data that spans zero.
pub fn make_symlog_norm(lin_thresh: f64, vmin: f64, vmax: f64) -> SymLogNorm {
    SymLogNorm::new(lin_thresh, vmin, vmax)
}

// ── Color helpers ──────────────────────────────────────────────────────

/// Pick a color from the theme chart palette, cycling automatically.
pub fn cycle_color(idx: usize, theme: &ThemeColors) -> Color {
    theme.chart[idx % theme.chart.len()]
}

// ── Format helpers ─────────────────────────────────────────────────────

/// Format a byte count as a human-readable size string (KB/MB/GB).
pub fn format_size(bytes: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{:.0} B", bytes)
    }
}

/// Format a duration in seconds as a compact string (e.g., "3.2s", "5m03s", "2h15m").
pub fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else if secs < 3600.0 {
        format!("{}m{:02}s", secs as u64 / 60, secs as u64 % 60)
    } else {
        format!("{}h{:02}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
    }
}
