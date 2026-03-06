use super::PhasmaConfig;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

pub fn validate(cfg: &PhasmaConfig) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // spatial_resolution must be power-of-2 for FFT Poisson
    if cfg.solver.poisson.starts_with("fft") {
        if !is_power_of_two(cfg.domain.spatial_resolution) {
            errors.push(ValidationError {
                field: "domain.spatial_resolution".to_string(),
                message: format!(
                    "{} is not a power of 2 (required for FFT Poisson)",
                    cfg.domain.spatial_resolution
                ),
            });
        }
        if !is_power_of_two(cfg.domain.velocity_resolution) {
            errors.push(ValidationError {
                field: "domain.velocity_resolution".to_string(),
                message: format!(
                    "{} is not a power of 2 (required for FFT Poisson)",
                    cfg.domain.velocity_resolution
                ),
            });
        }
    }

    // velocity_extent should cover escape velocity for equilibrium models
    let model = cfg.model.model_type.as_str();
    if matches!(model, "plummer" | "hernquist" | "nfw" | "king") {
        let g = cfg.domain.gravitational_constant;
        let m = cfg.model.total_mass;
        let a = cfg.model.scale_radius;
        let v_esc = (2.0 * g * m / a).sqrt();
        if cfg.domain.velocity_extent < v_esc * 1.2 {
            errors.push(ValidationError {
                field: "domain.velocity_extent".to_string(),
                message: format!(
                    "{:.3} may be too small (escape velocity ≈ {:.3}, recommend ≥ {:.3})",
                    cfg.domain.velocity_extent,
                    v_esc,
                    v_esc * 1.5
                ),
            });
        }
    }

    // memory estimate for uniform grid
    if cfg.solver.representation == "uniform" {
        let nx = cfg.domain.spatial_resolution as u64;
        let nv = cfg.domain.velocity_resolution as u64;
        let gb = 8 * nx.pow(3) * nv.pow(3) / 1_000_000_000;
        if gb as f64 > cfg.performance.memory_budget_gb {
            errors.push(ValidationError {
                field: "domain.resolution".to_string(),
                message: format!(
                    "Estimated grid memory ~{gb} GB exceeds budget ({:.1} GB)",
                    cfg.performance.memory_budget_gb
                ),
            });
        }
    }

    // SLAR requires HT representation
    if cfg.solver.advection == "slar"
        && cfg.solver.representation != "hierarchical_tucker"
    {
        errors.push(ValidationError {
            field: "solver.advection".to_string(),
            message: "SLAR advection requires representation = \"hierarchical_tucker\"".to_string(),
        });
    }

    // custom_file path check
    if let Some(cf) = &cfg.model.custom_file {
        if !std::path::Path::new(&cf.file_path).exists() {
            errors.push(ValidationError {
                field: "model.custom_file.file_path".to_string(),
                message: format!("File not found: {}", cf.file_path),
            });
        }
    }

    errors
}

fn is_power_of_two(n: u32) -> bool {
    n > 0 && (n & (n - 1)) == 0
}
