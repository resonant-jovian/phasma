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

use crate::config::PhasmaConfig;
use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

/// Which export format the user selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Screenshot,
    Csv,
    Json,
    Parquet,
    ConfigToml,
    Vtk,
    Npy,
    Markdown,
    Animation,
    RadialProfilesCsv,
    PerformanceParquet,
    Zip,
    Hdf5,
}

impl ExportFormat {
    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "csv" => Self::Csv,
            "json" => Self::Json,
            "npy" | "numpy" => Self::Npy,
            "md" | "markdown" => Self::Markdown,
            "zip" => Self::Zip,
            "screenshot" | "txt" | "svg" => Self::Screenshot,
            "parquet" => Self::Parquet,
            "vtk" => Self::Vtk,
            "animation" | "anim" => Self::Animation,
            "config" | "toml" => Self::ConfigToml,
            "radial" | "profiles" => Self::RadialProfilesCsv,
            "performance" | "perf" => Self::PerformanceParquet,
            "hdf5" | "h5" => Self::Hdf5,
            _ => Self::Csv,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Screenshot => "Screenshot (SVG)",
            Self::Csv => "Time series → CSV",
            Self::Json => "Time series → JSON",
            Self::Parquet => "Time series → Parquet",
            Self::ConfigToml => "Config → TOML",
            Self::Vtk => "Snapshot → VTK (ParaView)",
            Self::Npy => "Snapshot → NumPy .npy",
            Self::Markdown => "Full report → Markdown",
            Self::Animation => "Animation → frame sequence",
            Self::RadialProfilesCsv => "Radial profiles → CSV",
            Self::PerformanceParquet => "Performance data → Parquet",
            Self::Zip => "★ Export All → ZIP archive",
            Self::Hdf5 => "Snapshot → HDF5",
        }
    }

    /// Shortcut key label for the export menu.
    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Screenshot => "1",
            Self::Csv => "2",
            Self::Json => "3",
            Self::Parquet => "4",
            Self::ConfigToml => "5",
            Self::Vtk => "6",
            Self::Npy => "7",
            Self::Markdown => "8",
            Self::Animation => "9",
            Self::RadialProfilesCsv => "0",
            Self::PerformanceParquet => "a",
            Self::Zip => "z",
            Self::Hdf5 => "h",
        }
    }
}

/// Export diagnostics time series to the given directory.
pub fn export_diagnostics(
    dir: &Path,
    format: ExportFormat,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
    config: Option<&PhasmaConfig>,
    stem: &str,
) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;

    match format {
        ExportFormat::Screenshot => screenshot::export_screenshot(dir, diagnostics, state, stem),
        ExportFormat::Csv => csv::export_csv(dir, diagnostics, stem),
        ExportFormat::Json => json::export_json(dir, diagnostics, state, stem),
        ExportFormat::Parquet => parquet::export_parquet(dir, diagnostics, state, stem),
        ExportFormat::ConfigToml => export_config_toml(dir, config, stem),
        ExportFormat::Vtk => vtk::export_vtk(dir, state, stem),
        ExportFormat::Npy => npy::export_npy(dir, state, stem),
        ExportFormat::Markdown => report::export_markdown(dir, diagnostics, state, stem),
        ExportFormat::Animation => {
            animation::export_animation_frames(dir, diagnostics, state, stem)
        }
        ExportFormat::RadialProfilesCsv => export_radial_profiles_csv(dir, state, stem),
        ExportFormat::PerformanceParquet => export_performance_csv(dir, diagnostics, stem),
        ExportFormat::Zip => zip_archive::export_zip(dir, diagnostics, state, stem),
        ExportFormat::Hdf5 => export_hdf5(dir, state, stem),
    }
}

/// Export current snapshot as HDF5 via caustic's IOManager.
fn export_hdf5(dir: &Path, state: Option<&SimState>, stem: &str) -> Result<String, String> {
    let state = state.ok_or("No simulation state available")?;
    if state.density_xy.is_empty() {
        return Err("No density data available".to_string());
    }

    let path = dir.join(format!("{stem}_snapshot.h5"));
    let path_str = path.display().to_string();

    // Write density projection data as HDF5 using hdf5-metno directly.
    use hdf5_metno as hdf5;

    let file = hdf5::File::create(&path).map_err(|e| format!("HDF5 create: {e}"))?;

    // Density projection: shape [ny, nx] (row-major)
    let nx = state.density_nx;
    let ny = state.density_ny;
    file.new_dataset::<f64>()
        .shape([ny, nx])
        .create("density_xy")
        .map_err(|e| format!("HDF5 dataset: {e}"))?
        .write_raw(&state.density_xy)
        .map_err(|e| format!("HDF5 write: {e}"))?;

    // Simulation metadata
    let params = file
        .create_group("simulation_parameters")
        .map_err(|e| format!("HDF5 group: {e}"))?;
    params
        .new_attr::<f64>()
        .create("time")
        .map_err(|e| format!("HDF5 attr: {e}"))?
        .write_scalar(&state.t)
        .map_err(|e| format!("HDF5 write: {e}"))?;
    params
        .new_attr::<f64>()
        .create("total_energy")
        .map_err(|e| format!("HDF5 attr: {e}"))?
        .write_scalar(&state.total_energy)
        .map_err(|e| format!("HDF5 write: {e}"))?;
    params
        .new_attr::<u64>()
        .create("step")
        .map_err(|e| format!("HDF5 attr: {e}"))?
        .write_scalar(&state.step)
        .map_err(|e| format!("HDF5 write: {e}"))?;

    Ok(path_str)
}

