use ratatui::style::Color;
use ratatui_plt::prelude::{
    AsinhNorm, Axis as PltAxis, Bounds, GridData, LinearNorm, LogNorm, Normalize, PowerNorm,
    ReferenceLine, Scale, Series, SymLogNorm, Theme,
};

// ── Theme extension trait ──────────────────────────────────────────────

/// Semantic color accessors that phasma needs but ratatui-plt's Theme does not
/// directly expose with phasma's naming conventions.
pub trait PhasmaThemeExt {
    fn warn(&self) -> Color;
    fn error(&self) -> Color;
    fn ok(&self) -> Color;
    fn dim(&self) -> Color;
    fn border_color(&self) -> Color;
    fn chart_color(&self, idx: usize) -> Color;
}

impl PhasmaThemeExt for Theme {
    fn warn(&self) -> Color {
        self.annotation_color
    }
    fn error(&self) -> Color {
        self.negative_color
    }
    fn ok(&self) -> Color {
        self.positive_color
    }
    fn dim(&self) -> Color {
        self.disabled_color
    }
    fn border_color(&self) -> Color {
        self.axis_color
    }
    fn chart_color(&self, idx: usize) -> Color {
        self.color_cycle.at(idx)
    }
}

// ── Theme state ────────────────────────────────────────────────────────

/// Cycling/serializable wrapper around ratatui-plt named theme presets.
pub struct ThemeState {
    name: String,
    index: usize,
}

impl ThemeState {
    pub fn new(name: &str) -> Self {
        let names = ratatui_plt::theme::theme_names();
        let index = names.iter().position(|&n| n == name).unwrap_or(0);
        Self {
            name: names[index].to_string(),
            index,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn theme(&self) -> Theme {
        Theme::from_name(&self.name).unwrap_or_default()
    }

    pub fn next(&mut self) {
        let names = ratatui_plt::theme::theme_names();
        self.index = (self.index + 1) % names.len();
        self.name = names[self.index].to_string();
    }

    pub fn prev(&mut self) {
        let names = ratatui_plt::theme::theme_names();
        self.index = (self.index + names.len() - 1) % names.len();
        self.name = names[self.index].to_string();
    }

    pub fn set(&mut self, name: &str) {
        let names = ratatui_plt::theme::theme_names();
        if let Some(idx) = names.iter().position(|&n| n == name) {
            self.index = idx;
            self.name = name.to_string();
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

impl Default for ThemeState {
    fn default() -> Self {
        Self::new("dark")
    }
}

// ── Colormap state ─────────────────────────────────────────────────────

/// Cycling/serializable wrapper around the ratatui-plt colormap registry.
pub struct ColormapState {
    name: String,
    index: usize,
}

impl ColormapState {
    pub fn new(name: &str) -> Self {
        let names = ratatui_plt::colormap::colormap_names();
        let index = names.iter().position(|&n| n == name).unwrap_or(0);
        Self {
            name: names[index].to_string(),
            index,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get a boxed colormap instance for the current selection.
    pub fn get(&self) -> Box<dyn ratatui_plt::colormap::Colormap> {
        ratatui_plt::colormap::get_colormap(&self.name)
            .unwrap_or_else(|| Box::new(ratatui_plt::colormap::Viridis))
    }

    pub fn next(&mut self) {
        let names = ratatui_plt::colormap::colormap_names();
        self.index = (self.index + 1) % names.len();
        self.name = names[self.index].to_string();
    }

    pub fn prev(&mut self) {
        let names = ratatui_plt::colormap::colormap_names();
        self.index = (self.index + names.len() - 1) % names.len();
        self.name = names[self.index].to_string();
    }

    pub fn set(&mut self, name: &str) {
        let names = ratatui_plt::colormap::colormap_names();
        if let Some(idx) = names.iter().position(|&n| n == name) {
            self.index = idx;
            self.name = name.to_string();
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn all_names() -> &'static [&'static str] {
        ratatui_plt::colormap::colormap_names()
    }
}

impl Default for ColormapState {
    fn default() -> Self {
        Self::new("viridis")
    }
}

// ── Grid data helpers ──────────────────────────────────────────────────

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

// ── Axis helpers ───────────────────────────────────────────────────────

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
