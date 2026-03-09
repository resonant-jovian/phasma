use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Snapshot of simulation state emitted at each diagnostic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimState {
    pub t: f64,
    pub t_final: f64,
    pub step: u64,
    pub total_energy: f64,
    pub initial_energy: f64,
    pub kinetic_energy: f64,
    pub potential_energy: f64,
    pub virial_ratio: f64,
    pub total_mass: f64,
    pub momentum: [f64; 3],
    pub casimir_c2: f64,
    pub entropy: f64,
    pub max_density: f64,
    pub step_wall_ms: f64,
    pub has_new_data: bool,
    /// Projected density ρ(x,y) — sum over z. Flat row-major nx×ny.
    pub density_xy: Vec<f64>,
    /// Projected density ρ(x,z) — sum over y. Flat row-major nx×nz.
    pub density_xz: Vec<f64>,
    /// Projected density ρ(y,z) — sum over x. Flat row-major ny×nz.
    pub density_yz: Vec<f64>,
    pub density_nx: usize,
    pub density_ny: usize,
    pub density_nz: usize,
    /// Phase-space projections f(x_i, v_j) for all 9 (i,j) combos.
    /// Indexed as phase_slices[dim_x * 3 + dim_v], each flat row-major nx×nv.
    pub phase_slices: Vec<Vec<f64>>,
    /// Legacy single slice (= phase_slices[0], x1-v1) for backward compat.
    pub phase_slice: Vec<f64>,
    pub phase_nx: usize,
    pub phase_nv: usize,
    /// Domain spatial half-extent (physical units).
    pub spatial_extent: f64,
    /// Gravitational constant used in this run.
    pub gravitational_constant: f64,
    /// Current adaptive timestep.
    pub dt: f64,
    pub exit_reason: Option<ExitReason>,
}

impl SimState {
    pub fn progress(&self) -> f64 {
        if self.t_final <= 0.0 {
            0.0
        } else {
            (self.t / self.t_final).clamp(0.0, 1.0)
        }
    }

