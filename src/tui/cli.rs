use clap::Parser;

use crate::tui::config::{get_config_dir, get_data_dir};

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
    /// Tick rate, i.e. number of ticks per second
    #[arg(short, long, value_name = "FLOAT", default_value_t = 4.0)]
    pub tick_rate: f64,

    /// Frame rate, i.e. number of frames per second
    #[arg(short, long, value_name = "FLOAT", default_value_t = 60.0)]
    pub frame_rate: f64,

    /// Path to simulation config file (TOML)
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<String>,

    /// Start simulation immediately on launch
    #[arg(long)]
    pub run: bool,

    /// Headless batch mode — run simulation without TUI (for HPC)
    #[arg(long)]
    pub batch: bool,

    /// Playback mode — replay recorded snapshots
    #[arg(long, value_name = "DIR")]
    pub playback: Option<String>,

    /// Comparison mode — side-by-side two runs
    #[arg(long, value_name = "DIR", num_args = 2)]
    pub compare: Option<Vec<String>>,

    /// Parameter sweep mode
    #[arg(long, value_name = "TOML")]
    pub sweep: Option<String>,

    /// Convergence study mode
    #[arg(long, value_name = "TOML")]
    pub convergence: Option<String>,

    /// Regression test mode (CI-compatible)
    #[arg(long, value_name = "DIR")]
    pub regression_test: Option<String>,

    /// Monitor a running batch job
    #[arg(long, value_name = "DIR")]
    pub monitor: Option<String>,

    /// Tail a recording directory (auto-advance as snapshots appear)
    #[arg(long, value_name = "PATH")]
    pub tail: Option<String>,

    /// Guided first-run wizard
    #[arg(long)]
    pub wizard: bool,

    /// Save current config as a named preset
    #[arg(long, value_name = "NAME")]
    pub save_preset: Option<String>,

    /// Batch comparison report across multiple runs
    #[arg(long, value_name = "DIR", num_args = 2..)]
    pub batch_compare: Option<Vec<String>>,

    /// Output path for batch comparison report
    #[arg(long, value_name = "PATH")]
    pub report: Option<String>,
}

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_DESCRIBE"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

pub fn version() -> String {
    let author = clap::crate_authors!();

    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{VERSION_MESSAGE}

Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}"
    )
}
