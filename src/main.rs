use clap::Parser;
use tui::app::App;
use tui::cli::Cli;
mod sim;
mod toml;
mod tui;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tui::errors::init()?;
    tui::logging::init()?;

    let args = Cli::parse();

    if args.batch {
        return run_batch(args.config).await;
    }

    let mut app = App::new(args.tick_rate, args.frame_rate, args.config, args.run)?;
    app.run().await?;
    Ok(())
}

/// Headless batch mode — runs the caustic simulation and prints progress to stderr.
async fn run_batch(config_path: Option<String>) -> color_eyre::Result<()> {
    let path = config_path.unwrap_or_else(|| "run.toml".to_string());
    eprintln!("phasma batch: starting simulation  config={path}");
    let mut handle = sim::SimHandle::spawn(path);
    // Drain state_rx until the sim thread finishes (channel closes) or sends an exit reason.
    while let Some(state) = handle.state_rx.recv().await {
        eprintln!(
            "  t={:.4}  step={}  |ΔE/E|={:.2e}  M={:.4}",
            state.t,
            state.step,
            state.energy_drift(),
            state.total_mass,
        );
        if state.exit_reason.is_some() {
            eprintln!("phasma batch: exit — {}", state.exit_reason.unwrap());
            break;
        }
    }
    handle.task.abort();
    eprintln!("phasma batch: simulation complete");
    Ok(())
}