    pub fn energy_drift(&self) -> f64 {
        if self.initial_energy == 0.0 {
            0.0
        } else {
            (self.total_energy - self.initial_energy) / self.initial_energy.abs()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    TimeLimitReached,
    SteadyState,
    EnergyDrift,
    MassLoss,
    CasimirDrift,
    CflViolation,
    WallClockLimit,
    UserStop,
    CausticFormed,
    VirialStabilized,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ExitReason::TimeLimitReached => "Time limit reached",
            ExitReason::SteadyState => "Steady state",
            ExitReason::EnergyDrift => "Energy drift threshold",
            ExitReason::MassLoss => "Mass loss threshold",
            ExitReason::CasimirDrift => "Casimir drift",
            ExitReason::CflViolation => "CFL violation",
            ExitReason::WallClockLimit => "Wall-clock limit",
            ExitReason::UserStop => "User stop",
            ExitReason::CausticFormed => "Caustic formed",
            ExitReason::VirialStabilized => "Virial ratio stabilized",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimControl {
    Pause,
    Resume,
    Stop,
}

pub struct SimHandle {
    pub state_rx: mpsc::UnboundedReceiver<SimState>,
    pub control_tx: mpsc::UnboundedSender<SimControl>,
    pub task: JoinHandle<()>,
}

impl SimHandle {
    pub fn spawn(config_path: String) -> Self {
        let (state_tx, state_rx) = mpsc::unbounded_channel::<SimState>();
        let (control_tx, mut control_rx) = mpsc::unbounded_channel::<SimControl>();
        let (std_ctrl_tx, std_ctrl_rx) = std::sync::mpsc::channel::<SimControl>();

        // Simulation is not Send, so it runs on a dedicated std thread.
        let sim_state_tx = state_tx.clone();
        std::thread::spawn(move || {
            run_caustic_sim(config_path, sim_state_tx, std_ctrl_rx);
        });

        // Bridge tokio control channel → std channel.
        let task = tokio::spawn(async move {
            while let Some(ctrl) = control_rx.recv().await {
                if std_ctrl_tx.send(ctrl).is_err() {
                    break;
                }
            }
        });

        Self {
            state_rx,
            control_tx,
            task,
        }
    }
}

fn run_caustic_sim(
    config_path: String,
    state_tx: mpsc::UnboundedSender<SimState>,
    ctrl_rx: std::sync::mpsc::Receiver<SimControl>,
) {
    let mut sim = match build_caustic_sim(&config_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("phasma: sim build error: {e:#}");
            let _ = state_tx.send(error_state(e.to_string()));
            return;
        }
    };

    let initial_energy = sim
        .diagnostics
        .history
        .first()
        .map(|d| d.total_energy)
        .unwrap_or(0.0);
    let t_final = {
        use rust_decimal::prelude::ToPrimitive;
        sim.domain.time_range.t_final.to_f64().unwrap_or(10.0)
    };
    let spatial_extent = {
        use rust_decimal::prelude::ToPrimitive;
        sim.domain.spatial.x1.to_f64().unwrap_or(10.0)
    };
    let grav_const = sim.g;
    let mut paused = false;

    loop {
        while let Ok(ctrl) = ctrl_rx.try_recv() {
            match ctrl {
                SimControl::Pause => paused = true,
                SimControl::Resume => paused = false,
                SimControl::Stop => return,
            }
        }
        if paused {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let step_start = Instant::now();
        match sim.step() {
            Ok(None) => {
                let wall_ms = step_start.elapsed().as_secs_f64() * 1000.0;
                let state = extract_sim_state(
                    &sim,
                    initial_energy,
                    t_final,
                    None,
                    wall_ms,
                    spatial_extent,
                    grav_const,
                );
                if state_tx.send(state).is_err() {
                    return;
                }
            }
            Ok(Some(reason)) => {
                let wall_ms = step_start.elapsed().as_secs_f64() * 1000.0;
                let exit = map_exit_reason(reason);
                let _ = state_tx.send(extract_sim_state(
                    &sim,
                    initial_energy,
                    t_final,
                    Some(exit),
                    wall_ms,
                    spatial_extent,
                    grav_const,
                ));
                return;
            }
            Err(_e) => {
                let _ = state_tx.send(error_state(format!("step error: {_e}")));
                return;
            }
        }
    }
}

fn build_caustic_sim(config_path: &str) -> anyhow::Result<caustic::Simulation> {
    // Try new PhasmaConfig format first, fall back to legacy toml.rs
    match crate::config::load(config_path) {
        Ok(cfg) => build_from_config(&cfg),
        Err(_new_err) => {
            // Legacy fallback
            match build_from_legacy(config_path) {
                Ok(sim) => Ok(sim),
                Err(_legacy_err) => {
                    // Return the new-format error since that's the preferred path
                    Err(_new_err
                        .context("failed to parse config (tried both new and legacy formats)"))
                }
            }
        }
    }
}

fn build_from_config(cfg: &crate::config::PhasmaConfig) -> anyhow::Result<caustic::Simulation> {
    use caustic::{
        Domain, FftIsolated, FftPoisson, LieSplitting, MassLossCondition, SemiLagrangian,
        SteadyStateCondition, StrangSplitting, VirialRelaxedCondition, WallClockCondition,
        YoshidaSplitting,
    };

    let g = cfg.domain.gravitational_constant;

    let (spatial_bc, velocity_bc) = parse_boundary(&cfg.domain.boundary);

    let domain = Domain::builder()
        .spatial_extent(cfg.domain.spatial_extent)
        .velocity_extent(cfg.domain.velocity_extent)
        .spatial_resolution(cfg.domain.spatial_resolution as i128)
        .velocity_resolution(cfg.domain.velocity_resolution as i128)
        .t_final(cfg.time.t_final)
        .spatial_bc(spatial_bc)
        .velocity_bc(velocity_bc)
        .build()?;

    // Build initial conditions based on model type
    let snap = build_ic(cfg, &domain, g)?;

    // Build Poisson solver
    let poisson: Box<dyn caustic::PoissonSolver> = match cfg.solver.poisson.as_str() {
        "fft_periodic" | "fft" => Box::new(FftPoisson::new(&domain)),
        "fft_isolated" => Box::new(FftIsolated::new(&domain)),
        other => anyhow::bail!("unsupported poisson solver '{other}'"),
    };

    // Build integrator
    let integrator: Box<dyn caustic::TimeIntegrator> = match cfg.solver.integrator.as_str() {
        "strang" => Box::new(StrangSplitting::new(g)),
        "yoshida" => Box::new(YoshidaSplitting::new(g)),
        "lie" => Box::new(LieSplitting::new(g)),
        other => anyhow::bail!("unsupported integrator '{other}'"),
    };

    let mut sim = caustic::Simulation::builder()
        .domain(domain)
        .poisson_solver_boxed(poisson)
        .advector(SemiLagrangian::new())
        .integrator_boxed(integrator)
        .initial_conditions(snap)
        .time_final(cfg.time.t_final)
        .cfl_factor(cfg.time.cfl_factor)
        .gravitational_constant(g)
        .exit_on_energy_drift(cfg.exit.energy_drift_tolerance)
        .build()?;

    // Wire exit conditions
    sim.exit_evaluator
        .add_condition(Box::new(MassLossCondition {
            threshold: cfg.exit.mass_drift_tolerance,
        }));

    if let Some(limit) = cfg.exit.wall_clock_limit {
        sim.exit_evaluator
            .add_condition(Box::new(WallClockCondition::new(limit)));
    }
    if cfg.exit.steady_state {
        sim.exit_evaluator
            .add_condition(Box::new(SteadyStateCondition::new(
                cfg.exit.steady_state_tolerance,
            )));
    }
    if cfg.exit.virial_equilibrium {
        sim.exit_evaluator
            .add_condition(Box::new(VirialRelaxedCondition {
                tolerance: cfg.exit.virial_tolerance,
            }));
    }

    Ok(sim)
}

fn build_ic(
    cfg: &crate::config::PhasmaConfig,
    domain: &caustic::Domain,
    g: f64,
) -> anyhow::Result<caustic::PhaseSpaceSnapshot> {
    use caustic::{
        CustomICArray, HernquistIC, KingIC, MergerIC, NfwIC, PlummerIC, ZeldovichSingleMode,
        sample_on_grid,
    };

    let m = cfg.model.total_mass;
    let a = cfg.model.scale_radius;

    match cfg.model.model_type.as_str() {
        "plummer" => {
            let ic = PlummerIC::new(m, a, g);
            Ok(sample_on_grid(&ic, domain))
        }
        "hernquist" => {
            let ic = HernquistIC::new(m, a, g);
            Ok(sample_on_grid(&ic, domain))
        }
        "king" => {
            let king = cfg
                .model
                .king
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("king model requires [model.king] with w0"))?;
            let ic = KingIC::new(m, king.w0, a, g);
            Ok(sample_on_grid(&ic, domain))
        }
        "nfw" => {
            let nfw = cfg.model.nfw.as_ref().ok_or_else(|| {
                anyhow::anyhow!("nfw model requires [model.nfw] with concentration")
            })?;
            let ic = NfwIC::new(m, a, nfw.concentration, g);
            Ok(sample_on_grid(&ic, domain))
        }
        "zeldovich" => {
            let z = cfg.model.zeldovich.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "zeldovich requires [model.zeldovich] with amplitude and wave_number"
                )
            })?;
            let mean_density = m / (2.0 * cfg.domain.spatial_extent).powi(3);
            let sigma_v = 0.1; // small thermal spread for cold dark matter
            let ic = ZeldovichSingleMode {
                mean_density,
                amplitude: z.amplitude,
                wavenumber: z.wave_number,
                sigma_v,
            };
            Ok(ic.sample_on_grid(domain))
        }
        "merger" => {
            let merger = cfg.model.merger.as_ref().ok_or_else(|| {
                anyhow::anyhow!("merger requires [model.merger] with separation and mass_ratio")
            })?;
            let m1 = m / (1.0 + merger.mass_ratio);
            let m2 = m - m1;
            let body1 = Box::new(PlummerIC::new(m1, a, g));
            let body2 = Box::new(PlummerIC::new(m2, a, g));
            let sep = [merger.separation, 0.0, 0.0];
            let vel = [0.0, 0.0, 0.0];
            let ic = MergerIC::new(body1, m1, body2, m2, sep, vel, 0.0);
            Ok(ic.sample_on_grid(domain))
        }
        "custom_file" => {
            let cf = cfg.model.custom_file.as_ref().ok_or_else(|| {
                anyhow::anyhow!("custom_file requires [model.custom_file] with file_path")
            })?;
            let ic = CustomICArray::from_npy(&cf.file_path, domain)?;
            Ok(ic.snapshot)
        }
        other => anyhow::bail!("unsupported model type '{other}'"),
    }
}

