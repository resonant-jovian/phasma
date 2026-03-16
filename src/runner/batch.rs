//! Headless batch execution with structured output to disk.
//!
//! Output directory layout:
//! ```text
//! output/<prefix>_YYYYMMDD_HHMMSS/
//!   config.toml          -- copy of input config
//!   diagnostics.csv      -- time series (appended each step)
//!   snapshots/
//!     state_000000.json   -- periodic SimState snapshots
//!     state_final.json    -- last state
//!   metadata.json         -- version, timing, exit reason, snapshot count
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::PhasmaConfig;
use crate::runner::RunMetadata;
use crate::sim::{SimHandle, SimState};

/// Run a simulation in headless batch mode with output saved to disk.
pub async fn run_batch(config_path: Option<String>) -> color_eyre::Result<()> {
    let path = config_path.unwrap_or_else(|| "run.toml".to_string());
    eprintln!("phasma batch: starting simulation  config={path}");

    // Load config for output settings
    let cfg = crate::config::load(&path).map_err(|e| color_eyre::eyre::eyre!("{e:#}"))?;
    let output_dir = create_output_dir(&cfg, &path)?;
    eprintln!("phasma batch: output → {}", output_dir.display());

    // Copy config to output dir
    let _ = std::fs::copy(&path, output_dir.join("config.toml"));

    // Create snapshots dir
    let snap_dir = output_dir.join("snapshots");
    std::fs::create_dir_all(&snap_dir)?;

    // Open diagnostics CSV
    let csv_path = output_dir.join("diagnostics.csv");
    let mut csv_file = std::fs::File::create(&csv_path)?;
    writeln!(
        csv_file,
        "time,step,total_energy,kinetic_energy,potential_energy,total_mass,\
         casimir_c2,entropy,virial_ratio,max_density,step_wall_ms,energy_drift"
    )?;

    let start_time = chrono::Utc::now();
    let mut handle = SimHandle::spawn(path.clone());
    let mut snapshot_count: usize = 0;
    let mut last_snapshot_t: f64 = f64::NEG_INFINITY;
    let snapshot_interval = cfg.output.snapshot_interval;
    let mut final_state: Option<SimState> = None;

    while let Some(state) = handle.state_rx.recv().await {
        // Append to diagnostics CSV
        let _ = writeln!(
            csv_file,
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            state.t,
            state.step,
            state.total_energy,
            state.kinetic_energy,
            state.potential_energy,
            state.total_mass,
            state.casimir_c2,
            state.entropy,
            state.virial_ratio,
            state.max_density,
            state.step_wall_ms,
            state.energy_drift(),
        );

        // Verbose log messages to stderr
        for msg in &state.log_messages {
            eprintln!("  [verbose] {msg}");
        }

        // Progress to stderr
        eprintln!(
            "  t={:.4}  step={}  |ΔE/E|={:.2e}  M={:.4}",
            state.t,
            state.step,
            state.energy_drift(),
            state.total_mass,
        );

        // Periodic snapshots
        if state.t - last_snapshot_t >= snapshot_interval {
            let snap_path = snap_dir.join(format!("state_{:06}.json", snapshot_count));
            if let Ok(json) = serde_json::to_string_pretty(&state) {
                let _ = std::fs::write(&snap_path, json);
            }
            snapshot_count += 1;
            last_snapshot_t = state.t;
        }

        let is_exit = state.exit_reason.is_some();
        if is_exit {
            eprintln!(
                "phasma batch: exit — {}",
                state.exit_reason.as_ref().unwrap()
            );
        }
        final_state = Some(state);
        if is_exit {
            break;
        }
    }
    handle.task.abort();

    // Write final snapshot
    if let Some(ref state) = final_state {
        let final_path = snap_dir.join("state_final.json");
        if let Ok(json) = serde_json::to_string_pretty(state) {
            let _ = std::fs::write(&final_path, json);
        }
    }

    // Write metadata
    let end_time = chrono::Utc::now();
    let metadata = RunMetadata {
        phasma_version: env!("CARGO_PKG_VERSION").to_string(),
        config_path: path,
        output_dir: output_dir.display().to_string(),
        start_time: start_time.to_rfc3339(),
        end_time: Some(end_time.to_rfc3339()),
        exit_reason: final_state
            .as_ref()
            .and_then(|s| s.exit_reason.map(|r| r.to_string())),
        total_steps: final_state.as_ref().map(|s| s.step).unwrap_or(0),
        final_time: final_state.as_ref().map(|s| s.t).unwrap_or(0.0),
        snapshot_count,
    };
    let meta_path = output_dir.join("metadata.json");
    if let Ok(json) = serde_json::to_string_pretty(&metadata) {
        let _ = std::fs::write(&meta_path, json);
    }

    eprintln!("phasma batch: simulation complete");
    eprintln!(
        "phasma batch: {} snapshots written to {}",
        snapshot_count,
        output_dir.display()
    );
    Ok(())
}

/// Create the timestamped output directory.
fn create_output_dir(cfg: &PhasmaConfig, _config_path: &str) -> color_eyre::Result<PathBuf> {
    let now = chrono::Local::now();
    let dir_name = format!("{}_{}", cfg.output.prefix, now.format("%Y%m%d_%H%M%S"));
    let dir = PathBuf::from(&cfg.output.directory).join(dir_name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Load a completed run directory: returns (metadata, sorted snapshots).
pub fn load_run_dir(dir: &Path) -> anyhow::Result<(RunMetadata, Vec<SimState>)> {
    let meta_path = dir.join("metadata.json");
    let meta_str = std::fs::read_to_string(&meta_path)
        .map_err(|e| anyhow::anyhow!("read metadata.json: {e}"))?;
    let meta: RunMetadata =
        serde_json::from_str(&meta_str).map_err(|e| anyhow::anyhow!("parse metadata.json: {e}"))?;

    let snap_dir = dir.join("snapshots");
    let mut snapshots = Vec::new();

    if snap_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&snap_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            // Skip state_final.json to avoid duplicates
            if path.file_name().is_some_and(|n| n == "state_final.json") {
                continue;
            }
            if let Ok(json) = std::fs::read_to_string(&path)
                && let Ok(state) = serde_json::from_str::<SimState>(&json)
            {
                snapshots.push(state);
            }
        }
    }

    // If no numbered snapshots, try loading state_final.json
    if snapshots.is_empty() {
        let final_path = snap_dir.join("state_final.json");
        if final_path.exists()
            && let Ok(json) = std::fs::read_to_string(&final_path)
            && let Ok(state) = serde_json::from_str::<SimState>(&json)
        {
            snapshots.push(state);
        }
    }

    Ok((meta, snapshots))
}
