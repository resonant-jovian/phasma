//! Comprehensive tests for all phasma TOML configs.
//!
//! Validates loading, field correctness, validation, round-trip serialization,
//! smart defaults, and sub-config presence for every config in configs/.

use super::PhasmaConfig;

/// Load a config from the configs/ directory by stem name.
fn load_config(name: &str) -> PhasmaConfig {
    let path = format!("{}/configs/{name}.toml", env!("CARGO_MANIFEST_DIR"));
    super::load(&path).unwrap_or_else(|e| panic!("failed to load {name}: {e}"))
}

// ── 1. Load tests ────────────────────────────────────────────────────────────
// Each config file parses without error.

macro_rules! load_test {
    ($name:ident) => {
        paste::paste! {
            #[test]
            fn [<load_ $name>]() {
                load_config(stringify!($name));
            }
        }
    };
}

load_test!(debug);
load_test!(disk_bar);
load_test!(hernquist);
load_test!(jeans_stable);
load_test!(jeans_unstable);
load_test!(king);
load_test!(merger_equal);
load_test!(merger_unequal);
load_test!(nfw);
load_test!(nfw_tree);
load_test!(plummer);
load_test!(plummer_128);
load_test!(plummer_hires);
load_test!(plummer_ht);
load_test!(plummer_lomac);
load_test!(plummer_multigrid);
load_test!(plummer_spectral);
load_test!(plummer_spherical);
load_test!(plummer_tensor_poisson);
load_test!(plummer_tt);
load_test!(plummer_unsplit);
load_test!(plummer_yoshida);
load_test!(tidal_nfw);
load_test!(tidal_point);
load_test!(zeldovich);

// ── 2. Validation tests ──────────────────────────────────────────────────────
// Each config passes validation with no error-level warnings.
// Memory budget warnings are filtered (machine-dependent, not a config error).

fn non_memory_warnings(cfg: &PhasmaConfig) -> Vec<String> {
    super::validate::validate(cfg)
        .into_iter()
        .filter(|w| !w.field.contains("memory_budget"))
        .map(|w| format!("{}", w))
        .collect()
}

macro_rules! validate_test {
    ($name:ident) => {
        paste::paste! {
            #[test]
            fn [<validate_ $name>]() {
                let cfg = load_config(stringify!($name));
                let warnings = non_memory_warnings(&cfg);
                assert!(warnings.is_empty(), "validation warnings for {}: {:?}", stringify!($name), warnings);
            }
        }
    };
}

validate_test!(debug);
validate_test!(disk_bar);
validate_test!(hernquist);
validate_test!(jeans_stable);
validate_test!(jeans_unstable);
validate_test!(king);
validate_test!(merger_equal);
validate_test!(merger_unequal);
validate_test!(nfw);
validate_test!(nfw_tree);
validate_test!(plummer);
validate_test!(plummer_128);
validate_test!(plummer_hires);
validate_test!(plummer_ht);
validate_test!(plummer_lomac);
validate_test!(plummer_multigrid);
validate_test!(plummer_spectral);
validate_test!(plummer_spherical);
validate_test!(plummer_tensor_poisson);
validate_test!(plummer_tt);
validate_test!(plummer_unsplit);
validate_test!(plummer_yoshida);
validate_test!(tidal_nfw);
validate_test!(tidal_point);
validate_test!(zeldovich);

// ── 3. Round-trip tests ──────────────────────────────────────────────────────
// Load → serialize → deserialize preserves key fields.