fn build_from_legacy(config_path: &str) -> anyhow::Result<caustic::Simulation> {
    use caustic::{Domain, FftPoisson, PlummerIC, SemiLagrangian, StrangSplitting, sample_on_grid};

    let p = crate::toml::sim_params(config_path)?;

    if p.model_type != "plummer" {
        anyhow::bail!(
            "unsupported model type '{}' in legacy format — only 'plummer' is supported",
            p.model_type
        );
    }

    let (spatial_bc, velocity_bc) = parse_boundary(&p.boundary);

    let domain = Domain::builder()
        .spatial_extent(p.spatial_extent)
        .velocity_extent(p.velocity_extent)
        .spatial_resolution(p.spatial_resolution as i128)
        .velocity_resolution(p.velocity_resolution as i128)
        .t_final(p.t_final)
        .spatial_bc(spatial_bc)
        .velocity_bc(velocity_bc)
        .build()?;

    let ic = PlummerIC::new(p.mass, p.scale_radius, 1.0);
    let snap = sample_on_grid(&ic, &domain);
    let poisson = FftPoisson::new(&domain);

    caustic::Simulation::builder()
        .domain(domain)
        .poisson_solver(poisson)
        .advector(SemiLagrangian::new())
        .integrator(StrangSplitting::new(1.0))
        .initial_conditions(snap)
        .time_final(p.t_final)
        .cfl_factor(p.cfl_factor)
        .exit_on_energy_drift(p.energy_tolerance)
        .build()
}

