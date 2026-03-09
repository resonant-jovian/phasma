use std::io::Write;
use std::path::Path;

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

/// Export a ZIP bundle by running all other export formats into a temp directory,
/// then packaging every produced file into a single archive.
pub fn export_zip(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
) -> Result<String, String> {
    // Create a temp directory for individual exports
    let tmp = dir.join(".phasma_zip_staging");
    std::fs::create_dir_all(&tmp).map_err(|e| format!("create staging dir: {e}"))?;

    // Run every non-zip export format, collecting successes silently
    let _ = super::csv::export_csv(&tmp, diagnostics);
    let _ = super::json::export_json(&tmp, diagnostics, state);
    let _ = super::npy::export_npy(&tmp, state);
    let _ = super::report::export_markdown(&tmp, diagnostics, state);
    let _ = super::screenshot::export_screenshot(&tmp, diagnostics, state);
    let _ = super::parquet::export_parquet(&tmp, diagnostics, state);
    let _ = super::vtk::export_vtk(&tmp, state);
    let _ = super::animation::export_animation_frames(&tmp, diagnostics, state);

    // Package everything into the zip
    let zip_path = dir.join("phasma_export.zip");
    let file = std::fs::File::create(&zip_path).map_err(|e| format!("create zip: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    add_dir_to_zip(&mut zip, &tmp, &tmp, options)?;

    zip.finish().map_err(|e| format!("zip finish: {e}"))?;

    // Clean up staging directory
    let _ = std::fs::remove_dir_all(&tmp);

    Ok(zip_path.display().to_string())
}

/// Recursively add all files under `base` to the zip, using paths relative to `root`.
fn add_dir_to_zip(
    zip: &mut ZipWriter<std::fs::File>,
    current: &Path,
    root: &Path,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let entries = std::fs::read_dir(current).map_err(|e| format!("read dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let name = rel.to_string_lossy();

        if path.is_dir() {
            add_dir_to_zip(zip, &path, root, options)?;
        } else {
            let data = std::fs::read(&path).map_err(|e| format!("read {name}: {e}"))?;
            zip.start_file(name.to_string(), options)
                .map_err(|e| format!("zip entry {name}: {e}"))?;
            zip.write_all(&data)
                .map_err(|e| format!("write {name}: {e}"))?;
        }
    }
    Ok(())
}
