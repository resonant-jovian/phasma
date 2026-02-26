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
        let (state_tx, state_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(run_simulation(config_path, state_tx, control_rx));
        Self { state_rx, control_tx, task }
    }
}

async fn run_simulation(
    _config_path: String,
    state_tx: mpsc::UnboundedSender<SimState>,
    mut control_rx: mpsc::UnboundedReceiver<SimControl>,
) {
    // TODO: caustic::Config::load(&config_path)
    // TODO: caustic::Simulation::new(config)
    // TODO: caustic::InitialConditions::generate(config.model)

    let t_final = 10.0_f64;
    let dt_sim = 0.1_f64; // simulated time per step

    const NX: usize = 32;
    const NY: usize = 32;
    const PHASE_NX: usize = 32;
    const PHASE_NV: usize = 32;

    // TODO: caustic::diagnostics::total_energy(&repr, &potential)
    let initial_energy = -0.5_f64;

    let mut t = 0.0_f64;
    let mut step = 0_u64;
    let mut paused = false;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100)); // 10 Hz

    loop {
        // Drain control messages non-blocking
        while let Ok(ctrl) = control_rx.try_recv() {
            match ctrl {
                SimControl::Pause => paused = true,
                SimControl::Resume => paused = false,
                SimControl::Stop => return,
            }
        }

        interval.tick().await;

        if paused {
            continue;
        }

        // TODO: caustic::TimeIntegrator::step(&mut repr, &mut potential, dt_sim)
        t += dt_sim;
        step += 1;

        let state = make_dummy_state(t, t_final, step, initial_energy, NX, NY, PHASE_NX, PHASE_NV);

        if t >= t_final {
            let mut final_state = state;
            final_state.exit_reason = Some(ExitReason::TimeLimitReached);
            let _ = state_tx.send(final_state);
            return;
        }

        if state_tx.send(state).is_err() {
            return; // receiver dropped — app exited
        }
    }
}

fn make_dummy_state(
    t: f64,
    t_final: f64,
    step: u64,
    initial_energy: f64,
    nx: usize,
    ny: usize,
    phase_nx: usize,
    phase_nv: usize,
) -> SimState {
    // TODO: caustic::PhaseSpaceRepr::compute_density() → project ρ(x,y)
    // Dummy: Plummer-like density ρ ∝ (1 + r²/a²)^(-5/2) with small oscillation
    let density_xy: Vec<f64> = (0..nx * ny)
        .map(|i| {
            let x = (i % nx) as f64 / nx as f64 - 0.5;
            let y = (i / nx) as f64 / ny as f64 - 0.5;
            let r2 = x * x + y * y;
            let osc = 1.0 + 0.05 * (t * 0.7).sin();
            (1.0 + r2 / 0.01).powf(-2.5) * osc
        })
        .collect();

    // TODO: caustic::PhaseSpaceRepr::phase_slice() → f(x,vx) at y=0
    // Dummy: drifting Gaussian sheet — simulates a cold dark matter stream
    let phase_slice: Vec<f64> = (0..phase_nx * phase_nv)
        .map(|i| {
            let x = (i % phase_nx) as f64 / phase_nx as f64 - 0.5;
            let vx = (i / phase_nx) as f64 / phase_nv as f64 - 0.5;
            let x0 = 0.1 * (t * 0.5).sin();
            let vx0 = 0.05 * (t * 0.5).cos();
            let dx = x - x0;
            let dv = vx - vx0;
            (-50.0 * (dx * dx + dv * dv)).exp()
        })
        .collect();

    // TODO: caustic::diagnostics::total_energy(&repr, &potential)
    let energy_drift_frac = 1e-5 * t * (1.0 + 0.1 * (t * 1.3).sin());
    let total_energy = initial_energy * (1.0 + energy_drift_frac);

    // TODO: caustic::diagnostics::total_mass(&repr)
    let total_mass = 1.0 - 1e-8 * t;

    // TODO: caustic::diagnostics::total_momentum(&repr)
    let momentum = [1e-10 * t.sin(), 1e-10 * t.cos(), 0.0];

    // TODO: caustic::diagnostics::casimir_c2(&repr)  ∫f² dx³dv³
    let casimir_c2 = 0.998 - 1e-6 * t;

    // TODO: caustic::diagnostics::entropy(&repr)  -∫f ln f dx³dv³
    let entropy = -2.5 + 0.001 * t;

    SimState {
        t,
        t_final,
        step,
        total_energy,
        initial_energy,
        total_mass,
        momentum,
        casimir_c2,
        entropy,
        density_xy,
        density_nx: nx,
        density_ny: ny,
        phase_slice,
        phase_nx,
        phase_nv,
        exit_reason: None,
    }
}
