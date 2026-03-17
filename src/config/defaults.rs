use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::PhasmaConfig;

/// Helper: create Decimal from f64 literal.
fn dec(f: f64) -> Decimal {
    Decimal::from_f64_retain(f).unwrap_or(Decimal::ZERO)
}

/// Itemized memory breakdown for display and validation.
#[derive(Debug, Clone, Default)]
pub struct MemoryBreakdown {
    /// Phase-space representation (grid / HT leaves+transfers / TT cores).
    pub phase_space_mb: f64,
    /// Poisson solver precomputed buffers (Green's function FFT, etc.).
    pub poisson_buffers_mb: f64,
    /// Per-step workspace: density N³ + potential N³ + acceleration 3×N³ + scratch.
    pub workspace_mb: f64,
    /// Advection temporaries: full grid clone during advect_x/v (transient — freed after step).
    pub advection_clone_mb: f64,
    /// LoMaC conservation overlay (KFVS macroscopic fields).
    pub lomac_mb: f64,
    /// HT re-compression workspace during SLAR (transient — freed after step).
    pub ht_recompression_mb: f64,
}

impl MemoryBreakdown {
    /// Persistent (resident) memory — allocated at construction and kept for lifetime.
    pub fn resident_mb(&self) -> f64 {
        self.phase_space_mb + self.poisson_buffers_mb + self.workspace_mb + self.lomac_mb
    }

    /// Peak transient memory during a step (advection clone, recompression scratch).
    pub fn peak_transient_mb(&self) -> f64 {
        self.advection_clone_mb + self.ht_recompression_mb
    }

    /// Total peak memory in MB (resident + peak transient).
    pub fn total_mb(&self) -> f64 {
        self.resident_mb() + self.peak_transient_mb()
    }
}