fn round_trip(name: &str) {
    let original = load_config(name);
    let serialized = toml::to_string_pretty(&original)
        .unwrap_or_else(|e| panic!("{name}: serialize failed: {e}"));
    let restored: PhasmaConfig = toml::from_str(&serialized)
        .unwrap_or_else(|e| panic!("{name}: deserialize round-trip failed: {e}"));

    assert_eq!(
        original.model.model_type, restored.model.model_type,
        "{name}: model_type mismatch after round-trip"
    );
    assert_eq!(
        original.solver.representation, restored.solver.representation,
        "{name}: representation mismatch after round-trip"
    );
    assert_eq!(
        original.solver.poisson, restored.solver.poisson,
        "{name}: poisson mismatch after round-trip"
    );
    assert_eq!(
        original.solver.integrator, restored.solver.integrator,
        "{name}: integrator mismatch after round-trip"
    );
    assert_eq!(
        original.domain.spatial_resolution, restored.domain.spatial_resolution,
        "{name}: spatial_resolution mismatch after round-trip"
    );
    assert_eq!(
        original.domain.velocity_resolution, restored.domain.velocity_resolution,
        "{name}: velocity_resolution mismatch after round-trip"
    );
    assert_eq!(
        original.domain.boundary, restored.domain.boundary,
        "{name}: boundary mismatch after round-trip"
    );

    // Sub-config presence preserved
    assert_eq!(
        original.model.king.is_some(),
        restored.model.king.is_some(),
        "{name}: king sub-config presence mismatch"
    );
    assert_eq!(
        original.model.nfw.is_some(),
        restored.model.nfw.is_some(),
        "{name}: nfw sub-config presence mismatch"
    );
    assert_eq!(
        original.model.merger.is_some(),
        restored.model.merger.is_some(),
        "{name}: merger sub-config presence mismatch"
    );
    assert_eq!(
        original.model.disk.is_some(),
        restored.model.disk.is_some(),
        "{name}: disk sub-config presence mismatch"
    );
    assert_eq!(
        original.model.tidal.is_some(),
        restored.model.tidal.is_some(),
        "{name}: tidal sub-config presence mismatch"
    );
    assert_eq!(
        original.model.zeldovich.is_some(),
        restored.model.zeldovich.is_some(),
        "{name}: zeldovich sub-config presence mismatch"
    );
    assert_eq!(
        original.model.uniform_perturbation.is_some(),
        restored.model.uniform_perturbation.is_some(),
        "{name}: uniform_perturbation sub-config presence mismatch"
    );
    assert_eq!(
        original.solver.ht.is_some(),
        restored.solver.ht.is_some(),
        "{name}: ht sub-config presence mismatch"
    );
    assert_eq!(
        original.solver.slar.is_some(),
        restored.solver.slar.is_some(),
        "{name}: slar sub-config presence mismatch"
    );
    assert_eq!(
        original.solver.lomac.is_some(),
        restored.solver.lomac.is_some(),
        "{name}: lomac sub-config presence mismatch"
    );
    assert_eq!(
        original.solver.multigrid.is_some(),
        restored.solver.multigrid.is_some(),
        "{name}: multigrid sub-config presence mismatch"
    );
    assert_eq!(
        original.solver.exponential_sum.is_some(),
        restored.solver.exponential_sum.is_some(),
        "{name}: exponential_sum sub-config presence mismatch"
    );
}

macro_rules! round_trip_test {
    ($name:ident) => {
        paste::paste! {
            #[test]
            fn [<round_trip_ $name>]() {
                round_trip(stringify!($name));
            }
        }
    };
}

round_trip_test!(debug);
round_trip_test!(disk_bar);
round_trip_test!(hernquist);
round_trip_test!(jeans_stable);
round_trip_test!(jeans_unstable);
round_trip_test!(king);
round_trip_test!(merger_equal);
round_trip_test!(merger_unequal);
round_trip_test!(nfw);
round_trip_test!(nfw_tree);
round_trip_test!(plummer);
round_trip_test!(plummer_128);
round_trip_test!(plummer_hires);
round_trip_test!(plummer_ht);
round_trip_test!(plummer_lomac);
round_trip_test!(plummer_multigrid);
round_trip_test!(plummer_spectral);
round_trip_test!(plummer_spherical);
round_trip_test!(plummer_tensor_poisson);
round_trip_test!(plummer_tt);
round_trip_test!(plummer_unsplit);
round_trip_test!(plummer_yoshida);
round_trip_test!(tidal_nfw);
round_trip_test!(tidal_point);
round_trip_test!(zeldovich);

// ── 4. Field assertion tests ─────────────────────────────────────────────────
// Each config has the expected field values.

struct ExpectedFields {
    name: &'static str,
    model_type: &'static str,
    representation: &'static str,
    poisson: &'static str,
    integrator: &'static str,
    nx: u32,
    nv: u32,
    boundary: &'static str,
}

fn assert_fields(e: &ExpectedFields) {
    let cfg = load_config(e.name);
    assert_eq!(cfg.model.model_type, e.model_type, "{}: model_type", e.name);
    assert_eq!(
        cfg.solver.representation, e.representation,
        "{}: representation",
        e.name
    );
    assert_eq!(cfg.solver.poisson, e.poisson, "{}: poisson", e.name);
    assert_eq!(
        cfg.solver.integrator, e.integrator,
        "{}: integrator",
        e.name
    );
    assert_eq!(
        cfg.domain.spatial_resolution, e.nx,
        "{}: spatial_resolution",
        e.name
    );
    assert_eq!(
        cfg.domain.velocity_resolution, e.nv,
        "{}: velocity_resolution",
        e.name
    );
    assert_eq!(cfg.domain.boundary, e.boundary, "{}: boundary", e.name);
}

