//! Parameter sweep mode — vary config fields across a grid, run batch for each.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::config::PhasmaConfig;
use crate::sim::SimHandle;

#[derive(Debug, Deserialize)]
pub struct SweepConfig {
    pub base_config: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    /// Maximum concurrent simulations (default: available_parallelism / 2).
    #[serde(default)]
    pub max_concurrent: Option<usize>,
    pub sweep: SweepSpec,
}

#[derive(Debug, Deserialize)]
pub struct SweepSpec {
    pub parameters: Vec<String>,
    pub values: std::collections::HashMap<String, Vec<toml::Value>>,
}

fn default_output_dir() -> String {
    "output/sweep".to_string()
}

/// Run a parameter sweep.
pub async fn run_sweep(toml_path: &str) -> anyhow::Result<()> {
    let sweep_str = std::fs::read_to_string(toml_path)?;
    let sweep_cfg: SweepConfig = toml::from_str(&sweep_str)?;

    let base_cfg = crate::config::load(&sweep_cfg.base_config)?;

    // Generate Cartesian product of parameter values
    let combos = cartesian_product(&sweep_cfg.sweep.parameters, &sweep_cfg.sweep.values);
    let n_combos = combos.len();
    eprintln!(
        "phasma sweep: {} combinations from {} parameters",
        n_combos,
        sweep_cfg.sweep.parameters.len()
    );

    std::fs::create_dir_all(&sweep_cfg.output_dir)?;

    let max_concurrent = sweep_cfg.max_concurrent.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(1))
            .unwrap_or(2)
    });
    eprintln!("phasma sweep: max_concurrent = {max_concurrent}");
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    // Prepare all configs up front (override_field needs &mut PhasmaConfig)
    let mut prepared: Vec<(usize, String, String)> = Vec::with_capacity(n_combos);
    for (i, combo) in combos.iter().enumerate() {
        let mut cfg = base_cfg.clone();
        let mut combo_desc = Vec::new();
        for (param, value) in combo {
            override_field(&mut cfg, param, value)?;
            combo_desc.push(format!("{param}={value}"));
        }
        let desc = combo_desc.join("_");
        let temp_dir = PathBuf::from(&sweep_cfg.output_dir).join(format!("run_{i:04}"));
        std::fs::create_dir_all(&temp_dir)?;
        cfg.output.directory = temp_dir.display().to_string();
        let temp_config = temp_dir.join("config.toml");
        let toml_str = toml::to_string_pretty(&cfg)?;
        std::fs::write(&temp_config, &toml_str)?;
        prepared.push((i, desc, temp_config.display().to_string()));
    }

    // Spawn all simulations concurrently, gated by semaphore
    let mut join_set = JoinSet::new();
    for (i, desc, config_str) in prepared {
        let permit = Arc::clone(&semaphore).acquire_owned().await?;
        eprintln!("phasma sweep: [{}/{}] {desc}", i + 1, n_combos);
        join_set.spawn(async move {
            let mut handle = SimHandle::spawn_unbounded(config_str);
            let mut final_state = None;
            while let Some(state) = handle.state_rx.recv_async().await {
                for msg in &state.log_messages {
                    eprintln!("  [{desc}] {msg}");
                }
                let is_exit = state.exit_reason.is_some();
                final_state = Some(state);
                if is_exit {
                    break;
                }
            }
            handle.task.abort();
            drop(permit);

            let result = final_state.map(|state| {
                (
                    desc.clone(),
                    state.step,
                    state.t,
                    state.energy_drift(),
                    state.total_mass,
                    state
                        .exit_reason
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "—".into()),
                )
            });
            (i, result)
        });
    }

    // Collect results and sort by original index
    let mut results = Vec::with_capacity(n_combos);
    while let Some(res) = join_set.join_next().await {
        if let Ok((_, Some(row))) = res {
            results.push(row);
        }
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));

    // Print summary table
    eprintln!("\n{:-<80}", "");
    eprintln!(
        "{:<30} {:>6} {:>8} {:>12} {:>8} {:>12}",
        "Config", "Steps", "t_final", "|ΔE/E|", "Mass", "Exit"
    );
    eprintln!("{:-<80}", "");
    for (desc, steps, t, drift, mass, exit) in &results {
        eprintln!(
            "{:<30} {:>6} {:>8.3} {:>12.2e} {:>8.4} {:>12}",
            desc, steps, t, drift, mass, exit
        );
    }

    eprintln!(
        "\nphasma sweep: complete — {} runs in {}",
        results.len(),
        sweep_cfg.output_dir
    );
    Ok(())
}

/// Generate Cartesian product of parameter values.
fn cartesian_product(
    params: &[String],
    values: &std::collections::HashMap<String, Vec<toml::Value>>,
) -> Vec<Vec<(String, toml::Value)>> {
    if params.is_empty() {
        return vec![vec![]];
    }

    let mut result = vec![vec![]];
    for param in params {
        let vals = match values.get(param) {
            Some(v) => v.clone(),
            None => continue,
        };
        let mut new_result = Vec::new();
        for existing in &result {
            for val in &vals {
                let mut combo = existing.clone();
                combo.push((param.clone(), val.clone()));
                new_result.push(combo);
            }
        }
        result = new_result;
    }
    result
}

/// Override a dotted-path field in a PhasmaConfig via TOML round-trip.
fn override_field(cfg: &mut PhasmaConfig, path: &str, value: &toml::Value) -> anyhow::Result<()> {
    let mut table: toml::Value = toml::Value::try_from(&*cfg)?;

    // Walk dotted path
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = &mut table;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Set the value
            if let toml::Value::Table(t) = current {
                t.insert(part.to_string(), value.clone());
            } else {
                anyhow::bail!("path '{path}' does not point to a table field");
            }
        } else {
            current = current
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("path '{path}': intermediate is not a table"))?
                .entry(part.to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        }
    }

    // Deserialize back
    *cfg = table.try_into()?;
    Ok(())
}