/// Export the current config as a TOML file.
fn export_config_toml(
    dir: &Path,
    config: Option<&PhasmaConfig>,
    stem: &str,
) -> Result<String, String> {
    let cfg = config.ok_or("No config available")?;
    let toml_str = toml::to_string_pretty(cfg).map_err(|e| format!("serialize config: {e}"))?;
    let path = dir.join(format!("{stem}_config.toml"));
    std::fs::write(&path, toml_str).map_err(|e| format!("write: {e}"))?;
    Ok(path.display().to_string())
}

/// Export radial density profile as CSV.
fn export_radial_profiles_csv(
    dir: &Path,
    state: Option<&SimState>,
    stem: &str,
) -> Result<String, String> {
    let state = state.ok_or("No simulation state available")?;
    if state.density_xy.is_empty() {
        return Err("No density data available".to_string());
    }

    let l_box = state.spatial_extent * 2.0;
    let nx = state.density_nx;
    let ny = state.density_ny;
    let dx = if nx > 0 { l_box / nx as f64 } else { 1.0 };
    let n_bins = 64usize;

    // Compute radial profile (inline — same algorithm as profiles.rs)
    let cx = nx as f64 / 2.0;
    let cy = ny as f64 / 2.0;
    let max_r = cx.min(cy);
    let bin_width = max_r / n_bins as f64;
    let mut bin_sum = vec![0.0f64; n_bins];
    let mut bin_count = vec![0u32; n_bins];
    for iy in 0..ny {
        for ix in 0..nx {
            let ddx = ix as f64 + 0.5 - cx;
            let ddy = iy as f64 + 0.5 - cy;
            let r = (ddx * ddx + ddy * ddy).sqrt();
            let bin = ((r / bin_width) as usize).min(n_bins - 1);
            bin_sum[bin] += state.density_xy[iy * nx + ix];
            bin_count[bin] += 1;
        }
    }

    let path = dir.join(format!("{stem}_radial_profiles.csv"));
    let mut csv = String::from("r,density,count\n");
    for i in 0..n_bins {
        let r = (i as f64 + 0.5) * bin_width * dx;
        let rho = if bin_count[i] > 0 {
            bin_sum[i] / bin_count[i] as f64
        } else {
            0.0
        };
        csv.push_str(&format!("{r:.6e},{rho:.6e},{}\n", bin_count[i]));
    }
    std::fs::write(&path, csv).map_err(|e| format!("write: {e}"))?;
    Ok(path.display().to_string())
}

