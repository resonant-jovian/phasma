use std::path::Path;

use ratatui_plt::export::{render_to_buffer, buffer_to_svg, ExportOptions};
use ratatui_plt::prelude::{
    Axis as PltAxis, Bounds, Heatmap, LinePlot, Scale, Series,
};

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;
use crate::themes::ThemeColors;
use crate::tui::plt_bridge::{flat_to_grid_data, phasma_cmap_to_plt, phasma_theme_to_plt};
use crate::colormaps::Colormap;

const EXPORT_WIDTH: u16 = 120;
const EXPORT_HEIGHT: u16 = 40;

/// Export an energy evolution chart as SVG.
pub fn export_energy_svg(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &ThemeColors,
    stem: &str,
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

    let buf = render_to_buffer(&plot, EXPORT_WIDTH, EXPORT_HEIGHT);
    let svg = buffer_to_svg(&buf, 14.0);

    let path = dir.join(format!("{stem}_energy.svg"));
    std::fs::write(&path, &svg).map_err(|e| format!("write SVG: {e}"))?;
    Ok(path.display().to_string())
}

/// Export a density heatmap as SVG.
pub fn export_density_svg(
    dir: &Path,
    state: &SimState,
    theme: &ThemeColors,
    cmap: Colormap,
    stem: &str,
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

    let buf = render_to_buffer(&heatmap, EXPORT_WIDTH, EXPORT_HEIGHT);
    let svg = buffer_to_svg(&buf, 14.0);

    let path = dir.join(format!("{stem}_density.svg"));
    std::fs::write(&path, &svg).map_err(|e| format!("write SVG: {e}"))?;
    Ok(path.display().to_string())
}

/// Export conservation diagnostics as SVG.
pub fn export_conservation_svg(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    theme: &ThemeColors,
    stem: &str,
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

    let buf = render_to_buffer(&plot, EXPORT_WIDTH, EXPORT_HEIGHT);
    let svg = buffer_to_svg(&buf, 14.0);

    let path = dir.join(format!("{stem}_conservation.svg"));
    std::fs::write(&path, &svg).map_err(|e| format!("write SVG: {e}"))?;
    Ok(path.display().to_string())
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
    let charts_dir = dir.join("charts");
    std::fs::create_dir_all(&charts_dir).map_err(|e| format!("create charts dir: {e}"))?;

    let mut exported = Vec::new();

    match export_energy_svg(&charts_dir, diagnostics, theme, stem) {
        Ok(path) => exported.push(path),
        Err(_) => {}
    }

    match export_conservation_svg(&charts_dir, diagnostics, theme, stem) {
        Ok(path) => exported.push(path),
        Err(_) => {}
    }

    if let Some(s) = state {
        match export_density_svg(&charts_dir, s, theme, cmap, stem) {
            Ok(path) => exported.push(path),
            Err(_) => {}
        }
    }

    if exported.is_empty() {
        Err("No charts could be exported".to_string())
    } else {
        Ok(exported)
    }
}