macro_rules! field_test {
    ($name:ident, $model:expr, $repr:expr, $poisson:expr, $integrator:expr,
     $nx:expr, $nv:expr, $boundary:expr) => {
        paste::paste! {
            #[test]
            fn [<fields_ $name>]() {
                assert_fields(&ExpectedFields {
                    name: stringify!($name),
                    model_type: $model,
                    representation: $repr,
                    poisson: $poisson,
                    integrator: $integrator,
                    nx: $nx,
                    nv: $nv,
                    boundary: $boundary,
                });
            }
        }
    };
}

field_test!(
    debug,
    "plummer",
    "uniform",
    "fft_periodic",
    "lie",
    4,
    4,
    "periodic|truncated"
);
field_test!(
    disk_bar,
    "disk_stability",
    "uniform",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    hernquist,
    "hernquist",
    "uniform",
    "fft_isolated",
    "yoshida",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    jeans_stable,
    "uniform_perturbation",
    "uniform",
    "fft_periodic",
    "strang",
    8,
    8,
    "periodic|open"
);
field_test!(
    jeans_unstable,
    "uniform_perturbation",
    "uniform",
    "fft_periodic",
    "strang",
    8,
    8,
    "periodic|open"
);
field_test!(
    king,
    "king",
    "uniform",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    merger_equal,
    "merger",
    "uniform",
    "fft_periodic",
    "strang",
    16,
    16,
    "periodic|truncated"
);
field_test!(
    merger_unequal,
    "merger",
    "uniform",
    "fft_periodic",
    "strang",
    32,
    32,
    "periodic|truncated"
);
field_test!(
    nfw,
    "nfw",
    "uniform",
    "fft_isolated",
    "yoshida",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    nfw_tree,
    "nfw",
    "uniform",
    "tree",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer,
    "plummer",
    "uniform",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_128,
    "plummer",
    "hierarchical_tucker",
    "fft_isolated",
    "strang",
    128,
    128,
    "isolated|truncated"
);
field_test!(
    plummer_hires,
    "plummer",
    "uniform",
    "fft_isolated",
    "yoshida",
    32,
    32,
    "isolated|truncated"
);
field_test!(
    plummer_ht,
    "plummer",
    "hierarchical_tucker",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_lomac,
    "plummer",
    "uniform",
    "fft_periodic",
    "strang",
    16,
    16,
    "periodic|truncated"
);
field_test!(
    plummer_multigrid,
    "plummer",
    "uniform",
    "multigrid",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_spectral,
    "plummer",
    "spectral",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_spherical,
    "plummer",
    "uniform",
    "spherical_harmonics",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_tensor_poisson,
    "plummer",
    "uniform",
    "tensor",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_tt,
    "plummer",
    "tensor_train",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    plummer_unsplit,
    "plummer",
    "uniform",
    "fft_periodic",
    "unsplit_rk4",
    8,
    8,
    "periodic|truncated"
);
field_test!(
    plummer_yoshida,
    "plummer",
    "uniform",
    "fft_isolated",
    "yoshida",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    tidal_nfw,
    "tidal",
    "uniform",
    "fft_isolated",
    "strang",
    16,
    16,
    "isolated|truncated"
);
field_test!(
    tidal_point,
    "tidal",
    "uniform",
    "fft_isolated",
    "strang",
    32,
    32,
    "isolated|truncated"
);
field_test!(
    zeldovich,
    "zeldovich",
    "uniform",
    "fft_periodic",
    "strang",
    32,
    32,
    "periodic|truncated"
);

// ── 5. Smart defaults don't introduce validation errors ──────────────────────

