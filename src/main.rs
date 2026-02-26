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

/// Headless batch mode — run the simulation stub without the TUI.
/// Intended for HPC / scripted use with `--batch`.
async fn run_batch(config_path: Option<String>) -> color_eyre::Result<()> {
    let path = config_path.unwrap_or_else(|| "run.toml".to_string());
    eprintln!("phasma batch: starting simulation  config={path}");
    // TODO: caustic::Simulation::new() + step loop with progress written to stderr
    let handle = sim::SimHandle::spawn(path);
    let _ = handle.task.await;
    eprintln!("phasma batch: simulation complete");
    Ok(())
}
