//! Regression test runner — re-run a reference config and compare results.
//! Exit code 0 = PASS, exit code 1 = FAIL.

use std::path::Path;

use crate::runner::batch::load_run_dir;
use crate::sim::SimHandle;

pub async fn run_regression_test(dir: &str) -> anyhow::Result<bool> {
    let ref_dir = Path::new(dir);
    eprintln!("phasma regression: reference directory: {dir}");

    // 1. Load reference metadata + final state
    let (ref_meta, ref_snapshots) = load_run_dir(ref_dir)?;
    let ref_final = ref_snapshots
        .last()
        .ok_or_else(|| anyhow::anyhow!("no snapshots in reference directory"))?;

    let ref_energy_drift = ref_final.energy_drift().abs();
    let ref_mass = ref_final.total_mass;
    eprintln!(
        "phasma regression: reference — steps={}, t={:.4}, |ΔE/E|={:.2e}, M={:.4}",
        ref_meta.total_steps, ref_meta.final_time, ref_energy_drift, ref_mass,
    );

    // 2. Re-run with the same config
    let config_path = ref_dir.join("config.toml");
    if !config_path.exists() {
        anyhow::bail!("config.toml not found in {dir}");
    }
    let config_str = config_path.display().to_string();
    eprintln!("phasma regression: re-running {config_str}...");

    let mut handle = SimHandle::spawn(config_str);
    let mut new_final = None;
    while let Some(state) = handle.state_rx.recv().await {
        let is_exit = state.exit_reason.is_some();
        new_final = Some(state);
        if is_exit {
            break;
        }
    }
    handle.task.abort();

    let new_state = new_final.ok_or_else(|| anyhow::anyhow!("simulation produced no output"))?;
    let new_energy_drift = new_state.energy_drift().abs();
    let new_mass = new_state.total_mass;

    eprintln!(
        "phasma regression: new run — steps={}, t={:.4}, |ΔE/E|={:.2e}, M={:.4}",
        new_state.step, new_state.t, new_energy_drift, new_mass,
    );

    // 3. Compare
    let energy_diff = (new_energy_drift - ref_energy_drift).abs();
    let mass_rel_diff = if ref_mass != 0.0 {
        (new_mass - ref_mass).abs() / ref_mass.abs()
    } else {
        0.0
    };

    // Tolerances: energy drift should not differ by more than 10x or absolute 1e-4
    let energy_tol = (ref_energy_drift * 10.0).max(1e-4);
    let mass_tol = 1e-6;

    let energy_pass = energy_diff <= energy_tol;
    let mass_pass = mass_rel_diff <= mass_tol;

    eprintln!("\nResults:");
    eprintln!(
        "  Energy drift: ref={:.2e}, new={:.2e}, diff={:.2e} (tol={:.2e}) — {}",
        ref_energy_drift,
        new_energy_drift,
        energy_diff,
        energy_tol,
        if energy_pass { "PASS" } else { "FAIL" },
    );
    eprintln!(
        "  Mass:         ref={:.6}, new={:.6}, rel_diff={:.2e} (tol={:.2e}) — {}",
        ref_mass,
        new_mass,
        mass_rel_diff,
        mass_tol,
        if mass_pass { "PASS" } else { "FAIL" },
    );

    let pass = energy_pass && mass_pass;
    if pass {
        eprintln!("\nphasma regression: PASS");
    } else {
        eprintln!("\nphasma regression: FAIL");
    }

    Ok(pass)
}
