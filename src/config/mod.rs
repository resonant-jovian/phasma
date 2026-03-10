// Full §1 schema — native serde f64 (no two-stage string deserialization).
// rust_decimal is used only when constructing caustic domain objects in runner/live.rs.

pub mod defaults;
pub mod history;
pub mod presets;
pub mod validate;

use serde::{Deserialize, Serialize};

// ── Top-level ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PhasmaConfig {
    #[serde(default)]
    pub domain: DomainConfig,
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub solver: SolverConfig,
    #[serde(default)]
    pub time: TimeConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub exit: ExitConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub playback: PlaybackConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

// ── Domain ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DomainConfig {
    pub spatial_extent: f64,
    pub velocity_extent: f64,
    pub spatial_resolution: u32,
    pub velocity_resolution: u32,
    /// "periodic|truncated", "periodic|open", "isolated|truncated", etc.
    pub boundary: String,
    pub coordinates: String,
    pub gravitational_constant: f64,
}

impl Default for DomainConfig {
    fn default() -> Self {
        Self {
            spatial_extent: 10.0,
            velocity_extent: 5.0,
            spatial_resolution: 8,
            velocity_resolution: 8,
            boundary: "periodic|truncated".to_string(),
            coordinates: "cartesian".to_string(),
            gravitational_constant: 1.0,
        }
    }
}

// ── Model ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    #[serde(rename = "type", default = "default_model_type")]
    pub model_type: String,
    #[serde(default = "default_mass", alias = "mass")]
    pub total_mass: f64,
    #[serde(default = "default_scale_radius")]
    pub scale_radius: f64,
    // Optional sub-models
    #[serde(default)]
    pub king: Option<KingModelConfig>,
    #[serde(default)]
    pub nfw: Option<NfwModelConfig>,
    #[serde(default)]
    pub zeldovich: Option<ZeldovichConfig>,
    #[serde(default)]
    pub merger: Option<MergerConfig>,
    #[serde(default)]
    pub uniform_perturbation: Option<PerturbationConfig>,
    #[serde(default)]
    pub custom_function: Option<CustomFunctionConfig>,
    #[serde(default)]
    pub custom_file: Option<CustomFileConfig>,
}

fn default_model_type() -> String {
    "plummer".to_string()
}
fn default_mass() -> f64 {
    1.0
}
fn default_scale_radius() -> f64 {
    1.0
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_type: default_model_type(),
            total_mass: default_mass(),
            scale_radius: default_scale_radius(),
            king: None,
            nfw: None,
            zeldovich: None,
            merger: None,
            uniform_perturbation: None,
            custom_function: None,
            custom_file: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KingModelConfig {
    pub w0: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NfwModelConfig {
    pub concentration: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZeldovichConfig {
    pub amplitude: f64,
    pub wave_number: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MergerConfig {
    pub separation: f64,
    pub mass_ratio: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerturbationConfig {
    pub mode_m: u32,
    pub amplitude: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomFunctionConfig {
    pub library_path: String,
    pub function_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomFileConfig {
    pub file_path: String,
    pub format: String,
}

// ── Solver ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SolverConfig {
    #[serde(default = "default_representation")]
    pub representation: String,
    #[serde(default = "default_poisson")]
    pub poisson: String,
    #[serde(default = "default_advection")]
    pub advection: String,
    #[serde(default = "default_splitting")]
    pub integrator: String,
}

fn default_representation() -> String {
    "uniform".to_string()
}
fn default_poisson() -> String {
    "fft_periodic".to_string()
}
fn default_advection() -> String {
    "semi_lagrangian".to_string()
}
fn default_splitting() -> String {
    "strang".to_string()
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            representation: default_representation(),
            poisson: default_poisson(),
            advection: default_advection(),
            integrator: default_splitting(),
        }
    }
}

// ── Time ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimeConfig {
    #[serde(default = "default_t_final")]
    pub t_final: f64,
    /// "adaptive" or "fixed"
    #[serde(default = "default_dt_mode", alias = "dt")]
    pub dt_mode: String,
    #[serde(default = "default_dt_fixed")]
    pub dt_fixed: f64,
    #[serde(default = "default_cfl_factor")]
    pub cfl_factor: f64,
    #[serde(default = "default_dt_min")]
    pub dt_min: f64,
    #[serde(default = "default_dt_max")]
    pub dt_max: f64,
}

fn default_t_final() -> f64 {
    10.0
}
fn default_dt_mode() -> String {
    "adaptive".to_string()
}
fn default_dt_fixed() -> f64 {
    0.1
}
fn default_cfl_factor() -> f64 {
    0.5
}
fn default_dt_min() -> f64 {
    1e-6
}
fn default_dt_max() -> f64 {
    1.0
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            t_final: default_t_final(),
            dt_mode: default_dt_mode(),
            dt_fixed: default_dt_fixed(),
            cfl_factor: default_cfl_factor(),
            dt_min: default_dt_min(),
            dt_max: default_dt_max(),
        }
    }
}

// ── Output ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    #[serde(default = "default_output_dir")]
    pub directory: String,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde(default = "default_snapshot_interval", alias = "interval")]
    pub snapshot_interval: f64,
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval: f64,
    #[serde(default = "default_diagnostics_interval")]
    pub diagnostics_interval: f64,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_output_dir() -> String {
    "output".to_string()
}
fn default_prefix() -> String {
    "run".to_string()
}
fn default_snapshot_interval() -> f64 {
    1.0
}
fn default_checkpoint_interval() -> f64 {
    10.0
}
fn default_diagnostics_interval() -> f64 {
    0.1
}
fn default_format() -> String {
    "binary".to_string()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            directory: default_output_dir(),
            prefix: default_prefix(),
            snapshot_interval: default_snapshot_interval(),
            checkpoint_interval: default_checkpoint_interval(),
            diagnostics_interval: default_diagnostics_interval(),
            format: default_format(),
        }
    }
}

