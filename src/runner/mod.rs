pub mod batch;
pub mod compare;
pub mod convergence;
pub mod live;
pub mod monitor;
pub mod regression;
pub mod sweep;
pub mod wizard;

use serde::{Deserialize, Serialize};

/// Metadata written to `metadata.json` in each batch output directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    pub phasma_version: String,
    pub config_path: String,
    pub output_dir: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub exit_reason: Option<String>,
    pub total_steps: u64,
    pub final_time: f64,
    pub snapshot_count: usize,
}