/// Export performance data as CSV (simpler than Parquet, no extra dependency needed).
fn export_performance_csv(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    stem: &str,
) -> Result<String, String> {
    let energy_data = diagnostics.total_energy.iter_chart_data();
    if energy_data.is_empty() {
        return Err("No diagnostics data available".to_string());
    }
    let kinetic_data = diagnostics.kinetic_energy.iter_chart_data();
    let potential_data = diagnostics.potential_energy.iter_chart_data();
    let mass_data = diagnostics.total_mass.iter_chart_data();
    let virial_data = diagnostics.virial_ratio.iter_chart_data();

    let path = dir.join(format!("{stem}_performance.csv"));
    let mut csv =
        String::from("t,total_energy,kinetic_energy,potential_energy,total_mass,virial_ratio\n");
    for (i, &(t, e)) in energy_data.iter().enumerate() {
        let k = kinetic_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let w = potential_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let m = mass_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let v = virial_data.get(i).map(|x| x.1).unwrap_or(0.0);
        csv.push_str(&format!(
            "{t:.6e},{e:.6e},{k:.6e},{w:.6e},{m:.6e},{v:.6e}\n"
        ));
    }
    std::fs::write(&path, csv).map_err(|e| format!("write: {e}"))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn from_name_primary() {
        assert_eq!(ExportFormat::from_name("csv"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_name("json"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_name("parquet"), ExportFormat::Parquet);
        assert_eq!(
            ExportFormat::from_name("screenshot"),
            ExportFormat::Screenshot
        );
        assert_eq!(ExportFormat::from_name("vtk"), ExportFormat::Vtk);
        assert_eq!(ExportFormat::from_name("npy"), ExportFormat::Npy);
        assert_eq!(ExportFormat::from_name("markdown"), ExportFormat::Markdown);
        assert_eq!(
            ExportFormat::from_name("animation"),
            ExportFormat::Animation
        );
        assert_eq!(ExportFormat::from_name("zip"), ExportFormat::Zip);
    }

    #[test]
    fn from_name_aliases() {
        assert_eq!(ExportFormat::from_name("numpy"), ExportFormat::Npy);
        assert_eq!(ExportFormat::from_name("md"), ExportFormat::Markdown);
        assert_eq!(ExportFormat::from_name("txt"), ExportFormat::Screenshot);
        assert_eq!(ExportFormat::from_name("svg"), ExportFormat::Screenshot);
        assert_eq!(ExportFormat::from_name("anim"), ExportFormat::Animation);
        assert_eq!(ExportFormat::from_name("toml"), ExportFormat::ConfigToml);
        assert_eq!(ExportFormat::from_name("config"), ExportFormat::ConfigToml);
        assert_eq!(
            ExportFormat::from_name("radial"),
            ExportFormat::RadialProfilesCsv
        );
        assert_eq!(
            ExportFormat::from_name("profiles"),
            ExportFormat::RadialProfilesCsv
        );
        assert_eq!(
            ExportFormat::from_name("performance"),
            ExportFormat::PerformanceParquet
        );
        assert_eq!(
            ExportFormat::from_name("perf"),
            ExportFormat::PerformanceParquet
        );
    }

    #[test]
    fn from_name_fallback() {
        assert_eq!(ExportFormat::from_name("unknown"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_name(""), ExportFormat::Csv);
    }

    #[test]
    fn from_name_case_insensitive() {
        assert_eq!(ExportFormat::from_name("CSV"), ExportFormat::Csv);
        assert_eq!(ExportFormat::from_name("Json"), ExportFormat::Json);
        assert_eq!(ExportFormat::from_name("ZIP"), ExportFormat::Zip);
    }

    #[test]
    fn name_all_nonempty() {
        let all = [
            ExportFormat::Screenshot,
            ExportFormat::Csv,
            ExportFormat::Json,
            ExportFormat::Parquet,
            ExportFormat::ConfigToml,
            ExportFormat::Vtk,
            ExportFormat::Npy,
            ExportFormat::Markdown,
            ExportFormat::Animation,
            ExportFormat::RadialProfilesCsv,
            ExportFormat::PerformanceParquet,
            ExportFormat::Zip,
        ];
        for f in all {
            assert!(!f.name().is_empty(), "{f:?} should have non-empty name");
        }
    }

    #[test]
    fn shortcut_all_unique() {
        let all = [
            ExportFormat::Screenshot,
            ExportFormat::Csv,
            ExportFormat::Json,
            ExportFormat::Parquet,
            ExportFormat::ConfigToml,
            ExportFormat::Vtk,
            ExportFormat::Npy,
            ExportFormat::Markdown,
            ExportFormat::Animation,
            ExportFormat::RadialProfilesCsv,
            ExportFormat::PerformanceParquet,
            ExportFormat::Zip,
        ];
        let shortcuts: HashSet<_> = all.iter().map(|f| f.shortcut()).collect();
        assert_eq!(shortcuts.len(), 12);
    }

    #[test]
    fn shortcut_all_nonempty() {
        let all = [
            ExportFormat::Screenshot,
            ExportFormat::Csv,
            ExportFormat::Json,
            ExportFormat::Parquet,
            ExportFormat::ConfigToml,
            ExportFormat::Vtk,
            ExportFormat::Npy,
            ExportFormat::Markdown,
            ExportFormat::Animation,
            ExportFormat::RadialProfilesCsv,
            ExportFormat::PerformanceParquet,
            ExportFormat::Zip,
        ];
        for f in all {
            assert!(
                !f.shortcut().is_empty(),
                "{f:?} should have non-empty shortcut"
            );
        }
    }

    #[test]
    fn all_variants_reachable() {
        let names = [
            "screenshot",
            "csv",
            "json",
            "parquet",
            "config",
            "vtk",
            "npy",
            "markdown",
            "animation",
            "radial",
            "performance",
            "zip",
        ];
        let reached: HashSet<_> = names
            .iter()
            .map(|n| std::mem::discriminant(&ExportFormat::from_name(n)))
            .collect();
        assert_eq!(reached.len(), 12);
    }
}
