use std::path::Path;

use crate::sim::SimState;

pub fn export_npy(dir: &Path, state: Option<&SimState>, stem: &str) -> Result<String, String> {
    let Some(state) = state else {
        return Err("no simulation state to export".to_string());
    };

    let path = dir.join(format!("{stem}_density_xy.npy"));
    let nx = state.density_nx;
    let ny = state.density_ny;

    let arr = ndarray::ArrayView2::from_shape((ny, nx), &state.density_xy)
        .map_err(|e| format!("array shape: {e}"))?;

    ndarray_npy::write_npy(&path, &arr).map_err(|e| format!("write npy: {e}"))?;

    Ok(path.display().to_string())
}
