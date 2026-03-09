use std::io::Write;
use std::path::Path;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

/// Export a text-based summary "screenshot" of the current simulation state.
pub fn export_screenshot(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
) -> Result<String, String> {
    let path = dir.join("screenshot.txt");
    let mut f = std::fs::File::create(&path).map_err(|e| format!("create: {e}"))?;

    writeln!(
        f,
        "PHASMA v{} — Simulation Snapshot",
        env!("CARGO_PKG_VERSION")
    )
    .map_err(|e| e.to_string())?;
    writeln!(f, "{}", "=".repeat(50)).map_err(|e| e.to_string())?;

    if let Some(s) = state {
        writeln!(f).map_err(|e| e.to_string())?;
        writeln!(
            f,
            "Time:         {:.6} / {:.6}  ({:.1}%)",
            s.t,
            s.t_final,
            s.progress() * 100.0
        )
        .map_err(|e| e.to_string())?;
        writeln!(f, "Step:         {}", s.step).map_err(|e| e.to_string())?;
        writeln!(f, "Wall/step:    {:.1} ms", s.step_wall_ms).map_err(|e| e.to_string())?;
        writeln!(f).map_err(|e| e.to_string())?;

        writeln!(f, "--- Energy ---").map_err(|e| e.to_string())?;
        writeln!(f, "Total E:      {:.6e}", s.total_energy).map_err(|e| e.to_string())?;
        writeln!(f, "Kinetic T:    {:.6e}", s.kinetic_energy).map_err(|e| e.to_string())?;
        writeln!(f, "Potential W:  {:.6e}", s.potential_energy).map_err(|e| e.to_string())?;
        writeln!(f, "dE/E:         {:.2e}", s.energy_drift()).map_err(|e| e.to_string())?;
        writeln!(f, "Virial 2T/|W|:{:.4}", s.virial_ratio).map_err(|e| e.to_string())?;
        writeln!(f).map_err(|e| e.to_string())?;

        writeln!(f, "--- Conserved ---").map_err(|e| e.to_string())?;
        writeln!(f, "Mass:         {:.6e}", s.total_mass).map_err(|e| e.to_string())?;
        writeln!(f, "Casimir C2:   {:.6e}", s.casimir_c2).map_err(|e| e.to_string())?;
        writeln!(f, "Entropy S:    {:.6e}", s.entropy).map_err(|e| e.to_string())?;
        writeln!(
            f,
            "Momentum:     [{:.2e}, {:.2e}, {:.2e}]",
            s.momentum[0], s.momentum[1], s.momentum[2]
        )
        .map_err(|e| e.to_string())?;
        writeln!(f, "rho_max:      {:.6e}", s.max_density).map_err(|e| e.to_string())?;
        writeln!(f).map_err(|e| e.to_string())?;

        writeln!(f, "--- Grid ---").map_err(|e| e.to_string())?;
        writeln!(
            f,
            "Density:      {}x{}x{}",
            s.density_nx, s.density_ny, s.density_nz
        )
        .map_err(|e| e.to_string())?;
        writeln!(f, "Phase:        {}x{}", s.phase_nx, s.phase_nv).map_err(|e| e.to_string())?;

        if let Some(ref reason) = s.exit_reason {
            writeln!(f).map_err(|e| e.to_string())?;
            writeln!(f, "Exit: {reason}").map_err(|e| e.to_string())?;
        }
    } else {
        writeln!(f, "\nNo simulation state available.").map_err(|e| e.to_string())?;
    }

    writeln!(f).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "Diagnostics:  {} samples",
        diagnostics.total_energy.len()
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}
