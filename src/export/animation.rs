use std::path::Path;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

/// Export density projection snapshots as numbered NPY frames.
/// Each frame is the density_xy 2D array at successive time steps
/// stored in the diagnostics buffer.
///
/// Since we only have the current SimState snapshot (not a full history of
/// density fields), this exports what we have: the current density projections
/// plus a CSV of the time series for reconstruction.
pub fn export_animation_frames(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
    stem: &str,
) -> Result<String, String> {
    let Some(state) = state else {
        return Err("no simulation state — run a simulation first".to_string());
    };

    let frames_dir = dir.join(format!("{stem}_frames"));
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("create frames dir: {e}"))?;

    // Export current density projections as NPY
    let nx = state.density_nx;
    let ny = state.density_ny;
    let nz = state.density_nz;

    if nx > 0 && ny > 0 {
        let arr = ndarray::ArrayView2::from_shape((ny, nx), &state.density_xy)
            .map_err(|e| format!("array shape xy: {e}"))?;
        let path = frames_dir.join(format!("{stem}_density_xy_t{:.4}.npy", state.t));
        ndarray_npy::write_npy(&path, &arr).map_err(|e| format!("write npy: {e}"))?;
    }

    if nx > 0 && nz > 0 {
        let arr = ndarray::ArrayView2::from_shape((nz, nx), &state.density_xz)
            .map_err(|e| format!("array shape xz: {e}"))?;
        let path = frames_dir.join(format!("{stem}_density_xz_t{:.4}.npy", state.t));
        ndarray_npy::write_npy(&path, &arr).map_err(|e| format!("write npy: {e}"))?;
    }

    if ny > 0 && nz > 0 {
        let arr = ndarray::ArrayView2::from_shape((nz, ny), &state.density_yz)
            .map_err(|e| format!("array shape yz: {e}"))?;
        let path = frames_dir.join(format!("{stem}_density_yz_t{:.4}.npy", state.t));
        ndarray_npy::write_npy(&path, &arr).map_err(|e| format!("write npy: {e}"))?;
    }

    // Export phase-space slice (x1-v1 projection)
    if state.phase_nx > 0
        && state.phase_nv > 0
        && let Some(ps) = state.phase_slices.first()
        && !ps.is_empty()
    {
        let arr = ndarray::ArrayView2::from_shape((state.phase_nv, state.phase_nx), ps)
            .map_err(|e| format!("array shape phase: {e}"))?;
        let path = frames_dir.join(format!("{stem}_phase_xvx_t{:.4}.npy", state.t));
        ndarray_npy::write_npy(&path, &arr).map_err(|e| format!("write npy: {e}"))?;
    }

    // Export time series CSV for reconstruction
    let csv_path = frames_dir.join(format!("{stem}_time_series.csv"));
    let energy = diagnostics.total_energy.iter_chart_data();
    let mut csv = String::from("time,total_energy\n");
    for (t, e) in &energy {
        csv.push_str(&format!("{t},{e}\n"));
    }
    std::fs::write(&csv_path, csv).map_err(|e| format!("write csv: {e}"))?;

    let count = [
        nx > 0 && ny > 0,
        nx > 0 && nz > 0,
        ny > 0 && nz > 0,
        state.phase_nx > 0,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    Ok(format!(
        "{} — {count} arrays + time_series.csv",
        frames_dir.display()
    ))
}
