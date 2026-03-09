use super::PhasmaConfig;

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
