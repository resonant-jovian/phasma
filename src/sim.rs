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
    pub total_mass: f64,
    pub momentum: [f64; 3],
    pub casimir_c2: f64,
    pub entropy: f64,
    /// Projected density ρ(x,y), flat row-major nx×ny grid.
    pub density_xy: Vec<f64>,
    pub density_nx: usize,
    pub density_ny: usize,
    /// Phase-space slice f(x,vx), flat row-major nx×nv grid.
    pub phase_slice: Vec<f64>,
    pub phase_nx: usize,
    pub phase_nv: usize,
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

        Self { state_rx, control_tx, task }
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
            let _ = state_tx.send(error_state(e.to_string()));
            return;
        }
    };

    let initial_energy = sim.diagnostics.history.first().map(|d| d.total_energy).unwrap_or(0.0);
    let t_final = {
        use rust_decimal::prelude::ToPrimitive;
        sim.domain.time_range.t_final.to_f64().unwrap_or(10.0)
    };
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

        match sim.step() {
            Ok(None) => {
                let state = extract_sim_state(&sim, initial_energy, t_final, None);
                if state_tx.send(state).is_err() {
                    return;
                }
            }
            Ok(Some(reason)) => {
                let exit = map_exit_reason(reason);
                let _ = state_tx.send(extract_sim_state(&sim, initial_energy, t_final, Some(exit)));
                return;
            }
            Err(e) => {
                eprintln!("caustic step error: {e}");
                return;
            }
        }
    }
}

fn build_caustic_sim(config_path: &str) -> anyhow::Result<caustic::Simulation> {
    use caustic::{
        Domain, FftPoisson, PlummerIC, SemiLagrangian, SpatialBoundType, StrangSplitting,
        VelocityBoundType, sample_on_grid,
    };

    let p = crate::toml::sim_params(config_path)?;

    if p.model_type != "plummer" {
        anyhow::bail!("unsupported model type '{}' — only 'plummer' is implemented", p.model_type);
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
        .integrator(StrangSplitting::new(p.cfl_factor))
        .initial_conditions(snap)
        .time_final(p.t_final)
        .exit_on_energy_drift(p.energy_tolerance)
        .build()
        .map_err(Into::into)
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
) -> SimState {
    let diag = sim.diagnostics.history.last().copied().unwrap_or_else(zero_diag);

    // Density projection ρ(x,y): sum DensityField over z-axis.
    let density = sim.repr.compute_density();
    let [nx1, nx2, nx3] = density.shape;
    let mut density_xy = vec![0.0f64; nx1 * nx2];
    for ix1 in 0..nx1 {
        for ix2 in 0..nx2 {
            for ix3 in 0..nx3 {
                density_xy[ix1 * nx2 + ix2] +=
                    density.data[ix1 * nx2 * nx3 + ix2 * nx3 + ix3];
            }
        }
    }

    // Phase-space slice f(x,vx): sum 6D snapshot over x2,x3,v2,v3.
    let snap = sim.repr.to_snapshot(sim.time);
    let [sx1, sx2, sx3, sv1, sv2, sv3] = snap.shape;
    let mut phase_slice = vec![0.0f64; sx1 * sv1];
    let stride_x1 = sx2 * sx3 * sv1 * sv2 * sv3;
    let stride_x2 = sx3 * sv1 * sv2 * sv3;
    let stride_x3 = sv1 * sv2 * sv3;
    let stride_v1 = sv2 * sv3;
    for ix1 in 0..sx1 {
        for iv1 in 0..sv1 {
            let mut val = 0.0f64;
            for ix2 in 0..sx2 {
                for ix3 in 0..sx3 {
                    for iv2 in 0..sv2 {
                        for iv3 in 0..sv3 {
                            let idx = ix1 * stride_x1
                                + ix2 * stride_x2
                                + ix3 * stride_x3
                                + iv1 * stride_v1
                                + iv2 * sv3
                                + iv3;
                            val += snap.data[idx];
                        }
                    }
                }
            }
            phase_slice[ix1 * sv1 + iv1] = val;
        }
    }

    SimState {
        t: sim.time,
        t_final,
        step: sim.step,
        total_energy: diag.total_energy,
        initial_energy,
        total_mass: diag.mass_in_box,
        momentum: diag.total_momentum,
        casimir_c2: diag.casimir_c2,
        entropy: diag.entropy,
        density_xy,
        density_nx: nx1,
        density_ny: nx2,
        phase_slice,
        phase_nx: sx1,
        phase_nv: sv1,
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

fn error_state(msg: String) -> SimState {
    eprintln!("caustic build error: {msg}");
    SimState {
        t: 0.0,
        t_final: 0.0,
        step: 0,
        total_energy: 0.0,
        initial_energy: 0.0,
        total_mass: 0.0,
        momentum: [0.0; 3],
        casimir_c2: 0.0,
        entropy: 0.0,
        density_xy: vec![],
        density_nx: 0,
        density_ny: 0,
        phase_slice: vec![],
        phase_nx: 0,
        phase_nv: 0,
        exit_reason: Some(ExitReason::UserStop),
    }
}
