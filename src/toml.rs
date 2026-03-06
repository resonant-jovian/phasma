use rust_decimal::Decimal;
use serde::Deserialize;
#[derive(Deserialize)]
pub(crate) struct InitConfig {
    pub(crate) model: InitModel,
    pub(crate) domain: InitDomain,
    pub(crate) solver: InitSolver,
    pub(crate) time: InitTime,
    pub(crate) output: InitOutput,
    pub(crate) exit: InitExit,
}
pub(crate) struct Config {
    pub(crate) model: Model,
    pub(crate) domain: Domain,
    pub(crate) solver: Solver,
    pub(crate) time: Time,
    pub(crate) output: Output,
    pub(crate) exit: Exit,
}
#[derive(Deserialize)]
pub(crate) struct InitModel {
    pub(crate) r#type: String,
    pub(crate) mass: String,
    pub(crate) scale_radius: String,
}
pub(crate) struct Model {
    pub(crate) r#type: String,
    pub(crate) mass: Decimal,
    pub(crate) scale_radius: Decimal,
}
#[derive(Deserialize)]
pub(crate) struct InitDomain {
    pub(crate) spatial_extent: String,
    pub(crate) velocity_extent: String,
    pub(crate) spatial_resolution: String,
    pub(crate) velocity_resolution: String,
    pub(crate) boundary: String,
}
pub(crate) struct Domain {
    pub(crate) spatial_extent: Decimal,
    pub(crate) velocity_extent: Decimal,
    pub(crate) spatial_resolution: u128,
    pub(crate) velocity_resolution: u128,
    pub(crate) boundary: String,
}
#[derive(Deserialize)]
pub(crate) struct InitSolver {
    pub(crate) representation: String,
    pub(crate) poisson: String,
    pub(crate) advection: String,
    pub(crate) splitting: String,
}
pub(crate) struct Solver {
    pub(crate) representation: String,
    pub(crate) poisson: String,
    pub(crate) advection: String,
    pub(crate) splitting: String,
}
#[derive(Deserialize)]
pub(crate) struct InitTime {
    pub(crate) t_final: String,
    pub(crate) dt: String,
    pub(crate) cfl_factor: String,
}
pub(crate) struct Time {
    pub(crate) t_final: Decimal,
    pub(crate) dt: String,
    pub(crate) cfl_factor: Decimal,
}
#[derive(Deserialize)]
pub(crate) struct InitOutput {
    pub(crate) interval: String,
    pub(crate) directory: String,
    pub(crate) format: String,
}
pub(crate) struct Output {
    pub(crate) interval: Decimal,
    pub(crate) directory: String,
    pub(crate) format: String,
}
#[derive(Deserialize)]
pub(crate) struct InitExit {
    pub(crate) energy_tolerance: String,
    pub(crate) mass_threshold: String,
}
pub(crate) struct Exit {
    pub(crate) energy_tolerance: Decimal,
    pub(crate) mass_threshold: Decimal,
}
impl InitConfig {
    fn to_config(self) -> anyhow::Result<Config> {
        let config = Config {
            model: Model {
                r#type: self.model.r#type,
                mass: Decimal::from_str_exact(&*self.model.mass)?,
                scale_radius: Decimal::from_str_exact(&*self.model.scale_radius)?,
            },
            domain: Domain {
                spatial_extent: Decimal::from_str_exact(&*self.domain.spatial_extent)?,
                velocity_extent: Decimal::from_str_exact(&*self.domain.velocity_extent)?,
                spatial_resolution: self.domain.spatial_resolution.parse::<u128>()?,
                velocity_resolution: self.domain.velocity_resolution.parse::<u128>()?,
                boundary: self.domain.boundary,
            },
            solver: Solver {
                representation: self.solver.representation,
                poisson: self.solver.poisson,
                advection: self.solver.advection,
                splitting: self.solver.splitting,
            },
            time: Time {
                t_final: Decimal::from_str_exact(&*self.time.t_final)?,
                dt: self.time.dt,
                cfl_factor: Decimal::from_str_exact(&*self.time.cfl_factor)?,
            },
            output: Output {
                interval: Decimal::from_str_exact(&*self.output.interval)?,
                directory: self.output.directory,
                format: self.output.format,
            },
            exit: Exit {
                energy_tolerance: Decimal::from_scientific(&*self.exit.energy_tolerance)?,
                mass_threshold: Decimal::from_str_exact(&*self.exit.mass_threshold)?,
            },
        };
        Ok(config)
    }
}
pub(crate) fn read_config(path: &str) -> anyhow::Result<Config> {
    let init_config: InitConfig = toml::from_slice(&std::fs::read(path)?)?;
    init_config.to_config()
}

/// Flat, f64-typed view of the TOML config used by sim.rs to build a caustic Simulation.
pub(crate) struct SimParams {
    pub model_type: String,
    pub mass: f64,
    pub scale_radius: f64,
    pub spatial_extent: f64,
    pub velocity_extent: f64,
    pub spatial_resolution: usize,
    pub velocity_resolution: usize,
    pub boundary: String,
    pub t_final: f64,
    pub cfl_factor: f64,
    pub energy_tolerance: f64,
    pub mass_threshold: f64,
    pub output_directory: String,
}

pub(crate) fn sim_params(path: &str) -> anyhow::Result<SimParams> {
    use rust_decimal::prelude::ToPrimitive;
    let c = read_config(path)?;
    Ok(SimParams {
        model_type: c.model.r#type,
        mass: c.model.mass.to_f64().unwrap(),
        scale_radius: c.model.scale_radius.to_f64().unwrap(),
        spatial_extent: c.domain.spatial_extent.to_f64().unwrap(),
        velocity_extent: c.domain.velocity_extent.to_f64().unwrap(),
        spatial_resolution: c.domain.spatial_resolution as usize,
        velocity_resolution: c.domain.velocity_resolution as usize,
        boundary: c.domain.boundary,
        t_final: c.time.t_final.to_f64().unwrap(),
        cfl_factor: c.time.cfl_factor.to_f64().unwrap(),
        energy_tolerance: c.exit.energy_tolerance.to_f64().unwrap(),
        mass_threshold: c.exit.mass_threshold.to_f64().unwrap(),
        output_directory: c.output.directory,
    })
}