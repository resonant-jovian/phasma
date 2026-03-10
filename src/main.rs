use clap::Parser;
use color_eyre::eyre::eyre;
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
        return runner::batch::run_batch(args.config).await;
    }
    if args.wizard {
        runner::wizard::run_wizard().map_err(|e| eyre!("{e:#}"))?;
        return Ok(());
    }
    if let Some(ref name) = args.save_preset {
        let config_path = args.config.as_deref().unwrap_or("run.toml");
        config::presets::save_preset(name, config_path).map_err(|e| eyre!("{e:#}"))?;
        return Ok(());
    }
    if let Some(ref toml_path) = args.sweep {
        runner::sweep::run_sweep(toml_path)
            .await
            .map_err(|e| eyre!("{e:#}"))?;
        return Ok(());
    }
    if let Some(ref toml_path) = args.convergence {
        runner::convergence::run_convergence(toml_path)
            .await
            .map_err(|e| eyre!("{e:#}"))?;
        return Ok(());
    }
    if let Some(ref dirs) = args.batch_compare {
        runner::compare::run_batch_compare(dirs, args.report.as_deref())
            .map_err(|e| eyre!("{e:#}"))?;
        return Ok(());
    }
    if let Some(ref dir) = args.regression_test {
        let pass = runner::regression::run_regression_test(dir)
            .await
            .map_err(|e| eyre!("{e:#}"))?;
        if !pass {
            std::process::exit(1);
        }
        return Ok(());
    }

    // TUI-based modes
    if let Some(ref dir) = args.playback {
        return run_playback(dir).await;
    }
    if let Some(ref dirs) = args.compare {
        if dirs.len() >= 2 {
            return run_compare(&dirs[0], &dirs[1]).await;
        }
        eprintln!("phasma: --compare requires exactly 2 directories");
        return Ok(());
    }
    if let Some(ref dir) = args.monitor {
        return run_monitor(dir).await;
    }
    if let Some(ref dir) = args.tail {
        return run_tail(dir).await;
    }

    // Record config usage
    if let Some(ref path) = args.config {
        config::history::push_recent(path);
    }

    // Default: interactive TUI
    let mut app = App::new(4.0, 60.0, args.config, args.run)?;
    app.run().await?;
    Ok(())
}

/// Playback mode: load snapshots from a run directory and replay in TUI.
async fn run_playback(dir: &str) -> color_eyre::Result<()> {
    let path = std::path::Path::new(dir);
    let (_meta, snapshots) = runner::batch::load_run_dir(path)
        .map_err(|e| eyre!("failed to load run directory: {e}"))?;

    if snapshots.is_empty() {
        eprintln!("phasma: no snapshots found in {dir}");
        return Ok(());
    }

    eprintln!(
        "phasma playback: loaded {} snapshots from {dir}",
        snapshots.len()
    );

    // Load config if available
    let config_path = path.join("config.toml");
    let cfg = if config_path.exists() {
        config::load(&config_path.display().to_string()).ok()
    } else {
        None
    };

    let provider = data::playback::PlaybackDataProvider::new(snapshots, cfg);
    let mut app = App::new_with_playback(4.0, 60.0, provider)?;
    app.run().await?;
    Ok(())
}

/// Comparison mode: load two run directories for side-by-side TUI comparison.
async fn run_compare(dir_a: &str, dir_b: &str) -> color_eyre::Result<()> {
    let path_a = std::path::Path::new(dir_a);
    let path_b = std::path::Path::new(dir_b);

    let (_meta_a, snaps_a) =
        runner::batch::load_run_dir(path_a).map_err(|e| eyre!("failed to load {dir_a}: {e}"))?;
    let (_meta_b, snaps_b) =
        runner::batch::load_run_dir(path_b).map_err(|e| eyre!("failed to load {dir_b}: {e}"))?;

    if snaps_a.is_empty() || snaps_b.is_empty() {
        eprintln!("phasma: both directories must contain snapshots");
        return Ok(());
    }

    let cfg_a = std::path::Path::new(dir_a)
        .join("config.toml")
        .exists()
        .then(|| config::load(&format!("{dir_a}/config.toml")).ok())
        .flatten();
    let cfg_b = std::path::Path::new(dir_b)
        .join("config.toml")
        .exists()
        .then(|| config::load(&format!("{dir_b}/config.toml")).ok())
        .flatten();

    let prov_a = data::playback::PlaybackDataProvider::new(snaps_a, cfg_a);
    let prov_b = data::playback::PlaybackDataProvider::new(snaps_b, cfg_b);
    let provider = data::comparison::ComparisonDataProvider::new(prov_a, prov_b);

    let mut app = App::new_with_comparison(4.0, 60.0, provider)?;
    app.run().await?;
    Ok(())
}

/// Monitor mode: watch a batch job's output directory for new snapshots.
async fn run_monitor(dir: &str) -> color_eyre::Result<()> {
    eprintln!("phasma monitor: watching {dir} for new snapshots...");
    let monitor = runner::monitor::MonitorHandle::spawn(std::path::PathBuf::from(dir));

    let mut app = App::new(4.0, 60.0, None, false)?;
    app.set_monitor_handle(monitor);
    app.run().await?;
    Ok(())
}

/// Tail mode: like monitor but auto-advances.
async fn run_tail(dir: &str) -> color_eyre::Result<()> {
    eprintln!("phasma tail: watching {dir} (auto-advance)...");
    let monitor = runner::monitor::MonitorHandle::spawn(std::path::PathBuf::from(dir));
    let mut app = App::new(4.0, 60.0, None, false)?;
    app.set_monitor_handle(monitor);
    app.run().await?;
    Ok(())
}
