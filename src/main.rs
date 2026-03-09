use clap::Parser;
use tui::app::App;
use tui::cli::Cli;
mod annotations;
mod colormaps;
mod config;
mod data;
mod export;
mod notifications;
mod runner;
mod session;
mod sim;
mod themes;
mod toml;
mod tui;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tui::errors::init()?;
    tui::logging::init()?;

    let args = Cli::parse();

    // Modes that don't launch the TUI
    if args.generate_man {
        Cli::print_man_page()?;
        return Ok(());
    }
    if args.batch {
        return run_batch(args.config).await;
    }
    if let Some(ref dir) = args.playback {
        eprintln!("phasma: playback mode not yet available — requires HDF5/Parquet backend");
        eprintln!("  directory: {dir}");
        return Ok(());
    }
    if let Some(ref dirs) = args.compare {
        eprintln!("phasma: comparison mode not yet available");
        eprintln!("  directories: {}", dirs.join(", "));
        return Ok(());
    }
    if let Some(ref toml_path) = args.sweep {
        eprintln!("phasma: parameter sweep mode not yet available");
        eprintln!("  sweep config: {toml_path}");
        return Ok(());
    }
    if let Some(ref toml_path) = args.convergence {
        eprintln!("phasma: convergence study mode not yet available");
        eprintln!("  convergence config: {toml_path}");
        return Ok(());
    }
    if let Some(ref dir) = args.regression_test {
        eprintln!("phasma: regression test mode not yet available");
        eprintln!("  reference directory: {dir}");
        return Ok(());
    }
    if let Some(ref dir) = args.monitor {
        eprintln!("phasma: monitor mode not yet available");
        eprintln!("  job directory: {dir}");
        return Ok(());
    }
    if let Some(ref path) = args.tail {
        eprintln!("phasma: tail mode not yet available");
        eprintln!("  recording path: {path}");
        return Ok(());
    }
    if args.wizard {
        eprintln!("phasma: guided wizard not yet available");
        return Ok(());
    }
    if let Some(ref name) = args.save_preset {
        eprintln!("phasma: saving preset '{name}' not yet available");
        return Ok(());
    }
    if let Some(ref dirs) = args.batch_compare {
        eprintln!("phasma: batch comparison mode not yet available");
        eprintln!("  directories: {}", dirs.join(", "));
        return Ok(());
    }

    let mut app = App::new(4.0, 60.0, args.config, args.run)?;
    app.run().await?;
    Ok(())
}

/// Headless batch mode — runs the caustic simulation and prints progress to stderr.
async fn run_batch(config_path: Option<String>) -> color_eyre::Result<()> {
    let path = config_path.unwrap_or_else(|| "run.toml".to_string());
    eprintln!("phasma batch: starting simulation  config={path}");
    let mut handle = sim::SimHandle::spawn(path);
    while let Some(state) = handle.state_rx.recv().await {
        eprintln!(
            "  t={:.4}  step={}  |ΔE/E|={:.2e}  M={:.4}",
            state.t,
            state.step,
            state.energy_drift(),
            state.total_mass,
        );
        if let Some(reason) = state.exit_reason {
            eprintln!("phasma batch: exit — {reason}");
            break;
        }
    }
    handle.task.abort();
    eprintln!("phasma batch: simulation complete");
    Ok(())
}
