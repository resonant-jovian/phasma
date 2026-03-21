use std::io::Write;
use std::path::Path;

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

/// Export a ZIP bundle with structured subdirectories matching the spec:
///
/// ```text
/// phasma_export/
///   config/
///     run.toml          (if available)
///   report/
///     report.md
///   diagnostics/
///     diagnostics.csv
///     conservation.csv
///   performance/
///     performance.csv
///   snapshots/
///     final_state.json
///     density.npy
///     density.vtk
///   screenshots/
///     screenshot.txt
///   README.txt
/// ```
pub fn export_zip(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
    stem: &str,
) -> Result<String, String> {
    // Create a temp directory for individual exports
    let tmp = dir.join(".phasma_zip_staging");
    std::fs::create_dir_all(&tmp).map_err(|e| format!("create staging dir: {e}"))?;

    // Create structured subdirectories
    let dirs = [
        "config",
        "report",
        "diagnostics",
        "performance",
        "snapshots",
        "screenshots",
        "scripts",
    ];
    for d in &dirs {
        let _ = std::fs::create_dir_all(tmp.join(d));
    }

    // Export into structured locations — run independent exports in parallel
    rayon::scope(|s| {
        let tmp = &tmp;
        s.spawn(|_| {
            let _ = super::csv::export_csv(&tmp.join("diagnostics"), diagnostics, "diagnostics");
        });
        s.spawn(|_| {
            let _ =
                super::json::export_json(&tmp.join("snapshots"), diagnostics, state, "final_state");
        });
        s.spawn(|_| {
            let _ = super::npy::export_npy(&tmp.join("snapshots"), state, "density");
        });
        s.spawn(|_| {
            let _ =
                super::report::export_markdown(&tmp.join("report"), diagnostics, state, "report");
        });
        s.spawn(|_| {
            let _ = super::screenshot::export_screenshot(
                &tmp.join("screenshots"),
                diagnostics,
                state,
                "screenshot",
            );
        });
        s.spawn(|_| {
            let _ = super::parquet::export_parquet(
                &tmp.join("diagnostics"),
                diagnostics,
                state,
                "diagnostics",
            );
        });
        s.spawn(|_| {
            let _ = super::vtk::export_vtk(&tmp.join("snapshots"), state, "density");
        });
        s.spawn(|_| {
            export_conservation_csv(&tmp.join("diagnostics"), diagnostics);
        });
        s.spawn(|_| {
            export_performance_csv(&tmp.join("performance"), state);
        });
        s.spawn(|_| {
            write_load_python(&tmp.join("scripts"));
            write_load_julia(&tmp.join("scripts"));
        });
    });

    // Generate README.txt manifest
    write_readme(&tmp);

    // Package everything into the zip
    let zip_path = dir.join(format!("{stem}.zip"));
    let file = std::fs::File::create(&zip_path).map_err(|e| format!("create zip: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    add_dir_to_zip(&mut zip, &tmp, &tmp, options)?;

    zip.finish().map_err(|e| format!("zip finish: {e}"))?;

    // Clean up staging directory
    let _ = std::fs::remove_dir_all(&tmp);

    Ok(zip_path.display().to_string())
}

fn export_conservation_csv(dir: &Path, diagnostics: &DiagnosticsStore) {
    let energy_drift = diagnostics.energy_drift_series();
    let mass_drift = diagnostics.mass_drift_series();
    let c2_drift = diagnostics.c2_drift_series();

    let path = dir.join("conservation.csv");
    let mut out = String::from("time,energy_drift,mass_drift,casimir_drift\n");

    // All three drift series share the same time base from iter_chart_data(),
    // so index alignment is correct — use direct indexing instead of O(N) find.
    for (i, &(t, de)) in energy_drift.iter().enumerate() {
        let dm = mass_drift.get(i).map(|(_, v)| *v).unwrap_or(0.0);
        let dc = c2_drift.get(i).map(|(_, v)| *v).unwrap_or(0.0);
        out.push_str(&format!("{t},{de},{dm},{dc}\n"));
    }

    let _ = std::fs::write(path, out);
}

fn export_performance_csv(dir: &Path, state: Option<&SimState>) {
    let path = dir.join("performance.csv");
    let mut out = String::from("step,sim_time,step_wall_ms\n");

    if let Some(s) = state {
        out.push_str(&format!("{},{},{:.3}\n", s.step, s.t, s.step_wall_ms));
    }

    let _ = std::fs::write(path, out);
}

fn write_load_python(dir: &Path) {
    let script = r#""""Load caustic export archive into analysis-ready data structures."""
import numpy as np
import pandas as pd
from pathlib import Path


def load_export(zip_path_or_dir):
    """Returns a dict with all data from a caustic export."""
    p = Path(zip_path_or_dir)
    result = {}

    config_path = p / "config" / "run.toml"
    if config_path.exists():
        result["config"] = config_path.read_text()

    diag_csv = p / "diagnostics" / "diagnostics.csv"
    if diag_csv.exists():
        result["diagnostics"] = pd.read_csv(diag_csv)

    diag_parquet = p / "diagnostics" / "diagnostics.parquet"
    if diag_parquet.exists():
        result["diagnostics"] = pd.read_parquet(diag_parquet)

    conservation = p / "diagnostics" / "conservation.csv"
    if conservation.exists():
        result["conservation"] = pd.read_csv(conservation)

    perf = p / "performance" / "performance.csv"
    if perf.exists():
        result["performance"] = pd.read_csv(perf)

    result["snapshots"] = {
        f.stem: np.load(f) for f in sorted((p / "snapshots").glob("*.npy"))
    }

    return result


if __name__ == "__main__":
    import sys
    path = sys.argv[1] if len(sys.argv) > 1 else "."
    data = load_export(path)
    print(f"Loaded: {list(data.keys())}")
    if "diagnostics" in data:
        print(f"  Diagnostics: {len(data['diagnostics'])} rows")
    if "snapshots" in data:
        print(f"  Snapshots: {list(data['snapshots'].keys())}")
"#;
    let _ = std::fs::write(dir.join("load_python.py"), script);
}

fn write_load_julia(dir: &Path) {
    let script = r#""""
Load caustic export archive into Julia data structures.
Requires: CSV, DataFrames, NPZ (or NPY) packages.
"""
module LoadExport

using CSV, DataFrames

function load_export(dir::String)
    result = Dict{String, Any}()

    config_path = joinpath(dir, "config", "run.toml")
    if isfile(config_path)
        result["config"] = read(config_path, String)
    end

    diag_csv = joinpath(dir, "diagnostics", "diagnostics.csv")
    if isfile(diag_csv)
        result["diagnostics"] = CSV.read(diag_csv, DataFrame)
    end

    conservation = joinpath(dir, "diagnostics", "conservation.csv")
    if isfile(conservation)
        result["conservation"] = CSV.read(conservation, DataFrame)
    end

    perf = joinpath(dir, "performance", "performance.csv")
    if isfile(perf)
        result["performance"] = CSV.read(perf, DataFrame)
    end

    return result
end

end  # module
"#;
    let _ = std::fs::write(dir.join("load_julia.jl"), script);
}

fn write_readme(dir: &Path) {
    let readme = "\
PHASMA Export Archive
=====================

This archive contains simulation results from the PHASMA TUI.

Directory structure:
  config/         - Simulation configuration (TOML)
  report/         - Markdown summary report
  diagnostics/    - Time series data (CSV, Parquet)
    diagnostics.csv     - Full diagnostics time series
    conservation.csv    - Energy/mass/Casimir drift
  performance/    - Performance metrics
    performance.csv     - Wall time per step
  snapshots/      - Simulation state
    final_state.json    - Last state as JSON
    density.npy         - Density field (NumPy format)
    density.vtk         - Density field (VTK format)
  screenshots/    - Terminal screenshots
  scripts/        - Analysis helpers
    load_python.py      - Python loader (pandas + numpy)
    load_julia.jl       - Julia loader (CSV + DataFrames)

Generated by PHASMA
";
    let _ = std::fs::write(dir.join("README.txt"), readme);
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