// ── Exit ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExitConfig {
    #[serde(default = "default_energy_drift_tol", alias = "energy_tolerance")]
    pub energy_drift_tolerance: f64,
    #[serde(default = "default_mass_drift_tol", alias = "mass_threshold")]
    pub mass_drift_tolerance: f64,
    #[serde(default)]
    pub virial_equilibrium: bool,
    #[serde(default = "default_virial_tol")]
    pub virial_tolerance: f64,
    #[serde(default)]
    pub wall_clock_limit: Option<f64>,
    #[serde(default)]
    pub rank_saturation: bool,
    #[serde(default = "default_rank_sat_steps")]
    pub rank_saturation_steps: u32,
    #[serde(default = "default_true")]
    pub cfl_violation: bool,
    #[serde(default)]
    pub steady_state: bool,
    #[serde(default = "default_steady_state_tol")]
    pub steady_state_tolerance: f64,
}

fn default_energy_drift_tol() -> f64 {
    0.5
}
fn default_mass_drift_tol() -> f64 {
    0.1
}
fn default_virial_tol() -> f64 {
    0.05
}
fn default_rank_sat_steps() -> u32 {
    5
}
fn default_true() -> bool {
    true
}
fn default_steady_state_tol() -> f64 {
    1e-6
}

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            energy_drift_tolerance: default_energy_drift_tol(),
            mass_drift_tolerance: default_mass_drift_tol(),
            virial_equilibrium: false,
            virial_tolerance: default_virial_tol(),
            wall_clock_limit: None,
            rank_saturation: false,
            rank_saturation_steps: default_rank_sat_steps(),
            cfl_violation: true,
            steady_state: false,
            steady_state_tolerance: default_steady_state_tol(),
        }
    }
}

// ── Performance ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_num_threads")]
    pub num_threads: u32,
    #[serde(default = "default_memory_budget")]
    pub memory_budget_gb: f64,
    #[serde(default = "default_rank_budget_warn")]
    pub rank_budget_warn: u32,
    #[serde(default = "default_true")]
    pub simd: bool,
    #[serde(default = "default_allocator")]
    pub allocator: String,
}

fn default_num_threads() -> u32 {
    0
} // 0 = use all available
fn default_memory_budget() -> f64 {
    4.0
}
fn default_rank_budget_warn() -> u32 {
    64
}
fn default_allocator() -> String {
    "system".to_string()
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            num_threads: default_num_threads(),
            memory_budget_gb: default_memory_budget(),
            rank_budget_warn: default_rank_budget_warn(),
            simd: true,
            allocator: default_allocator(),
        }
    }
}

// ── Playback ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PlaybackConfig {
    #[serde(default)]
    pub source_directory: Option<String>,
    #[serde(default)]
    pub source_prefix: Option<String>,
    #[serde(default = "default_format")]
    pub source_format: String,
    #[serde(default = "default_fps")]
    pub fps: f64,
    #[serde(default)]
    pub loop_playback: bool,
    #[serde(default)]
    pub start_time: Option<f64>,
    #[serde(default)]
    pub end_time: Option<f64>,
}

fn default_fps() -> f64 {
    10.0
}

// ── Appearance ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppearanceConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_colormap")]
    pub colormap_default: String,
    #[serde(default = "default_true")]
    pub braille_density: bool,
    #[serde(default = "default_border_style")]
    pub border_style: String,
    #[serde(default = "default_true")]
    pub square_pixels: bool,
    #[serde(default = "default_aspect_mode")]
    pub aspect_ratio_mode: String,
    #[serde(default = "default_cell_aspect")]
    pub cell_aspect_ratio: f64,
    #[serde(default = "default_min_columns")]
    pub min_columns: u16,
    #[serde(default = "default_min_rows")]
    pub min_rows: u16,
}

fn default_theme() -> String {
    "dark".to_string()
}
fn default_colormap() -> String {
    "viridis".to_string()
}
fn default_border_style() -> String {
    "rounded".to_string()
}
fn default_aspect_mode() -> String {
    "letterbox".to_string()
}
fn default_cell_aspect() -> f64 {
    0.5
}
fn default_min_columns() -> u16 {
    80
}
fn default_min_rows() -> u16 {
    24
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            colormap_default: default_colormap(),
            braille_density: true,
            border_style: default_border_style(),
            square_pixels: true,
            aspect_ratio_mode: default_aspect_mode(),
            cell_aspect_ratio: default_cell_aspect(),
            min_columns: default_min_columns(),
            min_rows: default_min_rows(),
        }
    }
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

/// Load a PhasmaConfig from a TOML file.
pub fn load(path: &str) -> anyhow::Result<PhasmaConfig> {
    let s = std::fs::read_to_string(path)?;
    let cfg: PhasmaConfig = toml::from_str(&s)?;
    Ok(cfg)
}
