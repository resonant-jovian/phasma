use std::path::Path;

use ratatui_plt::export::{
    ExportOptions, buffer_to_png, buffer_to_svg, render_to_buffer, save_svg,
};
use ratatui_plt::prelude::{Axis as PltAxis, Bounds, Heatmap, LinePlot, Scale, Series};

use crate::colormaps::Colormap;
use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;
use crate::themes::ThemeColors;
use crate::tui::plt_bridge::{flat_to_grid_data, phasma_cmap_to_plt, phasma_theme_to_plt};

/// Default export dimensions (cells).
pub const DEFAULT_EXPORT_WIDTH: u16 = 120;
pub const DEFAULT_EXPORT_HEIGHT: u16 = 40;

/// High-resolution export dimensions.
pub const HIRES_EXPORT_WIDTH: u16 = 200;
pub const HIRES_EXPORT_HEIGHT: u16 = 80;

/// Export resolution preset.
#[derive(Clone, Copy, Debug, Default)]
pub enum ExportResolution {
    #[default]
    Standard,
    HighRes,
    Custom(u16, u16),
}

impl ExportResolution {
    pub fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Standard => (DEFAULT_EXPORT_WIDTH, DEFAULT_EXPORT_HEIGHT),
            Self::HighRes => (HIRES_EXPORT_WIDTH, HIRES_EXPORT_HEIGHT),
            Self::Custom(w, h) => (w, h),
        }
    }
}

/// Export an energy evolution chart as SVG (and optionally PNG).
pub fn export_energy_svg(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &ThemeColors,
    stem: &str,
) -> Result<String, String> {
    export_energy(dir, diagnostics, theme, stem, ExportResolution::default())
}

pub fn export_energy(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &ThemeColors,
    stem: &str,
    resolution: ExportResolution,
) -> Result<String, String> {
    let energy = diagnostics.total_energy.iter_chart_data();
    let kinetic = diagnostics.kinetic_energy.iter_chart_data();
    let potential = diagnostics.potential_energy.iter_chart_data();

    if energy.is_empty() {
        return Err("No energy data available".to_string());
    }

    let plt_theme = phasma_theme_to_plt(theme);
    let plot = LinePlot::new()
        .series(Series::new("E_tot").data(energy).color(theme.chart[0]))
        .series(Series::new("T").data(kinetic).color(theme.chart[1]))
        .series(Series::new("W").data(potential).color(theme.chart[2]))
        .x_axis(PltAxis::new().label("t"))
        .y_axis(PltAxis::new().label("Energy"))
        .title("Energy Evolution")
        .show_legend(true)
        .theme(plt_theme);

    let (w, h) = resolution.dimensions();
    let svg_path = dir.join(format!("{stem}_energy.svg"));
    save_svg(&plot, w, h, &svg_path).map_err(|e| format!("write SVG: {e}"))?;

    // Also export PNG
    let buf = render_to_buffer(&plot, w, h);
    let png_opts = ExportOptions {
        cell_width: 8,
        cell_height: 16,
    };
    if let Ok(png_bytes) = buffer_to_png(&buf, &png_opts) {
        let png_path = dir.join(format!("{stem}_energy.png"));
        let _ = std::fs::write(&png_path, &png_bytes);
    }

    Ok(svg_path.display().to_string())
}

/// Export a density heatmap as SVG (and optionally PNG).
pub fn export_density_svg(
    dir: &Path,
    state: &SimState,
    theme: &ThemeColors,
    cmap: Colormap,
    stem: &str,
) -> Result<String, String> {
    export_density(dir, state, theme, cmap, stem, ExportResolution::default())
}

