use std::path::Path;

use ratatui_plt::export::{
    ExportOptions, buffer_to_ansi, buffer_to_png, buffer_to_svg, buffer_to_text, render_to_buffer,
    save_svg,
};
use ratatui_plt::prelude::{Axis as PltAxis, Bounds, Heatmap, LinePlot, Scale, Series, Theme};

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;
use crate::tui::plt_bridge::{PhasmaThemeExt, flat_to_grid_data};

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
    theme: &Theme,
    stem: &str,
) -> Result<String, String> {
    export_energy(dir, diagnostics, theme, stem, ExportResolution::default())
}

pub fn export_energy(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &Theme,
    stem: &str,
    resolution: ExportResolution,
) -> Result<String, String> {
    let energy = diagnostics.total_energy.iter_chart_data();
    let kinetic = diagnostics.kinetic_energy.iter_chart_data();
    let potential = diagnostics.potential_energy.iter_chart_data();

    if energy.is_empty() {
        return Err("No energy data available".to_string());
    }

    let plt_theme = theme.clone();
    let plot = LinePlot::new()
        .series(
            Series::new("E_tot")
                .data(energy)
                .color(theme.chart_color(0)),
        )
        .series(Series::new("T").data(kinetic).color(theme.chart_color(1)))
        .series(Series::new("W").data(potential).color(theme.chart_color(2)))
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
    theme: &Theme,
    colormap_name: &str,
    stem: &str,
) -> Result<String, String> {
    export_density(
        dir,
        state,
        theme,
        colormap_name,
        stem,
        ExportResolution::default(),
    )
}

pub fn export_density(
    dir: &Path,
    state: &SimState,
    theme: &Theme,
    colormap_name: &str,
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

    let plt_theme = theme.clone();
    let colormap = ratatui_plt::colormap::get_colormap(colormap_name)
        .unwrap_or_else(|| Box::new(ratatui_plt::colormap::Viridis));
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
    theme: &Theme,
    stem: &str,
) -> Result<String, String> {
    export_conservation(dir, diagnostics, theme, stem, ExportResolution::default())
}

pub fn export_conservation(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &Theme,
    stem: &str,
    resolution: ExportResolution,
) -> Result<String, String> {
    let energy_drift = diagnostics.energy_drift_series();
    let mass_drift = diagnostics.mass_drift_series();
    let c2_drift = diagnostics.c2_drift_series();

    if energy_drift.is_empty() {
        return Err("No drift data available".to_string());
    }

    let plt_theme = theme.clone();
    let plot = LinePlot::new()
        .series(
            Series::new("dE/E")
                .data(energy_drift)
                .color(theme.chart_color(0)),
        )
        .series(
            Series::new("dM/M")
                .data(mass_drift)
                .color(theme.chart_color(1)),
        )
        .series(
            Series::new("dC2/C2")
                .data(c2_drift)
                .color(theme.chart_color(2)),
        )
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
    theme: &Theme,
    colormap_name: &str,
    stem: &str,
) -> Result<Vec<String>, String> {
    export_charts_batch_with_resolution(
        dir,
        diagnostics,
        state,
        theme,
        colormap_name,
        stem,
        ExportResolution::default(),
    )
}

pub fn export_charts_batch_with_resolution(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
    theme: &Theme,
    colormap_name: &str,
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
        if let Ok(path) = export_density(&charts_dir, s, theme, colormap_name, stem, resolution) {
            exported.push(path);
        }
    }

    if exported.is_empty() {
        Err("No charts could be exported".to_string())
    } else {
        Ok(exported)
    }
}

// ── ANSI / Text export ────────────────────────────────────────────────

/// Export energy chart as ANSI-colored terminal text (for CI logs, HPC environments).
pub fn export_energy_ansi(diagnostics: &DiagnosticsStore, theme: &Theme) -> Result<String, String> {
    let energy = diagnostics.total_energy.iter_chart_data();
    if energy.is_empty() {
        return Err("No energy data".to_string());
    }
    let kinetic = diagnostics.kinetic_energy.iter_chart_data();
    let potential = diagnostics.potential_energy.iter_chart_data();

    let plt_theme = theme.clone();
    let plot = LinePlot::new()
        .series(
            Series::new("E_tot")
                .data(energy)
                .color(theme.chart_color(0)),
        )
        .series(Series::new("T").data(kinetic).color(theme.chart_color(1)))
        .series(Series::new("W").data(potential).color(theme.chart_color(2)))
        .x_axis(PltAxis::new().label("t"))
        .y_axis(PltAxis::new().label("Energy"))
        .title("Energy Evolution")
        .show_legend(true)
        .theme(plt_theme);

    let buf = render_to_buffer(&plot, DEFAULT_EXPORT_WIDTH, DEFAULT_EXPORT_HEIGHT);
    Ok(buffer_to_ansi(&buf))
}

/// Export conservation chart as plain text (no colors).
pub fn export_conservation_text(
    diagnostics: &DiagnosticsStore,
    theme: &Theme,
) -> Result<String, String> {
    let energy_drift = diagnostics.energy_drift_series();
    if energy_drift.is_empty() {
        return Err("No drift data".to_string());
    }
    let mass_drift = diagnostics.mass_drift_series();
    let c2_drift = diagnostics.c2_drift_series();

    let plt_theme = theme.clone();
    let plot = LinePlot::new()
        .series(
            Series::new("dE/E")
                .data(energy_drift)
                .color(theme.chart_color(0)),
        )
        .series(
            Series::new("dM/M")
                .data(mass_drift)
                .color(theme.chart_color(1)),
        )
        .series(
            Series::new("dC2/C2")
                .data(c2_drift)
                .color(theme.chart_color(2)),
        )
        .x_axis(PltAxis::new().label("t"))
        .y_axis(PltAxis::new().label("Relative drift"))
        .title("Conservation Diagnostics")
        .show_legend(true)
        .theme(plt_theme);

    let buf = render_to_buffer(&plot, DEFAULT_EXPORT_WIDTH, DEFAULT_EXPORT_HEIGHT);
    Ok(buffer_to_text(&buf))
}