#[test]
fn smart_defaults_no_new_validation_errors() {
    let configs_dir = format!("{}/configs", env!("CARGO_MANIFEST_DIR"));
    let entries: Vec<_> = std::fs::read_dir(&configs_dir)
        .expect("configs/ directory should exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();

    for entry in entries {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        let mut cfg = super::load(path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("failed to load {name}: {e}"));

        super::defaults::apply_smart_defaults(&mut cfg);

        let warnings = non_memory_warnings(&cfg);
        assert!(
            warnings.is_empty(),
            "{name} after smart defaults: {:?}",
            warnings
        );
    }
}

// ── 6. Sub-config presence tests ─────────────────────────────────────────────
// Configs that require sub-sections actually have them.

#[test]
fn king_has_king_subconfig() {
    let cfg = load_config("king");
    assert!(cfg.model.king.is_some(), "king.toml must have [model.king]");
}

#[test]
fn nfw_has_nfw_subconfig() {
    let cfg = load_config("nfw");
    assert!(cfg.model.nfw.is_some(), "nfw.toml must have [model.nfw]");
}

#[test]
fn nfw_tree_has_nfw_subconfig() {
    let cfg = load_config("nfw_tree");
    assert!(
        cfg.model.nfw.is_some(),
        "nfw_tree.toml must have [model.nfw]"
    );
}

#[test]
fn zeldovich_has_zeldovich_subconfig() {
    let cfg = load_config("zeldovich");
    assert!(
        cfg.model.zeldovich.is_some(),
        "zeldovich.toml must have [model.zeldovich]"
    );
}

#[test]
fn merger_equal_has_merger_subconfig() {
    let cfg = load_config("merger_equal");
    assert!(
        cfg.model.merger.is_some(),
        "merger_equal.toml must have [model.merger]"
    );
}

#[test]
fn merger_unequal_has_merger_subconfig() {
    let cfg = load_config("merger_unequal");
    assert!(
        cfg.model.merger.is_some(),
        "merger_unequal.toml must have [model.merger]"
    );
}

#[test]
fn disk_bar_has_disk_subconfig() {
    let cfg = load_config("disk_bar");
    assert!(
        cfg.model.disk.is_some(),
        "disk_bar.toml must have [model.disk]"
    );
}

#[test]
fn tidal_nfw_has_tidal_subconfig() {
    let cfg = load_config("tidal_nfw");
    assert!(
        cfg.model.tidal.is_some(),
        "tidal_nfw.toml must have [model.tidal]"
    );
}

#[test]
fn tidal_point_has_tidal_subconfig() {
    let cfg = load_config("tidal_point");
    assert!(
        cfg.model.tidal.is_some(),
        "tidal_point.toml must have [model.tidal]"
    );
}

#[test]
fn jeans_stable_has_perturbation_subconfig() {
    let cfg = load_config("jeans_stable");
    assert!(
        cfg.model.uniform_perturbation.is_some(),
        "jeans_stable.toml must have [model.uniform_perturbation]"
    );
}

#[test]
fn jeans_unstable_has_perturbation_subconfig() {
    let cfg = load_config("jeans_unstable");
    assert!(
        cfg.model.uniform_perturbation.is_some(),
        "jeans_unstable.toml must have [model.uniform_perturbation]"
    );
}

#[test]
fn plummer_128_has_solver_subconfigs() {
    let cfg = load_config("plummer_128");
    assert!(
        cfg.solver.ht.is_some(),
        "plummer_128.toml must have [solver.ht]"
    );
    assert!(
        cfg.solver.slar.is_some(),
        "plummer_128.toml must have [solver.slar]"
    );
    assert!(
        cfg.solver.lomac.is_some(),
        "plummer_128.toml must have [solver.lomac]"
    );
}

#[test]
fn plummer_ht_has_solver_subconfigs() {
    let cfg = load_config("plummer_ht");
    assert!(
        cfg.solver.ht.is_some(),
        "plummer_ht.toml must have [solver.ht]"
    );
    assert!(
        cfg.solver.slar.is_some(),
        "plummer_ht.toml must have [solver.slar]"
    );
    assert!(
        cfg.solver.lomac.is_some(),
        "plummer_ht.toml must have [solver.lomac]"
    );
}

#[test]
fn plummer_lomac_has_lomac_subconfig() {
    let cfg = load_config("plummer_lomac");
    assert!(
        cfg.solver.lomac.is_some(),
        "plummer_lomac.toml must have [solver.lomac]"
    );
}

#[test]
fn plummer_multigrid_has_multigrid_subconfig() {
    let cfg = load_config("plummer_multigrid");
    assert!(
        cfg.solver.multigrid.is_some(),
        "plummer_multigrid.toml must have [solver.multigrid]"
    );
}

#[test]
fn plummer_tensor_poisson_has_exp_sum_subconfig() {
    let cfg = load_config("plummer_tensor_poisson");
    assert!(
        cfg.solver.exponential_sum.is_some(),
        "plummer_tensor_poisson.toml must have [solver.exponential_sum]"
    );
}

#[test]
fn plummer_spectral_has_ht_subconfig() {
    let cfg = load_config("plummer_spectral");
    assert!(
        cfg.solver.ht.is_some(),
        "plummer_spectral.toml must have [solver.ht]"
    );
}

#[test]
fn plummer_tt_has_ht_subconfig() {
    let cfg = load_config("plummer_tt");
    assert!(
        cfg.solver.ht.is_some(),
        "plummer_tt.toml must have [solver.ht]"
    );
}