pub fn export_density(
    dir: &Path,
    state: &SimState,
    theme: &ThemeColors,
    cmap: Colormap,
    stem: &str,
    resolution: ExportResolution,
) -> Result<String, String> {
    if state.density_xy.is_empty() {
        return Err("No density data available".to_string());
    }

    let ext = state.spatial_extent;
    let grid = flat_to_grid_data(
        &state.density_xy,
        state.density_nx,
        state.density_ny,
        (-ext, ext),
        (-ext, ext),
    );

    let plt_theme = phasma_theme_to_plt(theme);
    let colormap = phasma_cmap_to_plt(cmap);
    let heatmap = Heatmap::new(grid)
        .colormap(colormap)
        .title("Density projection (x-y)")
        .show_colorbar(true)
        .theme(plt_theme);

    let (w, h) = resolution.dimensions();
    let svg_path = dir.join(format!("{stem}_density.svg"));
    save_svg(&heatmap, w, h, &svg_path).map_err(|e| format!("write SVG: {e}"))?;

    // Also export PNG
    let buf = render_to_buffer(&heatmap, w, h);
    let png_opts = ExportOptions {
        cell_width: 8,
        cell_height: 16,
    };
    if let Ok(png_bytes) = buffer_to_png(&buf, &png_opts) {
        let png_path = dir.join(format!("{stem}_density.png"));
        let _ = std::fs::write(&png_path, &png_bytes);
    }

    Ok(svg_path.display().to_string())
}

/// Export conservation diagnostics as SVG (and optionally PNG).
pub fn export_conservation_svg(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &ThemeColors,
    stem: &str,
) -> Result<String, String> {
    export_conservation(dir, diagnostics, theme, stem, ExportResolution::default())
}

pub fn export_conservation(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &ThemeColors,
    stem: &str,
    resolution: ExportResolution,
) -> Result<String, String> {
    let energy_drift = diagnostics.energy_drift_series();
    let mass_drift = diagnostics.mass_drift_series();
    let c2_drift = diagnostics.c2_drift_series();

    if energy_drift.is_empty() {
        return Err("No drift data available".to_string());
    }

    let plt_theme = phasma_theme_to_plt(theme);
    let plot = LinePlot::new()
        .series(Series::new("dE/E").data(energy_drift).color(theme.chart[0]))
        .series(Series::new("dM/M").data(mass_drift).color(theme.chart[1]))
        .series(Series::new("dC2/C2").data(c2_drift).color(theme.chart[2]))
        .x_axis(PltAxis::new().label("t"))
        .y_axis(PltAxis::new().label("Relative drift"))
        .title("Conservation Diagnostics")
        .show_legend(true)
        .theme(plt_theme);

    let (w, h) = resolution.dimensions();
    let svg_path = dir.join(format!("{stem}_conservation.svg"));
    save_svg(&plot, w, h, &svg_path).map_err(|e| format!("write SVG: {e}"))?;

    // Also export PNG
    let buf = render_to_buffer(&plot, w, h);
    let png_opts = ExportOptions {
        cell_width: 8,
        cell_height: 16,
    };
    if let Ok(png_bytes) = buffer_to_png(&buf, &png_opts) {
        let png_path = dir.join(format!("{stem}_conservation.png"));
        let _ = std::fs::write(&png_path, &png_bytes);
    }

    Ok(svg_path.display().to_string())
}

/// Export a batch of charts (energy + density + conservation) to a directory.
pub fn export_charts_batch(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
    theme: &ThemeColors,
    cmap: Colormap,
    stem: &str,
) -> Result<Vec<String>, String> {
    export_charts_batch_with_resolution(
        dir,
        diagnostics,
        state,
        theme,
        cmap,
        stem,
        ExportResolution::default(),
    )
}

pub fn export_charts_batch_with_resolution(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
    theme: &ThemeColors,
    cmap: Colormap,
    stem: &str,
    resolution: ExportResolution,
) -> Result<Vec<String>, String> {
    let charts_dir = dir.join("charts");
    std::fs::create_dir_all(&charts_dir).map_err(|e| format!("create charts dir: {e}"))?;

    let mut exported = Vec::new();

    if let Ok(path) = export_energy(&charts_dir, diagnostics, theme, stem, resolution) {
        exported.push(path);
    }

    if let Ok(path) = export_conservation(&charts_dir, diagnostics, theme, stem, resolution) {
        exported.push(path);
    }

    if let Some(s) = state {
        if let Ok(path) = export_density(&charts_dir, s, theme, cmap, stem, resolution) {
            exported.push(path);
        }
    }

    if exported.is_empty() {
        Err("No charts could be exported".to_string())
    } else {
        Ok(exported)
    }
}
