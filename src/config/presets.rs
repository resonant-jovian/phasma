use super::PhasmaConfig;

const QUICK_TEST: &str = r#"
[model]
type = "plummer"
total_mass = 1.0
scale_radius = 1.0

[domain]
spatial_extent = 8.0
velocity_extent = 2.5
spatial_resolution = 8
velocity_resolution = 8
boundary = "periodic|truncated"
gravitational_constant = 1.0

[time]
t_final = 2.0
dt_mode = "adaptive"
cfl_factor = 0.5

[output]
directory = "output/quick_test"

[exit]
energy_drift_tolerance = 0.5
mass_drift_tolerance = 0.1
"#;

const PLUMMER_PRODUCTION: &str = r#"
[model]
type = "plummer"
total_mass = 1.0
scale_radius = 1.0

[domain]
spatial_extent = 10.0
velocity_extent = 3.0
spatial_resolution = 16
velocity_resolution = 16
boundary = "periodic|truncated"
gravitational_constant = 1.0

[time]
t_final = 20.0
dt_mode = "adaptive"
cfl_factor = 0.5

[output]
directory = "output/plummer_production"
snapshot_interval = 1.0

[exit]
energy_drift_tolerance = 0.05
mass_drift_tolerance = 0.01
"#;

const JEANS_INSTABILITY: &str = r#"
[model]
type = "uniform_perturbation"
total_mass = 1.0
scale_radius = 1.0

[model.uniform_perturbation]
mode_m = 1
amplitude = 0.01

[domain]
spatial_extent = 6.283185
velocity_extent = 2.0
spatial_resolution = 8
velocity_resolution = 8
boundary = "periodic|truncated"
gravitational_constant = 1.0

[time]
t_final = 5.0
dt_mode = "adaptive"
cfl_factor = 0.5

[output]
directory = "output/jeans"

[exit]
energy_drift_tolerance = 0.5
"#;

const COSMOLOGICAL: &str = r#"
[model]
type = "zeldovich"
total_mass = 1.0
scale_radius = 1.0

[model.zeldovich]
amplitude = 0.3
wave_number = 1.0

[domain]
spatial_extent = 6.283185
velocity_extent = 2.0
spatial_resolution = 16
velocity_resolution = 8
boundary = "periodic|truncated"
gravitational_constant = 1.0

[time]
t_final = 3.0
dt_mode = "adaptive"
cfl_factor = 0.5

[output]
directory = "output/cosmological"

[exit]
energy_drift_tolerance = 0.5
"#;

pub fn list_presets() -> Vec<&'static str> {
    vec!["quick_test", "plummer_production", "jeans_instability", "cosmological"]
}

pub fn load_preset(name: &str) -> Option<PhasmaConfig> {
    let src = match name {
        "quick_test" => QUICK_TEST,
        "plummer_production" => PLUMMER_PRODUCTION,
        "jeans_instability" => JEANS_INSTABILITY,
        "cosmological" => COSMOLOGICAL,
        _ => return None,
    };
    toml::from_str(src).ok()
}
