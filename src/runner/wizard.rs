//! Interactive CLI wizard for generating a TOML config file.

use std::io::{self, Write};

use rust_decimal::Decimal;

use crate::config::PhasmaConfig;

fn dec(f: f64) -> Decimal {
    Decimal::from_f64_retain(f).unwrap_or(Decimal::ZERO)
}

/// Run the interactive wizard, printing prompts to stderr and reading from stdin.
pub fn run_wizard() -> anyhow::Result<()> {
    eprintln!("phasma wizard: interactive configuration generator\n");

    let mut cfg = PhasmaConfig::default();

    // 1. Model type
    eprintln!("Available models: plummer, hernquist, king, nfw, zeldovich, merger");
    let model_type = prompt("Model type", "plummer")?;
    cfg.model.model_type = model_type.clone();
    cfg.model.total_mass = prompt_decimal("Total mass", 1.0)?;
    cfg.model.scale_radius = prompt_decimal("Scale radius", 1.0)?;

    match model_type.as_str() {
        "king" => {
            let w0 = prompt_decimal("King W0 parameter", 5.0)?;
            cfg.model.king = Some(crate::config::KingModelConfig {
                w0,
                anisotropy: Decimal::ZERO,
            });
        }
        "nfw" => {
            let c = prompt_decimal("NFW concentration", 10.0)?;
            cfg.model.nfw = Some(crate::config::NfwModelConfig {
                concentration: c,
                virial_mass: cfg.model.total_mass,
                velocity_anisotropy: "isotropic".to_string(),
                beta: Decimal::ZERO,
            });
        }
        "zeldovich" => {
            let amp = prompt_decimal("Zeldovich amplitude", 0.01)?;
            let k = prompt_decimal("Zeldovich wave number", 1.0)?;
            cfg.model.zeldovich = Some(crate::config::ZeldovichConfig {
                amplitude: amp,
                wave_number: k,
                box_size: dec(100.0),
                redshift_initial: dec(50.0),
                cosmology_h: dec(0.7),
                cosmology_omega_m: dec(0.3),
                cosmology_omega_lambda: dec(0.7),
            });
        }
        "merger" => {
            let sep = prompt_decimal("Merger separation", 5.0)?;
            let ratio = prompt_decimal("Merger mass ratio", 1.0)?;
            cfg.model.merger = Some(crate::config::MergerConfig {
                separation: sep,
                mass_ratio: ratio,
                relative_velocity: [0.0, 0.3, 0.0],
                impact_parameter: dec(2.0),
                model_1: "plummer".to_string(),
                model_2: "plummer".to_string(),
                scale_radius_1: cfg.model.scale_radius,
                scale_radius_2: cfg.model.scale_radius,
            });
        }
        _ => {}
    }

    // 2. Domain
    eprintln!("\n--- Domain ---");
    cfg.domain.spatial_extent = prompt_decimal("Spatial extent (half-box)", 10.0)?;
    cfg.domain.velocity_extent = prompt_decimal("Velocity extent (half-box)", 5.0)?;
    cfg.domain.spatial_resolution = prompt_u32("Spatial resolution (per axis)", 8)?;
    cfg.domain.velocity_resolution = prompt_u32("Velocity resolution (per axis)", 8)?;

    let mem_gb = {
        let nx = cfg.domain.spatial_resolution as u64;
        let nv = cfg.domain.velocity_resolution as u64;
        (nx * nx * nx * nv * nv * nv) as f64 * 8.0 / 1e9
    };
    eprintln!("  (estimated memory: {mem_gb:.2} GB)");

    cfg.domain.boundary = prompt(
        "Boundary (periodic|truncated, periodic|open, isolated|truncated)",
        "periodic|truncated",
    )?;

    // 3. Solver
    eprintln!("\n--- Solver ---");
    cfg.solver.poisson = prompt(
        "Poisson solver (fft_periodic, fft_isolated)",
        "fft_periodic",
    )?;
    cfg.solver.integrator = prompt("Integrator (strang, yoshida, lie)", "strang")?;

    // 4. Time
    eprintln!("\n--- Time ---");
    cfg.time.t_final = prompt_decimal("Final time (t_final)", 10.0)?;
    cfg.time.cfl_factor = prompt_decimal("CFL factor", 0.5)?;
    cfg.time.dt_mode = prompt("Timestep mode (adaptive, fixed)", "adaptive")?;

    // 5. Exit conditions
    eprintln!("\n--- Exit conditions ---");
    cfg.exit.energy_drift_tolerance = prompt_f64("Energy drift tolerance (|ΔE/E|)", 0.5)?;
    cfg.exit.mass_drift_tolerance = prompt_f64("Mass drift tolerance (|ΔM/M|)", 0.1)?;

    // 6. Output
    eprintln!("\n--- Output ---");
    let output_path = prompt("Output TOML path", "generated_config.toml")?;

    // Validate
    let warnings = crate::config::validate::validate(&cfg);
    if !warnings.is_empty() {
        eprintln!("\nWarnings:");
        for w in &warnings {
            eprintln!("  - {w}");
        }
    }

    // Write
    let toml_str = toml::to_string_pretty(&cfg)?;
    std::fs::write(&output_path, &toml_str)?;
    eprintln!("\nConfig written to {output_path}");

    Ok(())
}

fn prompt(label: &str, default: &str) -> anyhow::Result<String> {
    eprint!("{label} [{default}]: ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn prompt_f64(label: &str, default: f64) -> anyhow::Result<f64> {
    let s = prompt(label, &format!("{default}"))?;
    Ok(s.parse().unwrap_or(default))
}

fn prompt_decimal(label: &str, default: f64) -> anyhow::Result<Decimal> {
    let s = prompt(label, &format!("{default}"))?;
    Ok(dec(s.parse::<f64>().unwrap_or(default)))
}

fn prompt_u32(label: &str, default: u32) -> anyhow::Result<u32> {
    let s = prompt(label, &format!("{default}"))?;
    Ok(s.parse().unwrap_or(default))
}