fn parse_boundary(s: &str) -> (caustic::SpatialBoundType, caustic::VelocityBoundType) {
    use caustic::{SpatialBoundType, VelocityBoundType};
    let parts: Vec<&str> = s.split('|').collect();
    let spatial = match parts.first().map(|s| s.trim()) {
        Some("periodic") => SpatialBoundType::Periodic,
        Some("isolated") => SpatialBoundType::Isolated,
        Some("reflecting") => SpatialBoundType::Reflecting,
        _ => SpatialBoundType::Periodic,
    };
    let velocity = match parts.get(1).map(|s| s.trim()) {
        Some("truncated") => VelocityBoundType::Truncated,
        Some("open") => VelocityBoundType::Open,
        _ => VelocityBoundType::Open,
    };
    (spatial, velocity)
}

fn extract_sim_state(
    sim: &caustic::Simulation,
    initial_energy: f64,
    t_final: f64,
    exit_reason: Option<ExitReason>,
    wall_ms: f64,
    spatial_extent: f64,
    grav_const: f64,
) -> SimState {
    let diag = sim
        .diagnostics
        .history
        .last()
        .copied()
        .unwrap_or_else(zero_diag);

    // Density projections over 3 axes
    let density = sim.repr.compute_density();
    let [nx1, nx2, nx3] = density.shape;

    let mut density_xy = vec![0.0f64; nx1 * nx2];
    let mut density_xz = vec![0.0f64; nx1 * nx3];
    let mut density_yz = vec![0.0f64; nx2 * nx3];
    let mut max_density = 0.0f64;

    for ix1 in 0..nx1 {
        for ix2 in 0..nx2 {
            for ix3 in 0..nx3 {
                let v = density.data[ix1 * nx2 * nx3 + ix2 * nx3 + ix3];
                density_xy[ix1 * nx2 + ix2] += v;
                density_xz[ix1 * nx3 + ix3] += v;
                density_yz[ix2 * nx3 + ix3] += v;
                if v > max_density {
                    max_density = v;
                }
            }
        }
    }

    // Phase-space projections f(x_i, v_j) for all 9 (dim_x, dim_v) combinations.
    let snap = sim.repr.to_snapshot(sim.time);
    let [sx1, sx2, sx3, sv1, sv2, sv3] = snap.shape;
    let s = [sx1, sx2, sx3, sv1, sv2, sv3];
    let phase_slices = compute_all_phase_slices(&snap.data, s);
    let phase_slice = phase_slices[0].clone(); // x1-v1 for backward compat

    SimState {
        t: sim.time,
        t_final,
        step: sim.step,
        total_energy: diag.total_energy,
        initial_energy,
        kinetic_energy: diag.kinetic_energy,
        potential_energy: diag.potential_energy,
        virial_ratio: diag.virial_ratio,
        total_mass: diag.mass_in_box,
        momentum: diag.total_momentum,
        casimir_c2: diag.casimir_c2,
        entropy: diag.entropy,
        max_density,
        step_wall_ms: wall_ms,
        has_new_data: true,
        density_xy,
        density_xz,
        density_yz,
        density_nx: nx1,
        density_ny: nx2,
        density_nz: nx3,
        phase_slices,
        phase_slice,
        phase_nx: sx1,
        phase_nv: sv1,
        spatial_extent,
        gravitational_constant: grav_const,
        dt: 0.0, // computed by consumer from time differences
        exit_reason,
    }
}

