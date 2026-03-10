//! Config validation — checks PhasmaConfig for common mistakes before running.

use crate::config::PhasmaConfig;

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
    if cfg.domain.spatial_extent <= 0.0 {
        warnings.push(ValidationWarning {
            field: "domain.spatial_extent".into(),
            message: "must be > 0".into(),
        });
    }
    if cfg.domain.velocity_extent <= 0.0 {
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
        _ => {}
    }

    // Solver name validity
    let valid_poisson = ["fft_periodic", "fft", "fft_isolated"];
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

    let valid_integrator = ["strang", "yoshida", "lie"];
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

    // Memory estimate
    let nx = cfg.domain.spatial_resolution as u64;
    let nv = cfg.domain.velocity_resolution as u64;
    let cells = nx * nx * nx * nv * nv * nv;
    let mem_gb = cells as f64 * 8.0 / 1e9;
    if mem_gb > cfg.performance.memory_budget_gb {
        warnings.push(ValidationWarning {
            field: "performance.memory_budget_gb".into(),
            message: format!(
                "estimated memory {mem_gb:.1} GB exceeds budget {:.1} GB ({}^3 x {}^3 grid)",
                cfg.performance.memory_budget_gb, nx, nv
            ),
        });
    }

    // Time checks
    if cfg.time.t_final <= 0.0 {
        warnings.push(ValidationWarning {
            field: "time.t_final".into(),
            message: "must be > 0".into(),
        });
    }
    if cfg.time.cfl_factor <= 0.0 || cfg.time.cfl_factor > 1.0 {
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

    // Playback checks
    if cfg.playback.fps <= 0.0 {
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
