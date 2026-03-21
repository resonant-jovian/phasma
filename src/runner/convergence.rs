//! Convergence study — run at increasing resolutions and compute convergence rates.

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

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
    /// Maximum concurrent resolution runs (default: available_parallelism / 2).
    #[serde(default)]
    pub max_concurrent: Option<usize>,
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

    let resolutions = &conv_cfg.convergence.resolutions;
    let n_res = resolutions.len();
    eprintln!(
        "phasma convergence: {} resolutions: {:?}",
        n_res, resolutions
    );

    std::fs::create_dir_all(&conv_cfg.output_dir)?;

    let max_concurrent = conv_cfg.convergence.max_concurrent.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(1))
            .unwrap_or(2)
    });
    eprintln!("phasma convergence: max_concurrent = {max_concurrent}");
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    // Prepare configs up front
    let mut prepared: Vec<(u32, String)> = Vec::with_capacity(n_res);
    for &res in resolutions {
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
        prepared.push((res, temp_config.display().to_string()));
    }

    // Spawn resolution runs concurrently, gated by semaphore
    let mut join_set = JoinSet::new();
    for (res, config_str) in prepared {
        let permit = Arc::clone(&semaphore).acquire_owned().await?;
        eprintln!("phasma convergence: running N={res}...");
        join_set.spawn(async move {
            let mut handle = SimHandle::spawn_unbounded(config_str);
            let mut final_state = None;
            while let Some(state) = handle.state_rx.recv_async().await {
                for msg in &state.log_messages {
                    eprintln!("  [N={res}] {msg}");
                }
                let is_exit = state.exit_reason.is_some();
                final_state = Some(state);
                if is_exit {
                    break;
                }
            }
            handle.task.abort();
            drop(permit);

            final_state.map(|state| {
                let initial_mass = if state.total_mass != 0.0 {
                    state.total_mass
                } else {
                    1.0
                };
                RunResult {
                    resolution: res,
                    final_energy_drift: state.energy_drift().abs(),
                    final_mass_drift: (state.total_mass - initial_mass).abs() / initial_mass,
                    final_time: state.t,
                    steps: state.step,
                }
            })
        });
    }

    // Collect and sort by resolution
    let mut results: Vec<RunResult> = Vec::with_capacity(n_res);
    while let Some(res) = join_set.join_next().await {
        if let Ok(Some(run_result)) = res {
            results.push(run_result);
        }
    }
    results.sort_by_key(|r| r.resolution);

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
