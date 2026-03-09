pub mod animation;
pub mod csv;
pub mod json;
pub mod npy;
pub mod parquet;
pub mod report;
pub mod screenshot;
pub mod vtk;
pub mod zip_archive;

use std::path::Path;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

/// Which export format the user selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
    Npy,
    Markdown,
    Zip,
    Screenshot,
    Parquet,
    Vtk,
    Animation,
}

impl ExportFormat {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Npy => "NPY (NumPy)",
            Self::Markdown => "Markdown report",
            Self::Zip => "ZIP bundle",
            Self::Screenshot => "Text snapshot",
            Self::Parquet => "Parquet (Arrow)",
            Self::Vtk => "VTK (ParaView)",
            Self::Animation => "Animation frames",
        }
    }
}

/// Export diagnostics time series to the given directory.
pub fn export_diagnostics(
    dir: &Path,
    format: ExportFormat,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;

    match format {
        ExportFormat::Csv => csv::export_csv(dir, diagnostics),
        ExportFormat::Json => json::export_json(dir, diagnostics, state),
        ExportFormat::Npy => npy::export_npy(dir, state),
        ExportFormat::Markdown => report::export_markdown(dir, diagnostics, state),
        ExportFormat::Zip => zip_archive::export_zip(dir, diagnostics, state),
        ExportFormat::Screenshot => screenshot::export_screenshot(dir, diagnostics, state),
        ExportFormat::Parquet => parquet::export_parquet(dir, diagnostics, state),
        ExportFormat::Vtk => vtk::export_vtk(dir, state),
        ExportFormat::Animation => animation::export_animation_frames(dir, diagnostics, state),
    }
}
