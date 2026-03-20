//! Config validation — checks PhasmaConfig for common mistakes before running.

use crate::config::PhasmaConfig;
use rust_decimal::Decimal;

#[derive(Debug)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Validate a PhasmaConfig, returning warnings/errors.
pub fn validate(cfg: &PhasmaConfig) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    // Domain checks
    if cfg.domain.spatial_resolution == 0 {
        warnings.push(ValidationWarning {
            field: "domain.spatial_resolution".into(),
            message: "must be > 0".into(),
        });
    }
    if cfg.domain.velocity_resolution == 0 {
        warnings.push(ValidationWarning {
            field: "domain.velocity_resolution".into(),
            message: "must be > 0".into(),
        });
    }
    if cfg.domain.spatial_extent <= Decimal::ZERO {
        warnings.push(ValidationWarning {
            field: "domain.spatial_extent".into(),
            message: "must be > 0".into(),
        });
    }
    if cfg.domain.velocity_extent <= Decimal::ZERO {
        warnings.push(ValidationWarning {
            field: "domain.velocity_extent".into(),
            message: "must be > 0".into(),
        });
    }

    // Model sub-config presence
    match cfg.model.model_type.as_str() {
        "king" if cfg.model.king.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "king model requires [model.king] section with w0".into(),
            });
        }
        "nfw" if cfg.model.nfw.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "nfw model requires [model.nfw] section with concentration".into(),
            });
        }
        "zeldovich" if cfg.model.zeldovich.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "zeldovich model requires [model.zeldovich] section".into(),
            });
        }
        "merger" if cfg.model.merger.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "merger model requires [model.merger] section".into(),
            });
        }
        "custom_file" if cfg.model.custom_file.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "custom_file model requires [model.custom_file] section".into(),
            });
        }
        "uniform_perturbation" if cfg.model.uniform_perturbation.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "uniform_perturbation requires [model.uniform_perturbation] section"
                    .into(),
            });
        }
        "disk_exponential" | "disk_stability" if cfg.model.disk.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "disk model requires [model.disk] section".into(),
            });
        }
        "tidal" if cfg.model.tidal.is_none() => {
            warnings.push(ValidationWarning {
                field: "model".into(),
                message: "tidal model requires [model.tidal] section".into(),
            });
        }
        _ => {}
    }

    // Model type validity
    let valid_models = [
        "plummer",
        "hernquist",
        "king",
        "nfw",
        "zeldovich",
        "merger",
        "two_body_merger",
        "uniform_perturbation",
        "disk_exponential",
        "disk_stability",
        "tidal",
        "custom_function",
        "custom_file",
    ];
    if !valid_models.contains(&cfg.model.model_type.as_str()) {
        warnings.push(ValidationWarning {
            field: "model.type".into(),
            message: format!(
                "unknown model type '{}'; valid: {}",
                cfg.model.model_type,
                valid_models.join(", ")
            ),
        });
    }
    // disk_exponential is a legacy alias — suggest disk_stability instead
    if cfg.model.model_type == "disk_exponential" {
        warnings.push(ValidationWarning {
            field: "model.type".into(),
            message: "consider using 'disk_stability' instead of 'disk_exponential'".into(),
        });
    }

    // Solver name validity
    let valid_repr = [
        "uniform",
        "uniform_grid",
        "hierarchical_tucker",
        "ht",
        "tensor_train",
        "sheet_tracker",
        "spectral",
        "velocity_ht",
        "amr",
        "hybrid",
    ];
    if !valid_repr.contains(&cfg.solver.representation.as_str()) {
        warnings.push(ValidationWarning {
            field: "solver.representation".into(),
            message: format!(
                "unknown representation '{}'; valid: {}",
                cfg.solver.representation,
                valid_repr.join(", ")
            ),
        });
    }

    let valid_poisson = [
        "fft_periodic",
        "fft",
        "fft_isolated",
        "tensor",
        "tensor_poisson",
        "multigrid",
        "spherical",
        "spherical_harmonics",
        "tree",
        "barnes_hut",
        "vgf",
        "vgf_isolated",
    ];
    if !valid_poisson.contains(&cfg.solver.poisson.as_str()) {
        warnings.push(ValidationWarning {
            field: "solver.poisson".into(),
            message: format!(
                "unknown poisson solver '{}'; valid: {}",
                cfg.solver.poisson,
                valid_poisson.join(", ")
            ),
        });
    }

    let valid_advection = ["semi_lagrangian", "spectral", "slar"];
    if !valid_advection.contains(&cfg.solver.advection.as_str()) {
        warnings.push(ValidationWarning {
            field: "solver.advection".into(),
            message: format!(
                "unknown advection '{}'; valid: {}",
                cfg.solver.advection,
                valid_advection.join(", ")
            ),
        });
    }

    let valid_integrator = [
        "strang",
        "yoshida",
        "lie",
        "strang_splitting",
        "yoshida_splitting",
        "unsplit",
        "unsplit_rk2",
        "unsplit_rk3",
        "unsplit_rk4",
        "rkei",
        "bug",
        "midpoint_bug",
        "conservative_bug",
    ];
    if !valid_integrator.contains(&cfg.solver.integrator.as_str()) {
        warnings.push(ValidationWarning {
            field: "solver.integrator".into(),
            message: format!(
                "unknown integrator '{}'; valid: {}",
                cfg.solver.integrator,
                valid_integrator.join(", ")
            ),
        });
    }

    // Resolution must be power of 2 for FFT-based solvers
    let is_fft = cfg.solver.poisson.starts_with("fft") || cfg.solver.poisson.starts_with("vgf");
    if is_fft && !cfg.domain.spatial_resolution.is_power_of_two() {
        warnings.push(ValidationWarning {
            field: "domain.spatial_resolution".into(),
            message: format!(
                "{} requires power-of-2 resolution (got {})",
                cfg.solver.poisson, cfg.domain.spatial_resolution
            ),
        });
    }

    // SLAR requires hierarchical_tucker
    if cfg.solver.advection == "slar"
        && !matches!(
            cfg.solver.representation.as_str(),
            "hierarchical_tucker" | "ht"
        )
    {
        warnings.push(ValidationWarning {
            field: "solver.advection".into(),
            message: "slar advection requires hierarchical_tucker representation".into(),
        });
    }

    // LoMaC works with any representation but is most useful with HT
    let valid_conservation = ["none", "lomac", "standard_svd", "macro_micro"];
    if !valid_conservation.contains(&cfg.solver.conservation.as_str()) {
        warnings.push(ValidationWarning {
            field: "solver.conservation".into(),
            message: format!(
                "unknown conservation '{}'; valid: {}",
                cfg.solver.conservation,
                valid_conservation.join(", ")
            ),
        });
    }

    // Memory estimate — use the detailed breakdown, not naive full-grid
    let breakdown = super::defaults::estimate_memory_breakdown(cfg);
    let mem_gb = breakdown.total_mb() / 1000.0;
    if mem_gb > cfg.performance.memory_budget_gb {
        warnings.push(ValidationWarning {
            field: "performance.memory_budget_gb".into(),
            message: format!(
                "estimated peak memory {mem_gb:.1} GB exceeds budget {:.1} GB \
                 (resident {:.1} GB + transient {:.1} GB)",
                cfg.performance.memory_budget_gb,
                breakdown.resident_mb() / 1000.0,
                breakdown.peak_transient_mb() / 1000.0,
            ),
        });
    }

    // Time checks
    if cfg.time.t_final <= Decimal::ZERO {
        warnings.push(ValidationWarning {
            field: "time.t_final".into(),
            message: "must be > 0".into(),
        });
    }
    if cfg.time.cfl_factor <= Decimal::ZERO || cfg.time.cfl_factor > Decimal::ONE {
        warnings.push(ValidationWarning {
            field: "time.cfl_factor".into(),
            message: "should be in (0, 1]".into(),
        });
    }

    // Appearance checks
    let valid_themes = ["dark", "light", "solarized", "gruvbox"];
    if !valid_themes.contains(&cfg.appearance.theme.as_str()) {
        warnings.push(ValidationWarning {
            field: "appearance.theme".into(),
            message: format!(
                "unknown theme '{}'; valid: {}",
                cfg.appearance.theme,
                valid_themes.join(", ")
            ),
        });
    }

    if cfg.appearance.cell_aspect_ratio <= 0.0 || cfg.appearance.cell_aspect_ratio > 2.0 {
        warnings.push(ValidationWarning {
            field: "appearance.cell_aspect_ratio".into(),
            message: "should be in (0, 2]; typical values: 0.5 (default), 0.45-0.55".into(),
        });
    }

    let valid_aspect_modes = ["letterbox", "stretch", "crop"];
    if !valid_aspect_modes.contains(&cfg.appearance.aspect_ratio_mode.as_str()) {
        warnings.push(ValidationWarning {
            field: "appearance.aspect_ratio_mode".into(),
            message: format!(
                "unknown mode '{}'; valid: {}",
                cfg.appearance.aspect_ratio_mode,
                valid_aspect_modes.join(", ")
            ),
        });
    }

    // Playback checks — only validate if playback is explicitly configured
    if cfg.playback.source_directory.is_some() && cfg.playback.fps <= 0.0 {
        warnings.push(ValidationWarning {
            field: "playback.fps".into(),
            message: "must be > 0".into(),
        });
    }
    if let (Some(start), Some(end)) = (cfg.playback.start_time, cfg.playback.end_time)
        && start >= end
    {
        warnings.push(ValidationWarning {
            field: "playback".into(),
            message: format!("start_time ({start}) must be < end_time ({end})"),
        });
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PhasmaConfig;
    use rust_decimal::Decimal;

    fn has_warning(warnings: &[ValidationWarning], field: &str) -> bool {
        warnings.iter().any(|w| w.field == field)
    }

    #[test]
    fn valid_default() {
        let cfg = PhasmaConfig::default();
        let warnings = validate(&cfg);
        // Default config should only potentially warn about memory
        let non_memory: Vec<_> = warnings
            .iter()
            .filter(|w| w.field != "performance.memory_budget_gb")
            .collect();
        assert!(non_memory.is_empty(), "unexpected warnings: {non_memory:?}");
    }

    #[test]
    fn zero_spatial_res() {
        let mut cfg = PhasmaConfig::default();
        cfg.domain.spatial_resolution = 0;
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "domain.spatial_resolution"));
    }

    #[test]
    fn zero_velocity_res() {
        let mut cfg = PhasmaConfig::default();
        cfg.domain.velocity_resolution = 0;
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "domain.velocity_resolution"));
    }

    #[test]
    fn negative_spatial_extent() {
        let mut cfg = PhasmaConfig::default();
        cfg.domain.spatial_extent = Decimal::from_f64_retain(-1.0).unwrap();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "domain.spatial_extent"));
    }

    #[test]
    fn negative_velocity_extent() {
        let mut cfg = PhasmaConfig::default();
        cfg.domain.velocity_extent = Decimal::from_f64_retain(-1.0).unwrap();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "domain.velocity_extent"));
    }

    #[test]
    fn invalid_model_type() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "bogus".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "model.type"));
    }

    #[test]
    fn invalid_representation() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.representation = "bogus".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "solver.representation"));
    }

    #[test]
    fn invalid_poisson() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.poisson = "bogus".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "solver.poisson"));
    }

    #[test]
    fn invalid_integrator() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.integrator = "bogus".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "solver.integrator"));
    }

    #[test]
    fn invalid_advection() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.advection = "bogus".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "solver.advection"));
    }

    #[test]
    fn invalid_conservation() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.conservation = "bogus".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "solver.conservation"));
    }

    #[test]
    fn fft_non_power_of_two() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.poisson = "fft_periodic".to_string();
        cfg.domain.spatial_resolution = 12;
        let warnings = validate(&cfg);
        assert!(
            warnings.iter().any(|w| w.field == "domain.spatial_resolution" && w.message.contains("power-of-2")),
            "should warn about non-power-of-2 for FFT"
        );
    }

    #[test]
    fn fft_power_of_two_ok() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.poisson = "fft_periodic".to_string();
        cfg.domain.spatial_resolution = 16;
        let warnings = validate(&cfg);
        assert!(
            !warnings.iter().any(|w| w.field == "domain.spatial_resolution" && w.message.contains("power-of-2")),
            "should not warn about power-of-2 for N=16"
        );
    }

    #[test]
    fn slar_without_ht() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.advection = "slar".to_string();
        cfg.solver.representation = "uniform".to_string();
        let warnings = validate(&cfg);
        assert!(
            warnings.iter().any(|w| w.field == "solver.advection" && w.message.contains("hierarchical_tucker")),
            "should warn that slar requires HT"
        );
    }

    #[test]
    fn slar_with_ht_ok() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.advection = "slar".to_string();
        cfg.solver.representation = "ht".to_string();
        let warnings = validate(&cfg);
        assert!(
            !warnings.iter().any(|w| w.field == "solver.advection" && w.message.contains("hierarchical_tucker")),
            "should not warn about slar with HT representation"
        );
    }

    #[test]
    fn zero_t_final() {
        let mut cfg = PhasmaConfig::default();
        cfg.time.t_final = Decimal::ZERO;
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "time.t_final"));
    }

    #[test]
    fn cfl_above_one() {
        let mut cfg = PhasmaConfig::default();
        cfg.time.cfl_factor = Decimal::from_f64_retain(1.5).unwrap();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "time.cfl_factor"));
    }

    #[test]
    fn invalid_theme() {
        let mut cfg = PhasmaConfig::default();
        cfg.appearance.theme = "neon".to_string();
        let warnings = validate(&cfg);
        assert!(has_warning(&warnings, "appearance.theme"));
    }

    #[test]
    fn king_without_section() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "king".to_string();
        cfg.model.king = None;
        let warnings = validate(&cfg);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "model" && w.message.contains("king")),
            "should warn about missing king section"
        );
    }

    #[test]
    fn nfw_without_section() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "nfw".to_string();
        cfg.model.nfw = None;
        let warnings = validate(&cfg);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "model" && w.message.contains("nfw")),
            "should warn about missing nfw section"
        );
    }

    #[test]
    fn zeldovich_without_section() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "zeldovich".to_string();
        cfg.model.zeldovich = None;
        let warnings = validate(&cfg);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "model" && w.message.contains("zeldovich")),
            "should warn about missing zeldovich section"
        );
    }

    #[test]
    fn merger_without_section() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "merger".to_string();
        cfg.model.merger = None;
        let warnings = validate(&cfg);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "model" && w.message.contains("merger")),
            "should warn about missing merger section"
        );
    }
}
