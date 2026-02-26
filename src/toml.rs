use rust_decimal::Decimal;
use serde::Deserialize;
#[derive(Deserialize)]
pub struct InitConfig {
    model: InitModel,
    domain: InitDomain,
    solver: InitSolver,
    time: InitTime,
    output: InitOutput,
    exit: InitExit,
}
pub struct Config {
    model: Model,
    domain: Domain,
    solver: Solver,
    time: Time,
    output: Output,
    exit: Exit,
}
#[derive(Deserialize)]
struct InitModel {
    r#type: String,
    mass: String,
    scale_radius: String,
}
struct Model {
    r#type: String,
    mass: Decimal,
    scale_radius: Decimal,
}
#[derive(Deserialize)]
struct InitDomain {
    spatial_extent: String,
    velocity_extent: String,
    spatial_resolution: String,
    velocity_resolution: String,
    boundary: String,
}
struct Domain {
    spatial_extent: Decimal,
    velocity_extent: Decimal,
    spatial_resolution: u128,
    velocity_resolution: u128,
    boundary: String,
}
#[derive(Deserialize)]
struct InitSolver {
    representation: String,
    poisson: String,
    advection: String,
    splitting: String,
}
struct Solver {
    representation: String,
    poisson: String,
    advection: String,
    splitting: String,
}
#[derive(Deserialize)]
struct InitTime {
    t_final: String,
    dt: String,
    cfl_factor: String,
}
struct Time {
    t_final: Decimal,
    dt: String,
    cfl_factor: Decimal,
}
#[derive(Deserialize)]
struct InitOutput {
    interval: String,
    directory: String,
    format: String,
}
struct Output {
    interval: Decimal,
    directory: String,
    format: String,
}
#[derive(Deserialize)]
struct InitExit {
    energy_tolerance: String, //from_scientific
    mass_threshold: String,
}
struct Exit {
    energy_tolerance: Decimal, //from_scientific
    mass_threshold: Decimal,
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
pub fn read_config(path: &str) -> anyhow::Result<Config> {
    let init_config: InitConfig = toml::from_slice(&std::fs::read(path)?)?;
    init_config.to_config()
}