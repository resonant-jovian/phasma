use clap::{CommandFactory, Parser};

use crate::tui::config::{get_config_dir, get_data_dir};

#[derive(Parser, Debug)]
#[command(
    author,
    version = short_version(),
    long_version = long_version(),
    about = "Terminal interface for the caustic Vlasov-Poisson solver",
    long_about = "phasma is a terminal application built with ratatui that provides an interactive \
        workflow for setting up, running, and monitoring 6D Vlasov-Poisson simulations \
        using the caustic solver library. It runs entirely in the terminal — over SSH, \
        in tmux, on headless compute nodes.\n\n\
        Without arguments, phasma launches the interactive TUI on the Setup tab (F1), \
        where you can browse and load TOML configuration files.\n\n\
        MODES OF OPERATION:\n\
        Interactive TUI (default), batch (--batch), playback (--playback), \
        comparison (--compare), monitor (--monitor/--tail), parameter sweep (--sweep), \
        convergence study (--convergence), regression test (--regression-test), \
        batch comparison report (--batch-compare).\n\n\
        Supported models: plummer, hernquist, king, nfw, zeldovich, merger, custom_file.\n\
        Poisson solvers: fft_periodic (alias fft), fft_isolated.\n\
        Time integrators: strang (2nd-order), yoshida (4th-order), lie (1st-order).",
)]
pub struct Cli {
    /// Path to a simulation configuration file (TOML format).
    /// Required for --run, --batch, --save-preset. Optional for TUI mode.
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<String>,

    /// Start the simulation immediately after loading the config
    #[arg(long)]
    pub run: bool,

    /// Headless batch mode — run without TUI, save output to disk.
    /// Writes diagnostics.csv, periodic JSON snapshots, and metadata.json
    /// to a timestamped directory under the configured output path.
    /// Progress is printed to stderr. Suitable for HPC / SLURM jobs.
    #[arg(long)]
    pub batch: bool,

    /// Replay recorded snapshots from a batch output directory in the TUI.
    /// Loads all snapshots from DIR/snapshots/ and plays them back with
    /// scrubbing support (Left/Right to step, Space to play/pause).
    #[arg(long, value_name = "DIR")]
    pub playback: Option<String>,

    /// Side-by-side TUI comparison of two simulation output directories.
    /// Press 'c' to cycle between Run A, Run B, and Difference views
    /// on the density and phase-space tabs.
    #[arg(long, value_name = "DIR", num_args = 2)]
    pub compare: Option<Vec<String>>,

    /// Parameter sweep — run a batch simulation for each combination in a
    /// Cartesian product of parameter values. Takes a TOML file specifying
    /// the base config, parameters to vary, and their values.
    #[arg(long, value_name = "TOML")]
    pub sweep: Option<String>,

    /// Convergence study — run the same simulation at increasing spatial
    /// resolutions and compute convergence rates. Takes a TOML file
    /// specifying the base config and list of resolutions.
    #[arg(long, value_name = "TOML")]
    pub convergence: Option<String>,

    /// CI-compatible regression test against a reference batch output
    /// directory. Re-runs the saved config and compares energy drift
    /// and mass conservation against the reference. Exits 0 on pass, 1 on fail.
    #[arg(long, value_name = "DIR")]
    pub regression_test: Option<String>,

    /// Monitor a running batch job by watching its output directory for
    /// new snapshot files. Opens the TUI and updates live as new data appears.
    #[arg(long, value_name = "DIR")]
    pub monitor: Option<String>,

    /// Tail mode — like --monitor but always shows the latest snapshot,
    /// auto-advancing as new files appear.
    #[arg(long, value_name = "DIR")]
    pub tail: Option<String>,

    /// Launch the guided first-run wizard. Interactively prompts for model
    /// type, domain parameters, solver settings, and writes a TOML config file.
    #[arg(long)]
    pub wizard: bool,

    /// Save the loaded config (from --config) as a named preset to
    /// ~/.config/phasma/presets/NAME.toml for quick reuse.
    #[arg(long, value_name = "NAME")]
    pub save_preset: Option<String>,

    /// Generate a Markdown comparison report across multiple batch output
    /// directories. Compares energy drift, mass conservation, wall time,
    /// and exit reason for each run.
    #[arg(long, value_name = "DIR", num_args = 2..)]
    pub batch_compare: Option<Vec<String>>,

    /// Output file path for --batch-compare report (default: comparison_report.md)
    #[arg(long, value_name = "PATH")]
    pub report: Option<String>,

    /// Generate a man page (roff format) and print it to stdout.
    /// Install with: phasma --generate-man > phasma.1
    #[arg(long)]
    pub generate_man: bool,
}

impl Cli {
    /// Generate a ROFF man page from clap's command definition and write to stdout.
    pub fn print_man_page() -> std::io::Result<()> {
        let cmd = Self::command();
        let man = clap_mangen::Man::new(cmd)
            .title("PHASMA")
            .section("1")
            .manual("User Commands");
        man.render(&mut std::io::stdout())
    }
}

fn short_version() -> String {
    let ver = env!("CARGO_PKG_VERSION");
    let git = option_env!("VERGEN_GIT_DESCRIBE").unwrap_or("");
    let date = option_env!("VERGEN_BUILD_DATE").unwrap_or("unknown");
    if git.is_empty() {
        format!("{ver} ({date})")
    } else {
        format!("{ver}-{git} ({date})")
    }
}

fn long_version() -> String {
    let author = clap::crate_authors!();
    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{}

Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}",
        short_version()
    )
}