fn map_exit_reason(r: caustic::ExitReason) -> ExitReason {
    use caustic::ExitReason as C;
    match r {
        C::TimeLimitReached => ExitReason::TimeLimitReached,
        C::SteadyState => ExitReason::SteadyState,
        C::EnergyDrift => ExitReason::EnergyDrift,
        C::MassLoss => ExitReason::MassLoss,
        C::CasimirDrift => ExitReason::CasimirDrift,
        C::CflViolation => ExitReason::CflViolation,
        C::WallClockLimit => ExitReason::WallClockLimit,
        C::FirstCausticFormed => ExitReason::CausticFormed,
        C::VirialRelaxed => ExitReason::VirialStabilized,
        C::UserDefined => ExitReason::UserStop,
    }
}

fn zero_diag() -> caustic::GlobalDiagnostics {
    caustic::GlobalDiagnostics {
        time: 0.0,
        total_energy: 0.0,
        kinetic_energy: 0.0,
        potential_energy: 0.0,
        virial_ratio: 0.0,
        total_momentum: [0.0; 3],
        total_angular_momentum: [0.0; 3],
        casimir_c2: 0.0,
        entropy: 0.0,
        mass_in_box: 0.0,
    }
}

/// Compute all 9 phase-space 2D projections f(x_i, v_j) from 6D data.
/// Returns Vec of 9 flat arrays, indexed by dim_x * 3 + dim_v.
fn compute_all_phase_slices(data: &[f64], shape: [usize; 6]) -> Vec<Vec<f64>> {
    let [sx1, sx2, sx3, sv1, sv2, sv3] = shape;
    let spatial = [sx1, sx2, sx3];
    let velocity = [sv1, sv2, sv3];

    (0..9)
        .map(|idx| {
            let dim_x = idx / 3;
            let dim_v = idx % 3;
            let nx = spatial[dim_x];
            let nv = velocity[dim_v];
            let mut out = vec![0.0f64; nx * nv];

            for i1 in 0..sx1 {
                for i2 in 0..sx2 {
                    for i3 in 0..sx3 {
                        for j1 in 0..sv1 {
                            for j2 in 0..sv2 {
                                for j3 in 0..sv3 {
                                    let flat = i1 * sx2 * sx3 * sv1 * sv2 * sv3
                                        + i2 * sx3 * sv1 * sv2 * sv3
                                        + i3 * sv1 * sv2 * sv3
                                        + j1 * sv2 * sv3
                                        + j2 * sv3
                                        + j3;
                                    let ix = [i1, i2, i3][dim_x];
                                    let iv = [j1, j2, j3][dim_v];
                                    out[ix * nv + iv] += data[flat];
                                }
                            }
                        }
                    }
                }
            }
            out
        })
        .collect()
}

fn error_state(_msg: String) -> SimState {
    // Error is reported via the channel, not stderr (would corrupt TUI).
    SimState {
        t: 0.0,
        t_final: 0.0,
        step: 0,
        total_energy: 0.0,
        initial_energy: 0.0,
        kinetic_energy: 0.0,
        potential_energy: 0.0,
        virial_ratio: 0.0,
        total_mass: 0.0,
        momentum: [0.0; 3],
        casimir_c2: 0.0,
        entropy: 0.0,
        max_density: 0.0,
        step_wall_ms: 0.0,
        has_new_data: false,
        density_xy: vec![],
        density_xz: vec![],
        density_yz: vec![],
        density_nx: 0,
        density_ny: 0,
        density_nz: 0,
        phase_slices: vec![vec![]; 9],
        phase_slice: vec![],
        phase_nx: 0,
        phase_nv: 0,
        spatial_extent: 0.0,
        gravitational_constant: 0.0,
        dt: 0.0,
        exit_reason: Some(ExitReason::UserStop),
    }
}
