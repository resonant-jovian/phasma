//! Batch comparison report — load multiple run directories and generate a Markdown report.

use std::path::Path;

use crate::runner::batch::load_run_dir;

pub struct RunSummary {
    pub dir: String,
    pub resolution: String,
    pub integrator: String,
    pub energy_drift: f64,
    pub final_mass: f64,
    pub total_steps: u64,
    pub final_time: f64,
    pub wall_time: String,
    pub exit_reason: String,
}

/// Load summary from a completed run directory.
pub fn load_run_summary(dir: &Path) -> anyhow::Result<RunSummary> {
    let (meta, snapshots) = load_run_dir(dir)?;

    // Try to load config for resolution/integrator info
    let config_path = dir.join("config.toml");
    let (resolution, integrator) =
        if let Ok(cfg) = crate::config::load(&config_path.display().to_string()) {
            (
                format!(
                    "{}x{}",
                    cfg.domain.spatial_resolution, cfg.domain.velocity_resolution
                ),
                cfg.solver.integrator.clone(),
            )
        } else {
            ("?".into(), "?".into())
        };

    let energy_drift = snapshots
        .last()
        .map(|s| s.energy_drift().abs())
        .unwrap_or(0.0);
    let final_mass = snapshots.last().map(|s| s.total_mass).unwrap_or(0.0);

    // Compute wall time from metadata timestamps
    let wall_time = if let (Some(end), start) = (&meta.end_time, &meta.start_time) {
        if let (Ok(s), Ok(e)) = (
            chrono::DateTime::parse_from_rfc3339(start),
            chrono::DateTime::parse_from_rfc3339(end),
        ) {
            let dur = e.signed_duration_since(s);
            format_duration(dur.num_seconds() as f64)
        } else {
            "?".into()
        }
    } else {
        "?".into()
    };

    Ok(RunSummary {
        dir: dir.display().to_string(),
        resolution,
        integrator,
        energy_drift,
        final_mass,
        total_steps: meta.total_steps,
        final_time: meta.final_time,
        wall_time,
        exit_reason: meta.exit_reason.unwrap_or_else(|| "—".into()),
    })
}

/// Run batch comparison across multiple directories, write Markdown report.
pub fn run_batch_compare(dirs: &[String], report_path: Option<&str>) -> anyhow::Result<()> {
    let mut summaries = Vec::new();

    for dir in dirs {
        let path = Path::new(dir);
        match load_run_summary(path) {
            Ok(s) => summaries.push(s),
            Err(e) => {
                eprintln!("phasma: warning: failed to load {dir}: {e}");
            }
        }
    }

    if summaries.is_empty() {
        anyhow::bail!("no valid run directories found");
    }

    // Generate Markdown report
    let mut report = String::new();
    report.push_str("# Batch Comparison Report\n\n");
    report.push_str(&format!(
        "Generated: {}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    report.push_str("| Directory | Resolution | Integrator | |ΔE/E| | Mass | Steps | t_final | Wall time | Exit |\n");
    report.push_str("|---|---|---|---|---|---|---|---|---|\n");

    for s in &summaries {
        let short_dir = Path::new(&s.dir)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| s.dir.clone());
        report.push_str(&format!(
            "| {} | {} | {} | {:.2e} | {:.4} | {} | {:.3} | {} | {} |\n",
            short_dir,
            s.resolution,
            s.integrator,
            s.energy_drift,
            s.final_mass,
            s.total_steps,
            s.final_time,
            s.wall_time,
            s.exit_reason,
        ));
    }

    let output_path = report_path.unwrap_or("comparison_report.md");
    std::fs::write(output_path, &report)?;
    eprintln!("phasma: comparison report written to {output_path}");

    // Also print to stderr
    eprint!("{report}");

    Ok(())
}

fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else if secs < 3600.0 {
        format!("{}m{:02}s", secs as u64 / 60, secs as u64 % 60)
    } else {
        format!("{}h{:02}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
    }
}
