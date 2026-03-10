//! Interactive CLI wizard for generating a TOML config file.

use std::io::{self, Write};

use crate::config::PhasmaConfig;

/// Run the interactive wizard, printing prompts to stderr and reading from stdin.
pub fn run_wizard() -> anyhow::Result<()> {
    eprintln!("phasma wizard: interactive configuration generator\n");

    let mut cfg = PhasmaConfig::default();

    // 1. Model type
    eprintln!("Available models: plummer, hernquist, king, nfw, zeldovich, merger");
    let model_type = prompt("Model type", "plummer")?;
    cfg.model.model_type = model_type.clone();
    cfg.model.total_mass = prompt_f64("Total mass", 1.0)?;
    cfg.model.scale_radius = prompt_f64("Scale radius", 1.0)?;

    match model_type.as_str() {
        "king" => {
            let w0 = prompt_f64("King W0 parameter", 5.0)?;
            cfg.model.king = Some(crate::config::KingModelConfig { w0 });
        }
        "nfw" => {
            let c = prompt_f64("NFW concentration", 10.0)?;
            cfg.model.nfw = Some(crate::config::NfwModelConfig { concentration: c });
        }
        "zeldovich" => {
            let amp = prompt_f64("Zeldovich amplitude", 0.01)?;
            let k = prompt_f64("Zeldovich wave number", 1.0)?;
            cfg.model.zeldovich = Some(crate::config::ZeldovichConfig {
                amplitude: amp,
                wave_number: k,
            });
        }
        "merger" => {
            let sep = prompt_f64("Merger separation", 5.0)?;
            let ratio = prompt_f64("Merger mass ratio", 1.0)?;
            cfg.model.merger = Some(crate::config::MergerConfig {
                separation: sep,
                mass_ratio: ratio,
            });
        }
        _ => {}
    }

    // 2. Domain
    eprintln!("\n--- Domain ---");
    cfg.domain.spatial_extent = prompt_f64("Spatial extent (half-box)", 10.0)?;
    cfg.domain.velocity_extent = prompt_f64("Velocity extent (half-box)", 5.0)?;
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
    cfg.time.t_final = prompt_f64("Final time (t_final)", 10.0)?;
    cfg.time.cfl_factor = prompt_f64("CFL factor", 0.5)?;
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

fn prompt_u32(label: &str, default: u32) -> anyhow::Result<u32> {
    let s = prompt(label, &format!("{default}"))?;
    Ok(s.parse().unwrap_or(default))
}
