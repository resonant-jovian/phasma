//! Convergence study — run at increasing resolutions and compute convergence rates.

use serde::Deserialize;

use crate::sim::SimHandle;

#[derive(Debug, Deserialize)]
pub struct ConvergenceConfig {
    pub base_config: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    pub convergence: ConvergenceSpec,
}

#[derive(Debug, Deserialize)]
pub struct ConvergenceSpec {
    pub resolutions: Vec<u32>,
    #[serde(default = "default_true")]
    pub velocity_scale: bool,
}

fn default_output_dir() -> String {
    "output/convergence".to_string()
}
fn default_true() -> bool {
    true
}
struct RunResult {
    resolution: u32,
    final_energy_drift: f64,
    final_mass_drift: f64,
    final_time: f64,
    steps: u64,
}

pub async fn run_convergence(toml_path: &str) -> anyhow::Result<()> {
    let cfg_str = std::fs::read_to_string(toml_path)?;
    let conv_cfg: ConvergenceConfig = toml::from_str(&cfg_str)?;

    let base_cfg = crate::config::load(&conv_cfg.base_config)?;

    eprintln!(
        "phasma convergence: {} resolutions: {:?}",
        conv_cfg.convergence.resolutions.len(),
        conv_cfg.convergence.resolutions
    );

    std::fs::create_dir_all(&conv_cfg.output_dir)?;

    let mut results = Vec::new();

    for &res in &conv_cfg.convergence.resolutions {
        let mut cfg = base_cfg.clone();
        cfg.domain.spatial_resolution = res;
        if conv_cfg.convergence.velocity_scale {
            cfg.domain.velocity_resolution = res;
        }

        let run_dir = std::path::PathBuf::from(&conv_cfg.output_dir).join(format!("res_{res:04}"));
        std::fs::create_dir_all(&run_dir)?;
        cfg.output.directory = run_dir.display().to_string();

        let temp_config = run_dir.join("config.toml");
        let toml_str = toml::to_string_pretty(&cfg)?;
        std::fs::write(&temp_config, &toml_str)?;

        eprintln!("phasma convergence: running N={res}...");
        let config_str = temp_config.display().to_string();
        let mut handle = SimHandle::spawn_unbounded(config_str);
        let mut final_state = None;
        while let Some(state) = handle.state_rx.recv_async().await {
            for msg in &state.log_messages {
                eprintln!("  [verbose] {msg}");
            }
            let is_exit = state.exit_reason.is_some();
            final_state = Some(state);
            if is_exit {
                break;
            }
        }
        handle.task.abort();

        if let Some(state) = final_state {
            let initial_mass = if state.total_mass != 0.0 {
                state.total_mass
            } else {
                1.0
            };
            results.push(RunResult {
                resolution: res,
                final_energy_drift: state.energy_drift().abs(),
                final_mass_drift: (state.total_mass - initial_mass).abs() / initial_mass,
                final_time: state.t,
                steps: state.step,
            });
        }
    }

    // Print results table
    eprintln!("\n{:-<80}", "");
    eprintln!(
        "{:>4} {:>8} {:>12} {:>12} {:>6}",
        "N", "t_final", "|ΔE/E|", "|ΔM/M|", "Steps"
    );
    eprintln!("{:-<80}", "");
    for r in &results {
        eprintln!(
            "{:>4} {:>8.3} {:>12.2e} {:>12.2e} {:>6}",
            r.resolution, r.final_time, r.final_energy_drift, r.final_mass_drift, r.steps
        );
    }

    // Compute convergence rates
    if results.len() >= 2 {
        eprintln!("\nConvergence rates (log2(error_N / error_2N)):");
        for i in 0..results.len() - 1 {
            let r1 = &results[i];
            let r2 = &results[i + 1];
            if r1.final_energy_drift > 0.0 && r2.final_energy_drift > 0.0 {
                let rate = (r1.final_energy_drift / r2.final_energy_drift).log2();
                eprintln!(
                    "  N={} → {}: energy rate = {:.2}",
                    r1.resolution, r2.resolution, rate
                );
            }
            if r1.final_mass_drift > 0.0 && r2.final_mass_drift > 0.0 {
                let rate = (r1.final_mass_drift / r2.final_mass_drift).log2();
                eprintln!(
                    "  N={} → {}: mass rate = {:.2}",
                    r1.resolution, r2.resolution, rate
                );
            }
        }
    }

    eprintln!(
        "\nphasma convergence: complete — {} runs in {}",
        results.len(),
        conv_cfg.output_dir
    );
    Ok(())
}