/// Compute a detailed memory breakdown for the given config.
pub fn estimate_memory_breakdown(cfg: &PhasmaConfig) -> MemoryBreakdown {
    let nx = cfg.domain.spatial_resolution as f64;
    let nv = cfg.domain.velocity_resolution as f64;
    let n3 = nx * nx * nx;
    let bytes_per_mb = 1_000_000.0_f64;

    // 1. Phase-space representation
    let phase_space_mb = match cfg.solver.representation.as_str() {
        "uniform" | "uniform_grid" => 8.0 * n3 * nv.powi(3) / bytes_per_mb,
        "hierarchical_tucker" | "ht" => {
            let r = cfg
                .solver
                .ht
                .as_ref()
                .map(|h| h.initial_rank as f64)
                .unwrap_or(20.0);
            let leaf_mem = 6.0 * nx.max(nv) * r * 8.0;
            let transfer_mem = 5.0 * r.powi(3) * 8.0;
            (leaf_mem + transfer_mem) / bytes_per_mb
        }
        "tensor_train" => {
            let r = 8.0_f64;
            8.0 * 6.0 * nx.max(nv) * r * r / bytes_per_mb
        }
        _ => 0.0,
    };

    // 2. Poisson solver buffers
    let poisson_buffers_mb = match cfg.solver.poisson.as_str() {
        // FftIsolated: zero-padded (2N)³ real grid + (2N)³ complex Green's FFT
        // = (2N)³ * 8 bytes (real) + (2N)³ * 16 bytes (complex) ≈ 24 * (2N)³
        // But realfft R2C halves the complex: (2N)² * (N+1) * 16
        "fft_isolated" => {
            let n2 = 2.0 * nx;
            let real_buf = n2.powi(3) * 8.0; // zero-padded input
            let complex_buf = n2 * n2 * (nx + 1.0) * 16.0; // R2C output
            let green_fft = complex_buf; // precomputed Green's function
            (real_buf + complex_buf + green_fft) / bytes_per_mb
        }
        // FftPeriodic: N³ real + N² * (N/2+1) complex, two copies (rho_hat, phi_hat)
        "fft_periodic" | "fft" => {
            let real_buf = n3 * 8.0;
            let complex_buf = nx * nx * (nx / 2.0 + 1.0) * 16.0;
            (real_buf + 2.0 * complex_buf) / bytes_per_mb
        }
        // TensorPoisson: (2N)³ Green's FFT + zero-padded work arrays
        // Green's FFT is precomputed complex (2N)³, plus 2 work arrays for convolution
        "tensor" | "tensor_poisson" => {
            let n2 = 2.0 * nx;
            let green_fft = n2.powi(3) * 16.0; // complex Green's
            let work = 2.0 * n2.powi(3) * 16.0; // 2 complex work arrays
            (green_fft + work) / bytes_per_mb
        }
        // Multigrid: hierarchy of grids, ~2× finest grid (geometric sum)
        "multigrid" => {
            let levels = cfg
                .solver
                .multigrid
                .as_ref()
                .map(|m| m.levels as f64)
                .unwrap_or(5.0);
            // Each level: N³/4^l * 8 bytes * 3 arrays (u, rhs, residual)
            let mut total = 0.0;
            for l in 0..levels as u32 {
                let nl = (nx / 2.0_f64.powi(l as i32)).max(2.0);
                total += nl.powi(3) * 8.0 * 3.0;
            }
            total / bytes_per_mb
        }
        // SphericalHarmonics: moderate — radial grid × l_max² coefficients
        "spherical" | "spherical_harmonics" => {
            let l_max = nx.min(32.0);
            (nx * l_max * l_max * 8.0 * 2.0) / bytes_per_mb
        }
        // Tree/Barnes-Hut: ~8 * N³ nodes × node struct (~64 bytes)
        "tree" | "barnes_hut" => (n3 * 64.0) / bytes_per_mb,
        _ => 0.0,
    };

    // 3. Per-step workspace: density(N³) + potential(N³) + acceleration(3×N³)
    let workspace_mb = (n3 * 8.0 * 5.0) / bytes_per_mb;

    // 4. Advection clone: semi-lagrangian clones the entire grid for departure-point interpolation
    let advection_clone_mb = match cfg.solver.representation.as_str() {
        "uniform" | "uniform_grid" => phase_space_mb, // full 6D clone
        "hierarchical_tucker" | "ht" => phase_space_mb, // HT clone for SLAR
        _ => 0.0,
    };

    // 5. LoMaC conservation: KFVS needs macroscopic fields (ρ, ρu, E) on spatial grid
    // 5 scalar fields × N³ × 8 bytes, plus the projected f clone
    let lomac_mb = if cfg.solver.conservation == "lomac" {
        (n3 * 8.0 * 5.0) / bytes_per_mb + phase_space_mb
    } else {
        0.0
    };

    // 6. HT re-compression during SLAR: from_function_aca builds a new HT tensor
    // requiring ~2× the HT size (fiber sampling + QR + SVD intermediates)
    let ht_recompression_mb =
        if matches!(
            cfg.solver.representation.as_str(),
            "hierarchical_tucker" | "ht"
        ) && matches!(cfg.solver.advection.as_str(), "slar" | "semi_lagrangian")
        {
            phase_space_mb * 2.0
        } else {
            0.0
        };

    MemoryBreakdown {
        phase_space_mb,
        poisson_buffers_mb,
        workspace_mb,
        advection_clone_mb,
        lomac_mb,
        ht_recompression_mb,
    }
}

/// Estimate total memory usage in megabytes for the current config.
#[cfg_attr(not(test), allow(dead_code))]
pub fn estimate_memory_mb(cfg: &PhasmaConfig) -> f64 {
    estimate_memory_breakdown(cfg).total_mb()
}

/// Estimate peak memory at max rank for HT representation.
#[cfg_attr(not(test), allow(dead_code))]
pub fn estimate_peak_memory_mb(cfg: &PhasmaConfig) -> f64 {
    if !matches!(
        cfg.solver.representation.as_str(),
        "hierarchical_tucker" | "ht"
    ) {
        return estimate_memory_mb(cfg);
    }
    // Build a temporary config with initial_rank = max_rank
    let mut peak_cfg = cfg.clone();
    if let Some(ref mut ht) = peak_cfg.solver.ht {
        ht.initial_rank = ht.max_rank;
    }
    estimate_memory_mb(&peak_cfg)
}

/// Full-grid equivalent memory for comparison display.
#[cfg_attr(not(test), allow(dead_code))]
pub fn full_grid_memory_mb(cfg: &PhasmaConfig) -> f64 {
    let nx = cfg.domain.spatial_resolution as f64;
    let nv = cfg.domain.velocity_resolution as f64;
    8.0 * nx.powi(3) * nv.powi(3) / 1_000_000.0
}

