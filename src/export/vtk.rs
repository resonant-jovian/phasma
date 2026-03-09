use std::io::Write;
use std::path::Path;

use crate::sim::SimState;

/// Export 3D density field to VTK structured-points legacy format.
/// Readable by ParaView, VisIt, and other VTK-compatible tools.
pub fn export_vtk(dir: &Path, state: Option<&SimState>) -> Result<String, String> {
    let Some(state) = state else {
        return Err("no simulation state to export".to_string());
    };

    let nx = state.density_nx;
    let ny = state.density_ny;
    let nz = state.density_nz;

    if nx == 0 || ny == 0 || nz == 0 {
        return Err("density grid is empty".to_string());
    }

    // Reconstruct full 3D density from the three 2D projections is not possible;
    // export the three projected 2D slices as separate VTK files.
    export_vtk_2d(
        dir,
        "density_xy.vtk",
        &state.density_xy,
        nx,
        ny,
        "density_xy",
    )?;
    export_vtk_2d(
        dir,
        "density_xz.vtk",
        &state.density_xz,
        nx,
        nz,
        "density_xz",
    )?;
    export_vtk_2d(
        dir,
        "density_yz.vtk",
        &state.density_yz,
        ny,
        nz,
        "density_yz",
    )?;

    // Also export phase-space slice
    if !state.phase_slice.is_empty() {
        export_vtk_2d(
            dir,
            "phase_xvx.vtk",
            &state.phase_slice,
            state.phase_nx,
            state.phase_nv,
            "phase_xvx",
        )?;
    }

    Ok(dir.join("density_xy.vtk").display().to_string())
}

fn export_vtk_2d(
    dir: &Path,
    filename: &str,
    data: &[f64],
    nx: usize,
    ny: usize,
    field_name: &str,
) -> Result<(), String> {
    let path = dir.join(filename);
    let mut f = std::fs::File::create(&path).map_err(|e| format!("create {filename}: {e}"))?;

    // VTK legacy ASCII structured points
    writeln!(f, "# vtk DataFile Version 3.0").map_err(|e| e.to_string())?;
    writeln!(f, "phasma {field_name} export").map_err(|e| e.to_string())?;
    writeln!(f, "ASCII").map_err(|e| e.to_string())?;
    writeln!(f, "DATASET STRUCTURED_POINTS").map_err(|e| e.to_string())?;
    writeln!(f, "DIMENSIONS {nx} {ny} 1").map_err(|e| e.to_string())?;
    writeln!(f, "ORIGIN 0 0 0").map_err(|e| e.to_string())?;
    writeln!(f, "SPACING 1 1 1").map_err(|e| e.to_string())?;
    writeln!(f, "POINT_DATA {}", nx * ny).map_err(|e| e.to_string())?;
    writeln!(f, "SCALARS {field_name} double 1").map_err(|e| e.to_string())?;
    writeln!(f, "LOOKUP_TABLE default").map_err(|e| e.to_string())?;

    for val in data.iter().take(nx * ny) {
        writeln!(f, "{val:.8e}").map_err(|e| e.to_string())?;
    }

    Ok(())
}
