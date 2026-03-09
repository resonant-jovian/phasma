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
        Supported models: Plummer, Hernquist, King, NFW, Zeldovich, Merger, custom.\n\
        Poisson solvers: fft_periodic, fft_isolated.\n\
        Time integrators: strang (2nd-order), yoshida (4th-order), lie (1st-order).",
)]
pub struct Cli {
    /// Path to a simulation configuration file (TOML format)
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<String>,

    /// Start the simulation immediately after loading the config
    #[arg(long)]
    pub run: bool,

    /// Headless batch mode — run without TUI, print progress to stderr (for HPC / SLURM)
    #[arg(long)]
    pub batch: bool,

    /// Replay recorded snapshots from a previous run directory
    #[arg(long, value_name = "DIR")]
    pub playback: Option<String>,

    /// Side-by-side comparison of two simulation output directories
    #[arg(long, value_name = "DIR", num_args = 2)]
    pub compare: Option<Vec<String>>,

    /// Parameter sweep mode — vary parameters across a grid
    #[arg(long, value_name = "TOML")]
    pub sweep: Option<String>,

    /// Convergence study — run at increasing resolutions
    #[arg(long, value_name = "TOML")]
    pub convergence: Option<String>,

    /// CI-compatible regression test against a reference directory
    #[arg(long, value_name = "DIR")]
    pub regression_test: Option<String>,

    /// Monitor a running batch job by watching its output directory
    #[arg(long, value_name = "DIR")]
    pub monitor: Option<String>,

    /// Tail a recording directory, auto-advancing as new snapshots appear
    #[arg(long, value_name = "PATH")]
    pub tail: Option<String>,

    /// Launch the guided first-run wizard
    #[arg(long)]
    pub wizard: bool,

    /// Save the current configuration as a named preset
    #[arg(long, value_name = "NAME")]
    pub save_preset: Option<String>,

    /// Batch comparison report across multiple run directories
    #[arg(long, value_name = "DIR", num_args = 2..)]
    pub batch_compare: Option<Vec<String>>,

    /// Output path for batch comparison report
    #[arg(long, value_name = "PATH")]
    pub report: Option<String>,

    /// Generate a man page and print it to stdout
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
