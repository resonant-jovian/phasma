use super::PhasmaConfig;

/// Apply smart defaults based on the chosen model type.
/// Called whenever the model type changes in the Setup tab.
pub fn apply_model_defaults(cfg: &mut PhasmaConfig) {
    match cfg.model.model_type.as_str() {
        "zeldovich" => {
            cfg.domain.boundary = "periodic|truncated".to_string();
            cfg.solver.poisson = "fft_periodic".to_string();
        }
        "plummer" | "king" | "hernquist" => {
            cfg.domain.boundary = "periodic|truncated".to_string();
            // Would be "isolated" but fft_isolated is not yet implemented
            cfg.solver.poisson = "fft_periodic".to_string();
            // Ensure velocity_extent covers escape velocity
            let g = cfg.domain.gravitational_constant;
            let m = cfg.model.total_mass;
            let a = cfg.model.scale_radius;
            let v_esc = (2.0 * g * m / a).sqrt();
            let recommended = (v_esc * 1.5_f64).max(2.5);
            if cfg.domain.velocity_extent < recommended {
                cfg.domain.velocity_extent = recommended;
            }
        }
        "nfw" => {
            cfg.domain.boundary = "periodic|truncated".to_string();
            cfg.solver.poisson = "fft_periodic".to_string();
        }
        _ => {}
    }
}

/// Estimate grid memory usage in megabytes for the current config.
pub fn estimate_memory_mb(cfg: &PhasmaConfig) -> f64 {
    let nx = cfg.domain.spatial_resolution as f64;
    let nv = cfg.domain.velocity_resolution as f64;
    match cfg.solver.representation.as_str() {
        "uniform" => 8.0 * nx.powi(3) * nv.powi(3) / 1_000_000.0,
        "tensor_train" | "hierarchical_tucker" => {
            // Rough estimate: O(N^3 * r^3) where r ~ 8
            let r = 8.0_f64;
            8.0 * nx.powi(3) * r.powi(3) / 1_000_000.0
        }
        _ => 0.0,
    }
}