/// Apply smart defaults based on model type (spec §2.2).
///
/// When the model type changes, auto-fills velocity_extent, spatial_extent,
/// t_final, boundary, and solver.poisson to sensible values:
///   - "zeldovich" → periodic + fft_periodic, spatial_extent = box_size/2
///   - "plummer"/"hernquist"/"king"/"nfw" → isolated + fft_isolated
///   - "merger" → isolated + fft_isolated, larger domain
///   - "uniform_perturbation" → periodic + fft_periodic
///   - "hierarchical_tucker" representation → adds default HT params
pub fn apply_smart_defaults(cfg: &mut PhasmaConfig) {
    // Only override t_final if the user didn't set it explicitly in TOML
    // (i.e. it still has the serde default value of 10.0).
    let t_final_is_default = cfg.time.t_final == super::TimeConfig::default().t_final;

    match cfg.model.model_type.as_str() {
        "zeldovich" => {
            cfg.domain.boundary = "periodic|truncated".to_string();
            cfg.solver.poisson = "fft_periodic".to_string();
            if let Some(ref z) = cfg.model.zeldovich {
                cfg.domain.spatial_extent = z.box_size / dec(2.0);
            }
            if t_final_is_default {
                cfg.time.t_final = dec(1.2);
            }
        }
        "plummer" | "hernquist" | "king" | "nfw" => {
            cfg.domain.boundary = "isolated|truncated".to_string();
            cfg.solver.poisson = "fft_isolated".to_string();
            // Velocity extent ~ escape velocity estimate: v_esc = sqrt(2GM/a)
            let g = cfg.domain.gravitational_constant.to_f64().unwrap_or(1.0);
            let m = cfg.model.total_mass.to_f64().unwrap_or(1.0);
            let a = cfg.model.scale_radius.to_f64().unwrap_or(1.0);
            let v_esc = (2.0 * g * m / a).sqrt();
            cfg.domain.velocity_extent = dec((v_esc * 1.5).max(3.0));
            cfg.domain.spatial_extent = cfg.model.scale_radius * dec(10.0);
            if t_final_is_default {
                cfg.time.t_final = dec(50.0);
            }
        }
        "merger" | "two_body_merger" => {
            cfg.domain.boundary = "isolated|truncated".to_string();
            cfg.solver.poisson = "fft_isolated".to_string();
            if let Some(ref merger) = cfg.model.merger {
                cfg.domain.spatial_extent = merger.separation * dec(2.0);
            }
            if t_final_is_default {
                cfg.time.t_final = dec(100.0);
            }
        }
        "tidal" => {
            cfg.domain.boundary = "isolated|truncated".to_string();
            cfg.solver.poisson = "fft_isolated".to_string();
            if let Some(ref tc) = cfg.model.tidal {
                // Domain should encompass the orbit
                let r = (tc.progenitor_position[0].powi(2)
                    + tc.progenitor_position[1].powi(2)
                    + tc.progenitor_position[2].powi(2))
                .sqrt();
                cfg.domain.spatial_extent = dec((r * 2.5).max(10.0));
                let v = (tc.progenitor_velocity[0].powi(2)
                    + tc.progenitor_velocity[1].powi(2)
                    + tc.progenitor_velocity[2].powi(2))
                .sqrt();
                cfg.domain.velocity_extent = dec((v * 3.0).max(3.0));
            }
            if t_final_is_default {
                cfg.time.t_final = dec(50.0);
            }
        }
        "uniform_perturbation" => {
            cfg.domain.boundary = "periodic|open".to_string();
            cfg.solver.poisson = "fft_periodic".to_string();
            if t_final_is_default {
                cfg.time.t_final = dec(5.0);
            }
        }
        "disk_exponential" | "disk_stability" => {
            cfg.domain.boundary = "isolated|truncated".to_string();
            cfg.solver.poisson = "fft_isolated".to_string();
            if let Some(ref disk) = cfg.model.disk {
                cfg.domain.spatial_extent = disk.disk_scale_length * dec(10.0);
            }
            if t_final_is_default {
                cfg.time.t_final = dec(100.0);
            }
        }
        _ => {}
    }

    // HT representation auto-adds default solver.ht params
    if matches!(
        cfg.solver.representation.as_str(),
        "hierarchical_tucker" | "ht"
    ) && cfg.solver.ht.is_none()
    {
        cfg.solver.ht = Some(super::HtSolverConfig {
            max_rank: 100,
            initial_rank: 20,
            tolerance: 1e-6,
            rank_adaptation: "tolerance".to_string(),
            dimension_tree: "balanced_xv".to_string(),
            custom_tree: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PhasmaConfig;

    /// Load a config from configs/ directory.
    fn load_config(name: &str) -> PhasmaConfig {
        let path = format!("{}/configs/{name}.toml", env!("CARGO_MANIFEST_DIR"));
        crate::config::load(&path).unwrap_or_else(|e| panic!("failed to load {name}: {e}"))
    }

    #[test]
    fn breakdown_default_uniform_plummer() {
        let cfg = load_config("plummer");
        let b = estimate_memory_breakdown(&cfg);

        // Phase space: 16^6 * 8 bytes ≈ 134 MB
        let n = 16.0_f64;
        let nv = 16.0_f64;
        let expected_ps = 8.0 * n.powi(3) * nv.powi(3) / 1_000_000.0;
        assert!(
            (b.phase_space_mb - expected_ps).abs() < 0.01,
            "phase_space: got {}, expected {}",
            b.phase_space_mb,
            expected_ps
        );

        // Poisson: fft_isolated: (2N)³*8 + (2N)²*(N+1)*16 + same for Green's
        let n3 = n.powi(3);
        let n2 = 2.0 * n;
        let real_buf = n2.powi(3) * 8.0;
        let complex_buf = n2 * n2 * (n + 1.0) * 16.0;
        let expected_poisson = (real_buf + complex_buf + complex_buf) / 1_000_000.0;
        assert!(
            (b.poisson_buffers_mb - expected_poisson).abs() < 0.01,
            "poisson: got {}, expected {}",
            b.poisson_buffers_mb,
            expected_poisson
        );

        // Workspace: 5*N³*8
        let expected_ws = (n3 * 8.0 * 5.0) / 1_000_000.0;
        assert!(
            (b.workspace_mb - expected_ws).abs() < 0.01,
            "workspace: got {}, expected {}",
            b.workspace_mb,
            expected_ws
        );

        // Advection clone: same as phase_space for uniform
        assert!(
            (b.advection_clone_mb - b.phase_space_mb).abs() < 0.01,
            "advect clone: got {}, expected {}",
            b.advection_clone_mb,
            b.phase_space_mb
        );

        // No LoMaC, no HT recompression
        assert_eq!(b.lomac_mb, 0.0);
        assert_eq!(b.ht_recompression_mb, 0.0);

        // Total should be sum of all parts
        let sum = b.phase_space_mb
            + b.poisson_buffers_mb
            + b.workspace_mb
            + b.advection_clone_mb
            + b.lomac_mb
            + b.ht_recompression_mb;
        assert!(
            (b.total_mb() - sum).abs() < 0.01,
            "total: got {}, expected {}",
            b.total_mb(),
            sum
        );
    }

    #[test]
    fn breakdown_ht_plummer_with_lomac() {
        let cfg = load_config("plummer_ht");
        let b = estimate_memory_breakdown(&cfg);

        // HT at rank 10, N=32: 6*32*10*8 + 5*10³*8
        let n = 32.0_f64;
        let r = 10.0;
        let leaf_mem = 6.0 * n * r * 8.0;
        let transfer_mem = 5.0 * r.powi(3) * 8.0;
        let expected_ps = (leaf_mem + transfer_mem) / 1_000_000.0;
        assert!(
            (b.phase_space_mb - expected_ps).abs() < 0.001,
            "ht phase_space: got {}, expected {}",
            b.phase_space_mb,
            expected_ps
        );

        // fft_isolated Poisson buffers present
        assert!(
            b.poisson_buffers_mb > 0.0,
            "fft_isolated should have Poisson buffers"
        );

        // LoMaC enabled: should add 5*N³*8 + phase_space clone
        assert!(
            b.lomac_mb > 0.0,
            "lomac conservation should contribute memory"
        );

        // HT recompression for SLAR advection: 2x phase_space
        assert!(
            (b.ht_recompression_mb - 2.0 * b.phase_space_mb).abs() < 0.001,
            "ht recompression: got {}, expected {}",
            b.ht_recompression_mb,
            2.0 * b.phase_space_mb
        );
    }

    #[test]
    fn breakdown_isolated_plummer_fft_isolated() {
        let cfg = load_config("plummer");
        let b = estimate_memory_breakdown(&cfg);

        // FftIsolated: (2N)³*8 + (2N)²*(N+1)*16 + same for Green's
        let n = 16.0_f64;
        let n2 = 2.0 * n;
        let real_buf = n2.powi(3) * 8.0;
        let complex_buf = n2 * n2 * (n + 1.0) * 16.0;
        let expected_poisson = (real_buf + complex_buf + complex_buf) / 1_000_000.0;
        assert!(
            (b.poisson_buffers_mb - expected_poisson).abs() < 0.01,
            "fft_isolated poisson: got {}, expected {}",
            b.poisson_buffers_mb,
            expected_poisson
        );
    }

    #[test]
    fn peak_memory_exceeds_initial_for_ht() {
        let cfg = load_config("plummer_ht");
        let initial = estimate_memory_mb(&cfg);
        let peak = estimate_peak_memory_mb(&cfg);

        // max_rank=50 > initial_rank=10 → peak > initial
        assert!(
            peak > initial,
            "peak ({peak}) should exceed initial ({initial}) for HT"
        );
    }

    #[test]
    fn full_grid_much_larger_than_ht() {
        let cfg = load_config("plummer_ht");
        let ht_mem = estimate_memory_mb(&cfg);
        let full = full_grid_memory_mb(&cfg);

        // 16^6 * 8 = ~134 MB, HT at rank 10 ≈ 0.05 MB
        assert!(
            full > ht_mem * 10.0,
            "full grid ({full} MB) should be much larger than HT ({ht_mem} MB)"
        );
    }

    /// Load every config in configs/ and verify the estimate is positive
    /// and the breakdown sums correctly.
    #[test]
    fn all_configs_have_positive_estimates() {
        let configs_dir = format!("{}/configs", env!("CARGO_MANIFEST_DIR"));
        let entries: Vec<_> = std::fs::read_dir(&configs_dir)
            .expect("configs/ directory should exist")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();

        assert!(!entries.is_empty(), "should have at least one config");

        for entry in entries {
            let path = entry.path();
            let name = path.file_stem().unwrap().to_string_lossy();

            let cfg = crate::config::load(path.to_str().unwrap())
                .unwrap_or_else(|e| panic!("failed to load {name}: {e}"));

            let b = estimate_memory_breakdown(&cfg);
            let total = b.total_mb();

            assert!(
                total > 0.0,
                "{name}: total memory estimate should be > 0, got {total}"
            );

            // Verify sum consistency
            let sum = b.phase_space_mb
                + b.poisson_buffers_mb
                + b.workspace_mb
                + b.advection_clone_mb
                + b.lomac_mb
                + b.ht_recompression_mb;
            assert!(
                (total - sum).abs() < 0.001,
                "{name}: total_mb ({total}) != sum of parts ({sum})"
            );

            // Phase space should always be non-negative
            assert!(
                b.phase_space_mb >= 0.0,
                "{name}: phase_space_mb should be >= 0"
            );
        }
    }

    #[test]
    fn smart_defaults_zeldovich() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "zeldovich".to_string();
        cfg.model.zeldovich = Some(crate::config::ZeldovichConfig {
            amplitude: dec(0.01),
            wave_number: dec(1.0),
            box_size: dec(100.0),
            redshift_initial: dec(50.0),
            cosmology_h: dec(0.7),
            cosmology_omega_m: dec(0.3),
            cosmology_omega_lambda: dec(0.7),
        });
        apply_smart_defaults(&mut cfg);
        assert!(cfg.domain.boundary.contains("periodic"));
        assert_eq!(cfg.solver.poisson, "fft_periodic");
        assert_eq!(cfg.time.t_final, dec(1.2));
    }

    #[test]
    fn smart_defaults_plummer() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "plummer".to_string();
        apply_smart_defaults(&mut cfg);
        assert!(cfg.domain.boundary.contains("isolated"));
        assert_eq!(cfg.solver.poisson, "fft_isolated");
        assert!(cfg.domain.velocity_extent > Decimal::ZERO);
        assert_eq!(
            cfg.domain.spatial_extent,
            cfg.model.scale_radius * dec(10.0)
        );
    }

    #[test]
    fn smart_defaults_merger() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "merger".to_string();
        cfg.model.merger = Some(crate::config::MergerConfig {
            separation: dec(10.0),
            mass_ratio: dec(1.0),
            relative_velocity: [0.0, 0.0, 0.0],
            impact_parameter: dec(2.0),
            model_1: "plummer".to_string(),
            model_2: "plummer".to_string(),
            scale_radius_1: dec(1.0),
            scale_radius_2: dec(1.0),
        });
        apply_smart_defaults(&mut cfg);
        assert!(cfg.domain.boundary.contains("isolated"));
        assert_eq!(cfg.solver.poisson, "fft_isolated");
        assert_eq!(cfg.domain.spatial_extent, dec(10.0) * dec(2.0));
    }

    #[test]
    fn smart_defaults_perturbation() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "uniform_perturbation".to_string();
        cfg.model.uniform_perturbation = Some(crate::config::PerturbationConfig {
            background_density: dec(1.0),
            velocity_dispersion: dec(0.5),
            perturbation_amplitude: dec(0.01),
            perturbation_wavenumber: [1.0, 0.0, 0.0],
        });
        apply_smart_defaults(&mut cfg);
        assert!(cfg.domain.boundary.contains("periodic"));
        assert_eq!(cfg.solver.poisson, "fft_periodic");
        assert_eq!(cfg.time.t_final, dec(5.0));
    }

    #[test]
    fn smart_defaults_disk() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "disk_stability".to_string();
        cfg.model.disk = Some(crate::config::DiskModelConfig {
            disk_mass: dec(1.0),
            disk_scale_length: dec(3.0),
            disk_scale_height: dec(0.3),
            radial_velocity_dispersion: dec(0.15),
            halo_type: "plummer".to_string(),
            halo_mass: dec(10.0),
            halo_concentration: dec(10.0),
            toomre_q: dec(1.5),
        });
        apply_smart_defaults(&mut cfg);
        assert!(cfg.domain.boundary.contains("isolated"));
        assert_eq!(cfg.domain.spatial_extent, dec(3.0) * dec(10.0));
    }

    #[test]
    fn smart_defaults_tidal() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "tidal".to_string();
        cfg.model.tidal = Some(crate::config::TidalConfig {
            progenitor_type: "plummer".to_string(),
            progenitor_mass: dec(1.0),
            progenitor_scale_radius: dec(1.0),
            host_type: "point_mass".to_string(),
            host_mass: dec(10.0),
            host_scale_radius: dec(20.0),
            progenitor_position: [5.0, 0.0, 0.0],
            progenitor_velocity: [0.0, 0.0, 0.0],
        });
        apply_smart_defaults(&mut cfg);
        assert!(cfg.domain.boundary.contains("isolated"));
        // r = 5.0, spatial_extent = (5.0 * 2.5).max(10.0) = 12.5
        assert!(cfg.domain.spatial_extent > dec(10.0));
    }

    #[test]
    fn smart_defaults_ht_auto() {
        let mut cfg = PhasmaConfig::default();
        cfg.solver.representation = "ht".to_string();
        cfg.solver.ht = None;
        apply_smart_defaults(&mut cfg);
        assert!(cfg.solver.ht.is_some());
        let ht = cfg.solver.ht.unwrap();
        assert_eq!(ht.max_rank, 100);
        assert_eq!(ht.initial_rank, 20);
    }

    #[test]
    fn smart_defaults_preserves_explicit_t_final() {
        let mut cfg = PhasmaConfig::default();
        cfg.model.model_type = "plummer".to_string();
        cfg.time.t_final = dec(42.0);
        apply_smart_defaults(&mut cfg);
        assert_eq!(cfg.time.t_final, dec(42.0));
    }
}
