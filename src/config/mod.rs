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
    pub logging: LoggingConfig,
    #[serde(default)]
    pub playback: PlaybackConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

// ── Domain (§1.1) ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DomainConfig {
    pub spatial_extent: f64,
    pub velocity_extent: f64,
    pub spatial_resolution: u32,
    pub velocity_resolution: u32,
    /// "periodic", "isolated", "reflecting"
    pub boundary: String,
    /// "cartesian", "spherical" (future)
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
            boundary: "periodic".to_string(),
            coordinates: "cartesian".to_string(),
            gravitational_constant: 1.0,
        }
    }
}

// ── Model (§1.2) ────────────────────────────────────────────────────────────

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
    pub disk: Option<DiskModelConfig>,
    #[serde(default)]
    pub tidal: Option<TidalConfig>,
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
            disk: None,
            tidal: None,
            custom_function: None,
            custom_file: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KingModelConfig {
    #[serde(default = "default_king_w0", alias = "concentration")]
    pub w0: f64,
    #[serde(default)]
    pub anisotropy: f64,
}

fn default_king_w0() -> f64 {
    7.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NfwModelConfig {
    #[serde(default = "default_nfw_concentration")]
    pub concentration: f64,
    #[serde(default = "default_mass")]
    pub virial_mass: f64,
    /// "isotropic", "osipkov_merritt", "constant_beta"
    #[serde(default = "default_isotropic")]
    pub velocity_anisotropy: String,
    #[serde(default)]
    pub beta: f64,
}

fn default_nfw_concentration() -> f64 {
    10.0
}
fn default_isotropic() -> String {
    "isotropic".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZeldovichConfig {
    #[serde(default = "default_zel_amplitude", alias = "perturbation_amplitude")]
    pub amplitude: f64,
    #[serde(default = "default_zel_wave_number")]
    pub wave_number: f64,
    #[serde(default = "default_zel_box_size")]
    pub box_size: f64,
    #[serde(default = "default_zel_redshift")]
    pub redshift_initial: f64,
    #[serde(default = "default_cosmology_h")]
    pub cosmology_h: f64,
    #[serde(default = "default_cosmology_omega_m")]
    pub cosmology_omega_m: f64,
    #[serde(default = "default_cosmology_omega_lambda")]
    pub cosmology_omega_lambda: f64,
}

fn default_zel_amplitude() -> f64 {
    0.01
}
fn default_zel_wave_number() -> f64 {
    1.0
}
fn default_zel_box_size() -> f64 {
    100.0
}
fn default_zel_redshift() -> f64 {
    50.0
}
fn default_cosmology_h() -> f64 {
    0.7
}
fn default_cosmology_omega_m() -> f64 {
    0.3
}
fn default_cosmology_omega_lambda() -> f64 {
    0.7
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MergerConfig {
    #[serde(default = "default_merger_separation")]
    pub separation: f64,
    #[serde(default = "default_mass_ratio")]
    pub mass_ratio: f64,
    #[serde(default)]
    pub relative_velocity: [f64; 3],
    #[serde(default = "default_impact_parameter")]
    pub impact_parameter: f64,
    #[serde(default = "default_model_type")]
    pub model_1: String,
    #[serde(default = "default_model_type")]
    pub model_2: String,
    #[serde(default = "default_scale_radius")]
    pub scale_radius_1: f64,
    #[serde(default = "default_scale_radius")]
    pub scale_radius_2: f64,
}

fn default_merger_separation() -> f64 {
    10.0
}
fn default_mass_ratio() -> f64 {
    1.0
}
fn default_impact_parameter() -> f64 {
    2.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerturbationConfig {
    #[serde(default = "default_mass", alias = "mode_m")]
    pub background_density: f64,
    #[serde(default = "default_pert_dispersion")]
    pub velocity_dispersion: f64,
    #[serde(default = "default_zel_amplitude", alias = "amplitude")]
    pub perturbation_amplitude: f64,
    #[serde(default)]
    pub perturbation_wavenumber: [f64; 3],
}

fn default_pert_dispersion() -> f64 {
    0.5
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiskModelConfig {
    #[serde(default = "default_mass")]
    pub disk_mass: f64,
    #[serde(default = "default_disk_scale_length")]
    pub disk_scale_length: f64,
    #[serde(default = "default_disk_scale_height")]
    pub disk_scale_height: f64,
    #[serde(default = "default_disk_sigma_r")]
    pub radial_velocity_dispersion: f64,
    #[serde(default = "default_model_type")]
    pub halo_type: String,
    #[serde(default = "default_halo_mass")]
    pub halo_mass: f64,
    #[serde(default = "default_nfw_concentration")]
    pub halo_concentration: f64,
    #[serde(default = "default_toomre_q")]
    pub toomre_q: f64,
}

fn default_disk_scale_length() -> f64 {
    3.0
}
fn default_disk_scale_height() -> f64 {
    0.3
}
fn default_disk_sigma_r() -> f64 {
    0.15
}
fn default_halo_mass() -> f64 {
    10.0
}
fn default_toomre_q() -> f64 {
    1.5
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TidalConfig {
    /// Progenitor model: "plummer", "hernquist", "king", "nfw"
    #[serde(default = "default_model_type")]
    pub progenitor_type: String,
    #[serde(default = "default_mass")]
    pub progenitor_mass: f64,
    #[serde(default = "default_scale_radius")]
    pub progenitor_scale_radius: f64,
    /// Host potential type: "point_mass", "nfw_fixed", "logarithmic"
    #[serde(default = "default_tidal_host")]
    pub host_type: String,
    #[serde(default = "default_halo_mass")]
    pub host_mass: f64,
    #[serde(default = "default_tidal_host_scale")]
    pub host_scale_radius: f64,
    /// Progenitor initial position [x, y, z]
    #[serde(default = "default_tidal_position")]
    pub progenitor_position: [f64; 3],
    /// Progenitor initial velocity [vx, vy, vz]
    #[serde(default)]
    pub progenitor_velocity: [f64; 3],
}

fn default_tidal_host() -> String {
    "point_mass".to_string()
}
fn default_tidal_host_scale() -> f64 {
    20.0
}
fn default_tidal_position() -> [f64; 3] {
    [5.0, 0.0, 0.0]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomFunctionConfig {
    pub library_path: String,
    pub function_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomFileConfig {
    pub file_path: String,
    #[serde(default = "default_dataset_name")]
    pub dataset: String,
    /// Backward compat alias
    #[serde(default, alias = "format")]
    pub file_format: String,
}

fn default_dataset_name() -> String {
    "distribution_function".to_string()
}

// ── Solver (§1.3) ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SolverConfig {
    /// "uniform_grid", "hierarchical_tucker", "tensor_train", "sheet_tracker", "velocity_ht"
    #[serde(default = "default_representation")]
    pub representation: String,
    /// "fft_periodic", "fft_isolated", "multigrid", "spherical_harmonics"
    #[serde(default = "default_poisson")]
    pub poisson: String,
    /// "semi_lagrangian", "spectral", "slar"
    #[serde(default = "default_advection")]
    pub advection: String,
    /// "strang_splitting", "yoshida_splitting", "lie"
    #[serde(default = "default_splitting")]
    pub integrator: String,
    /// "standard_svd", "lomac", "macro_micro", "none"
    #[serde(default = "default_conservation")]
    pub conservation: String,
    // Sub-solver configs
    #[serde(default)]
    pub ht: Option<HtSolverConfig>,
    #[serde(default)]
    pub slar: Option<SlarConfig>,
    #[serde(default)]
    pub lomac: Option<LomacConfig>,
    #[serde(default)]
    pub semi_lagrangian: Option<SemiLagrangianConfig>,
    #[serde(default)]
    pub multigrid: Option<MultigridConfig>,
    #[serde(default)]
    pub exponential_sum: Option<ExponentialSumConfig>,
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
fn default_conservation() -> String {
    "none".to_string()
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            representation: default_representation(),
            poisson: default_poisson(),
            advection: default_advection(),
            integrator: default_splitting(),
            conservation: default_conservation(),
            ht: None,
            slar: None,
            lomac: None,
            semi_lagrangian: None,
            multigrid: None,
            exponential_sum: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HtSolverConfig {
    #[serde(default = "default_ht_max_rank")]
    pub max_rank: u32,
    #[serde(default = "default_ht_initial_rank")]
    pub initial_rank: u32,
    #[serde(default = "default_ht_tolerance")]
    pub tolerance: f64,
    /// "tolerance", "fixed", "budget"
    #[serde(default = "default_ht_rank_adaptation")]
    pub rank_adaptation: String,
    /// "balanced_xv", "velocity_only", "custom"
    #[serde(default = "default_ht_dim_tree")]
    pub dimension_tree: String,
    #[serde(default)]
    pub custom_tree: Option<String>,
}

fn default_ht_max_rank() -> u32 {
    100
}
fn default_ht_initial_rank() -> u32 {
    20
}
fn default_ht_tolerance() -> f64 {
    1e-6
}
fn default_ht_rank_adaptation() -> String {
    "tolerance".to_string()
}
fn default_ht_dim_tree() -> String {
    "balanced_xv".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlarConfig {
    #[serde(default = "default_slar_order")]
    pub reconstruction_order: u32,
    #[serde(default = "default_slar_aca_tol")]
    pub aca_tolerance: f64,
    #[serde(default = "default_slar_max_aca")]
    pub max_aca_iterations: u32,
    #[serde(default = "default_slar_oversample")]
    pub oversampling_factor: f64,
}

fn default_slar_order() -> u32 {
    3
}
fn default_slar_aca_tol() -> f64 {
    1e-8
}
fn default_slar_max_aca() -> u32 {
    500
}
fn default_slar_oversample() -> f64 {
    2.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LomacConfig {
    /// "kfvs", "upwind"
    #[serde(default = "default_lomac_flux")]
    pub flux_scheme: String,
    #[serde(default = "default_lomac_tol")]
    pub projection_tolerance: f64,
}

fn default_lomac_flux() -> String {
    "kfvs".to_string()
}
fn default_lomac_tol() -> f64 {
    1e-12
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SemiLagrangianConfig {
    /// "linear", "cubic", "quintic"
    #[serde(default = "default_sl_interp")]
    pub interpolation: String,
    /// "none", "minmod", "van_leer"
    #[serde(default = "default_sl_limiter")]
    pub limiter: String,
}

fn default_sl_interp() -> String {
    "cubic".to_string()
}
fn default_sl_limiter() -> String {
    "none".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultigridConfig {
    #[serde(default = "default_mg_levels")]
    pub levels: u32,
    /// "jacobi", "gauss_seidel", "sor"
    #[serde(default = "default_mg_smoother")]
    pub smoother: String,
    #[serde(default = "default_mg_vcycles")]
    pub v_cycles: u32,
    #[serde(default = "default_mg_omega")]
    pub omega: f64,
}

fn default_mg_levels() -> u32 {
    5
}
fn default_mg_smoother() -> String {
    "gauss_seidel".to_string()
}
fn default_mg_vcycles() -> u32 {
    3
}
fn default_mg_omega() -> f64 {
    1.5
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExponentialSumConfig {
    #[serde(default = "default_exp_sum_terms")]
    pub num_terms: u32,
    #[serde(default = "default_exp_sum_accuracy")]
    pub accuracy: f64,
}

fn default_exp_sum_terms() -> u32 {
    30
}
fn default_exp_sum_accuracy() -> f64 {
    1e-8
}

// ── Time (§1.4) ─────────────────────────────────────────────────────────────

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

// ── Output (§1.5) ───────────────────────────────────────────────────────────

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
    /// "hdf5", "parquet", "binary"
    #[serde(default = "default_format")]
    pub format: String,
    // Sub-configs for format-specific settings
    #[serde(default)]
    pub hdf5: Option<Hdf5OutputConfig>,
    #[serde(default)]
    pub parquet: Option<ParquetOutputConfig>,
    // What to save in snapshots
    #[serde(default)]
    pub fields: OutputFieldsConfig,
    #[serde(default)]
    pub tensor: TensorOutputConfig,
    #[serde(default, alias = "diagnostics")]
    pub diagnostics_fields: DiagnosticsOutputConfig,
    #[serde(default, alias = "performance")]
    pub performance_fields: PerformanceOutputConfig,
    #[serde(default)]
    pub profiles: ProfilesOutputConfig,
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
            hdf5: None,
            parquet: None,
            fields: OutputFieldsConfig::default(),
            tensor: TensorOutputConfig::default(),
            diagnostics_fields: DiagnosticsOutputConfig::default(),
            performance_fields: PerformanceOutputConfig::default(),
            profiles: ProfilesOutputConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Hdf5OutputConfig {
    /// "none", "gzip", "blosc_zstd", "blosc_lz4"
    #[serde(default = "default_hdf5_compression")]
    pub compression: String,
    #[serde(default = "default_compression_level")]
    pub compression_level: u32,
    #[serde(default = "default_true")]
    pub shuffle: bool,
    /// "slice_aligned", "auto", "none"
    #[serde(default = "default_chunk_strategy")]
    pub chunk_strategy: String,
}

fn default_hdf5_compression() -> String {
    "blosc_zstd".to_string()
}
fn default_compression_level() -> u32 {
    3
}
fn default_chunk_strategy() -> String {
    "slice_aligned".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParquetOutputConfig {
    /// "none", "snappy", "zstd", "lz4"
    #[serde(default = "default_parquet_compression")]
    pub compression: String,
    #[serde(default = "default_compression_level")]
    pub compression_level: u32,
    #[serde(default = "default_row_group_size")]
    pub row_group_size: u32,
}

fn default_parquet_compression() -> String {
    "zstd".to_string()
}
fn default_row_group_size() -> u32 {
    1000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputFieldsConfig {
    #[serde(default = "default_true")]
    pub distribution_function: bool,
    #[serde(default = "default_true")]
    pub density: bool,
    #[serde(default = "default_true")]
    pub potential: bool,
    #[serde(default = "default_true")]
    pub acceleration: bool,
    #[serde(default)]
    pub mean_velocity: bool,
    #[serde(default = "default_true")]
    pub velocity_dispersion: bool,
    #[serde(default)]
    pub stream_count: bool,
}

impl Default for OutputFieldsConfig {
    fn default() -> Self {
        Self {
            distribution_function: true,
            density: true,
            potential: true,
            acceleration: true,
            mean_velocity: false,
            velocity_dispersion: true,
            stream_count: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TensorOutputConfig {
    #[serde(default = "default_true")]
    pub singular_values: bool,
    #[serde(default = "default_true")]
    pub rank_per_node: bool,
    #[serde(default = "default_true")]
    pub ht_factors: bool,
    #[serde(default = "default_true")]
    pub truncation_errors: bool,
}

impl Default for TensorOutputConfig {
    fn default() -> Self {
        Self {
            singular_values: true,
            rank_per_node: true,
            ht_factors: true,
            truncation_errors: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiagnosticsOutputConfig {
    #[serde(default = "default_true")]
    pub total_energy: bool,
    #[serde(default = "default_true")]
    pub kinetic_energy: bool,
    #[serde(default = "default_true")]
    pub potential_energy: bool,
    #[serde(default = "default_true")]
    pub total_mass: bool,
    #[serde(default = "default_true")]
    pub momentum: bool,
    #[serde(default = "default_true")]
    pub angular_momentum: bool,
    #[serde(default = "default_true")]
    pub casimir_c2: bool,
    #[serde(default = "default_true")]
    pub entropy: bool,
    #[serde(default = "default_true")]
    pub virial_ratio: bool,
    #[serde(default = "default_true")]
    pub max_density: bool,
    #[serde(default = "default_true")]
    pub density_center: bool,
    #[serde(default = "default_true")]
    pub half_mass_radius: bool,
    #[serde(default)]
    pub lagrangian_radii: Vec<f64>,
    #[serde(default = "default_true")]
    pub velocity_dispersion_profile: bool,
    #[serde(default = "default_true")]
    pub anisotropy_profile: bool,
}

impl Default for DiagnosticsOutputConfig {
    fn default() -> Self {
        Self {
            total_energy: true,
            kinetic_energy: true,
            potential_energy: true,
            total_mass: true,
            momentum: true,
            angular_momentum: true,
            casimir_c2: true,
            entropy: true,
            virial_ratio: true,
            max_density: true,
            density_center: true,
            half_mass_radius: true,
            lagrangian_radii: vec![0.1, 0.25, 0.5, 0.75, 0.9],
            velocity_dispersion_profile: true,
            anisotropy_profile: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceOutputConfig {
    #[serde(default = "default_true")]
    pub step_wall_time: bool,
    #[serde(default = "default_true")]
    pub phase_timings: bool,
    #[serde(default = "default_true")]
    pub svd_count: bool,
    #[serde(default = "default_true")]
    pub htaca_evaluations: bool,
    #[serde(default = "default_true")]
    pub peak_rank: bool,
    #[serde(default = "default_true")]
    pub memory_rss: bool,
    #[serde(default)]
    pub cache_miss_rate: bool,
    #[serde(default)]
    pub allocation_count: bool,
}

impl Default for PerformanceOutputConfig {
    fn default() -> Self {
        Self {
            step_wall_time: true,
            phase_timings: true,
            svd_count: true,
            htaca_evaluations: true,
            peak_rank: true,
            memory_rss: true,
            cache_miss_rate: false,
            allocation_count: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfilesOutputConfig {
    #[serde(default = "default_profile_bins")]
    pub num_radial_bins: u32,
    #[serde(default = "default_profile_rmin")]
    pub radial_min: f64,
    #[serde(default = "default_profile_rmax")]
    pub radial_max: f64,
    /// "linear", "log"
    #[serde(default = "default_profile_spacing")]
    pub radial_spacing: String,
}

fn default_profile_bins() -> u32 {
    100
}
fn default_profile_rmin() -> f64 {
    0.01
}
fn default_profile_rmax() -> f64 {
    15.0
}
fn default_profile_spacing() -> String {
    "log".to_string()
}

impl Default for ProfilesOutputConfig {
    fn default() -> Self {
        Self {
            num_radial_bins: default_profile_bins(),
            radial_min: default_profile_rmin(),
            radial_max: default_profile_rmax(),
            radial_spacing: default_profile_spacing(),
        }
    }
}

// ── Exit (§1.6) ─────────────────────────────────────────────────────────────

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
    /// Seconds, or None for unlimited. String "24h" parsed externally.
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
    /// Casimir C₂ = ∫f² dx³dv³ drift tolerance. 0 = disabled.
    #[serde(default)]
    pub casimir_drift_tolerance: f64,
    /// Exit when first caustic is detected (stream_count > 1).
    #[serde(default)]
    pub caustic_formation: bool,
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
            casimir_drift_tolerance: 0.0,
            caustic_formation: false,
        }
    }
}

// ── Performance (§1.7) ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_num_threads")]
    pub num_threads: u32,
    #[serde(default = "default_memory_budget")]
    pub memory_budget_gb: f64,
    #[serde(default = "default_rank_budget_warn")]
    pub rank_budget_warn: u32,
    /// "auto", "avx2", "sse4.1", "none"
    #[serde(default = "default_simd_str")]
    pub simd: String,
    /// "system", "jemalloc", "mimalloc"
    #[serde(default = "default_allocator")]
    pub allocator: String,
    #[serde(default)]
    pub workspace_pool_size: u32,
    #[serde(default = "default_bump_arena_mb")]
    pub bump_arena_mb: u32,
    #[serde(default)]
    pub profiling: bool,
    /// "phases", "kernels", "full"
    #[serde(default = "default_profiling_detail")]
    pub profiling_detail: String,
}

fn default_num_threads() -> u32 {
    0
}
fn default_memory_budget() -> f64 {
    8.0
}
fn default_rank_budget_warn() -> u32 {
    64
}
fn default_simd_str() -> String {
    "auto".to_string()
}
fn default_allocator() -> String {
    "system".to_string()
}
fn default_bump_arena_mb() -> u32 {
    16
}
fn default_profiling_detail() -> String {
    "phases".to_string()
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            num_threads: default_num_threads(),
            memory_budget_gb: default_memory_budget(),
            rank_budget_warn: default_rank_budget_warn(),
            simd: default_simd_str(),
            allocator: default_allocator(),
            workspace_pool_size: 0,
            bump_arena_mb: default_bump_arena_mb(),
            profiling: false,
            profiling_detail: default_profiling_detail(),
        }
    }
}

// ── Logging (§1.7) ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// "trace", "debug", "info", "warn", "error"
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub structured: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: String::new(),
            structured: false,
        }
    }
}

// ── Playback (§1.8) ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PlaybackConfig {
    #[serde(default)]
    pub source_directory: Option<String>,
    #[serde(default)]
    pub source_prefix: Option<String>,
    /// "auto", "hdf5", "parquet"
    #[serde(default = "default_source_format")]
    pub source_format: String,
    #[serde(default = "default_fps")]
    pub fps: f64,
    #[serde(default, alias = "loop")]
    pub loop_playback: bool,
    #[serde(default)]
    pub start_time: Option<f64>,
    /// -1.0 = end of available data
    #[serde(default)]
    pub end_time: Option<f64>,
}

fn default_source_format() -> String {
    "auto".to_string()
}
fn default_fps() -> f64 {
    10.0
}

// ── Appearance (§1.9) ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppearanceConfig {
    /// "dark", "light", "solarized", "gruvbox"
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_colormap")]
    pub colormap_default: String,
    #[serde(default = "default_true")]
    pub braille_density: bool,
    /// "plain", "rounded", "double", "thick"
    #[serde(default = "default_border_style")]
    pub border_style: String,
    #[serde(default = "default_true")]
    pub square_pixels: bool,
    /// "letterbox", "crop"
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

/// Save a PhasmaConfig to a TOML file.
pub fn save(path: &str, cfg: &PhasmaConfig) -> anyhow::Result<()> {
    let s = toml::to_string_pretty(cfg)?;
    std::fs::write(path, s)?;
    Ok(())
}
