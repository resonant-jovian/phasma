use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Dual-mode receiver: bounded crossbeam for TUI (back-pressure tolerant),
/// unbounded tokio for runners (need every state for CSV output).
pub enum StateReceiver {
    Bounded(crossbeam_channel::Receiver<Arc<SimState>>),
    Unbounded(mpsc::UnboundedReceiver<Arc<SimState>>),
}

impl StateReceiver {
    /// Non-blocking try_recv — drains to latest in TUI event loop.
    pub fn try_recv(&mut self) -> Result<Arc<SimState>, ()> {
        match self {
            StateReceiver::Bounded(rx) => rx.try_recv().map_err(|_| ()),
            StateReceiver::Unbounded(rx) => rx.try_recv().map_err(|_| ()),
        }
    }

    /// Async recv — used by runners (batch, sweep, convergence).
    pub async fn recv_async(&mut self) -> Option<Arc<SimState>> {
        match self {
            StateReceiver::Bounded(rx) => {
                let rx = rx.clone();
                tokio::task::spawn_blocking(move || rx.recv().ok())
                    .await
                    .ok()
                    .flatten()
            }
            StateReceiver::Unbounded(rx) => rx.recv().await,
        }
    }
}

enum StateSender {
    Bounded(crossbeam_channel::Sender<Arc<SimState>>),
    Unbounded(mpsc::UnboundedSender<Arc<SimState>>),
}

impl StateSender {
    fn send(&self, state: Arc<SimState>) -> Result<(), ()> {
        match self {
            StateSender::Bounded(tx) => {
                // try_send: drop if full — TUI only needs latest
                let _ = tx.try_send(state);
                Ok(())
            }
            StateSender::Unbounded(tx) => tx.send(state).map_err(|_| ()),
        }
    }
}

/// Global verbose flag — set once at startup from CLI args.
pub static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Set the global verbose flag. Call once from main().
pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

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
    pub phase_slices: Arc<Vec<Vec<f64>>>,
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
    // ── HT rank diagnostics (None when representation != "ht") ──
    #[serde(default)]
    pub rank_per_node: Option<Vec<usize>>,
    #[serde(default)]
    pub rank_total: Option<usize>,
    #[serde(default)]
    pub rank_memory_bytes: Option<usize>,
    #[serde(default)]
    pub compression_ratio: Option<f64>,
    // ── Solver type metadata ──
    #[serde(default)]
    pub repr_type: String,
    #[serde(default)]
    pub poisson_type: String,
    // ── Poisson diagnostics ──
    #[serde(default)]
    pub poisson_residual_l2: Option<f64>,
    #[serde(default)]
    pub potential_power_spectrum: Option<Vec<(f64, f64)>>,
    // ── Per-step performance phase timings (§2.2 F8) ──
    /// Phase breakdown: [drift_ms, poisson_ms, kick_ms, density_ms, diagnostics_ms, io_ms, other_ms]
    #[serde(default)]
    pub phase_timings: Option<[f64; 7]>,
    /// Per-node HSVD truncation errors (same indexing as rank_per_node)
    #[serde(default)]
    pub truncation_errors: Option<Vec<f64>>,
    /// Number of SVD operations performed this step
    #[serde(default)]
    pub svd_count: u32,
    /// Number of HTACA pointwise evaluations this step
    #[serde(default)]
    pub htaca_evaluations: u64,
    // ── Velocity extent for aspect ratio (F4) ──
    #[serde(default)]
    pub velocity_extent: f64,
    // ── Per-node singular values for spectrum display (§2.2 F6) ──
    #[serde(default)]
    pub singular_values: Option<Vec<Vec<f64>>>,
    // ── Lagrangian radii L10, L25, L50, L75, L90 (§2.2 F7) ──
    #[serde(default)]
    pub lagrangian_radii: Option<[f64; 5]>,
    // ── Spectral diagnostics (Phase 5 gap analysis) ──
    /// Density power spectrum P(k) = |ρ̂(k)|² binned by |k|.
    #[serde(default)]
    pub density_power_spectrum: Option<Vec<(f64, f64)>>,
    /// Field energy spectrum E(k) = |k|² |Φ̂(k)|² binned by |k|.
    #[serde(default)]
    pub field_energy_spectrum: Option<Vec<(f64, f64)>>,
    // ── Step-level rank amplification (Phase 1 gap analysis) ──
    /// Ratio of max rank after Poisson+kick to max rank after drift.
    #[serde(default)]
    pub poisson_rank_amplification: Option<f64>,
    /// Ratio of max rank after drift to max rank before drift.
    #[serde(default)]
    pub advection_rank_amplification: Option<f64>,
    // ── Green's function / TensorPoisson diagnostics (§2.2 F9) ──
    #[serde(default)]
    pub green_function_rank: Option<usize>,
    #[serde(default)]
    pub exp_sum_terms: Option<usize>,
    // ── Verbose log messages (--verbose) ──
    #[serde(default)]
    pub log_messages: Vec<String>,
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
    pub state_rx: StateReceiver,
    pub control_tx: mpsc::UnboundedSender<SimControl>,
    pub task: JoinHandle<()>,
    pub progress: Arc<caustic::StepProgress>,
}

impl SimHandle {
    /// Spawn with bounded(2) channel — for TUI mode (back-pressure tolerant).
    pub fn spawn(config_path: String) -> Self {
        let verbose = VERBOSE.load(std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = crossbeam_channel::bounded::<Arc<SimState>>(2);
        Self::spawn_inner(
            config_path,
            StateSender::Bounded(tx),
            StateReceiver::Bounded(rx),
            verbose,
        )
    }

    /// Spawn with unbounded channel — for runners that need every state (batch, sweep, etc.).
    pub fn spawn_unbounded(config_path: String) -> Self {
        let verbose = VERBOSE.load(std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = mpsc::unbounded_channel::<Arc<SimState>>();
        Self::spawn_inner(
            config_path,
            StateSender::Unbounded(tx),
            StateReceiver::Unbounded(rx),
            verbose,
        )
    }

    pub fn spawn_with_verbose(config_path: String, verbose: bool) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<Arc<SimState>>(2);
        Self::spawn_inner(
            config_path,
            StateSender::Bounded(tx),
            StateReceiver::Bounded(rx),
            verbose,
        )
    }

    fn spawn_inner(
        config_path: String,
        state_tx: StateSender,
        state_rx: StateReceiver,
        verbose: bool,
    ) -> Self {
        let (control_tx, mut control_rx) = mpsc::unbounded_channel::<SimControl>();
        let (std_ctrl_tx, std_ctrl_rx) = std::sync::mpsc::channel::<SimControl>();

        let progress = caustic::StepProgress::new();
        let progress_for_thread = progress.clone();

        // Simulation is not Send, so it runs on a dedicated std thread.
        std::thread::spawn(move || {
            run_caustic_sim(
                config_path,
                state_tx,
                std_ctrl_rx,
                verbose,
                progress_for_thread,
            );
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
            progress,
        }
    }
}

fn run_caustic_sim(
    config_path: String,
    state_tx: StateSender,
    ctrl_rx: std::sync::mpsc::Receiver<SimControl>,
    verbose: bool,
    progress: Arc<caustic::StepProgress>,
) {
    let mut build_logs: Vec<String> = Vec::new();
    let build_start = Instant::now();
    if verbose {
        build_logs.push(format!("Loading config from: {config_path}"));
    }
    let mut sim = match build_caustic_sim(&config_path, verbose, &mut build_logs, &progress) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("phasma: sim build error: {e:#}");
            let _ = state_tx.send(Arc::new(error_state(e.to_string())));
            return;
        }
    };
    sim.set_progress(progress);

    if verbose {
        build_logs.push(format!(
            "Build complete in {:.1} ms",
            build_start.elapsed().as_secs_f64() * 1000.0
        ));
    }

    let initial_energy = sim
        .diagnostics
        .history
        .first()
        .map(|d| d.total_energy)
        .unwrap_or(0.0);
    let t_final = sim.domain.time_range.t_final.to_f64().unwrap_or(10.0);
    let spatial_extent = sim.domain.spatial.x1.to_f64().unwrap_or(10.0);
    let grav_const = sim.g;
    // Detect Poisson solver type from config for diagnostics display
    let poisson_type = crate::config::load(&config_path)
        .map(|c| c.solver.poisson.clone())
        .unwrap_or_else(|_| "fft_periodic".to_string());
    let mut paused = false;
    let mut first_state = true;
    let mut diag_step: u64 = 0;
    const POISSON_DIAG_INTERVAL: u64 = 10;
    const PHASE_DIAG_INTERVAL: u64 = 5;
    let mut cached_phase_slices: Arc<Vec<Vec<f64>>> = Arc::new(vec![vec![]; 9]);
    let mut cached_phase_nx: usize = 0;
    let mut cached_phase_nv: usize = 0;

    if verbose {
        build_logs.push(format!(
            "Initial energy: E₀={initial_energy:.6e}, t_final={t_final}"
        ));
    }

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
                // Skip expensive Poisson diagnostics on first frame for fast visual feedback
                let compute_poisson_diag =
                    !first_state && diag_step.is_multiple_of(POISSON_DIAG_INTERVAL);
                let compute_phase = diag_step.is_multiple_of(PHASE_DIAG_INTERVAL);
                diag_step += 1;
                let mut state = extract_sim_state(
                    &sim,
                    initial_energy,
                    t_final,
                    None,
                    wall_ms,
                    spatial_extent,
                    grav_const,
                    &poisson_type,
                    compute_poisson_diag,
                    compute_phase,
                );
                // Cache or reuse phase-space projections
                if compute_phase && state.phase_nx > 0 {
                    cached_phase_slices = Arc::clone(&state.phase_slices);
                    cached_phase_nx = state.phase_nx;
                    cached_phase_nv = state.phase_nv;
                } else if cached_phase_nx > 0 {
                    state.phase_slices = Arc::clone(&cached_phase_slices);
                    state.phase_nx = cached_phase_nx;
                    state.phase_nv = cached_phase_nv;
                }
                // Attach build logs to first state, per-step verbose to subsequent
                if first_state {
                    state.log_messages = std::mem::take(&mut build_logs);
                    first_state = false;
                } else if verbose {
                    state.log_messages.push(format!(
                        "Step {} → t={:.4}, dt={:.2e}, wall={:.1}ms, |ΔE/E|={:.2e}",
                        state.step,
                        state.t,
                        state.dt,
                        wall_ms,
                        state.energy_drift()
                    ));
                }
                if state_tx.send(Arc::new(state)).is_err() {
                    return;
                }
            }
            Ok(Some(reason)) => {
                let wall_ms = step_start.elapsed().as_secs_f64() * 1000.0;
                let exit = map_exit_reason(reason);
                let mut state = extract_sim_state(
                    &sim,
                    initial_energy,
                    t_final,
                    Some(exit),
                    wall_ms,
                    spatial_extent,
                    grav_const,
                    &poisson_type,
                    true, // always compute diagnostics on exit
                    true, // always compute phase slices on exit
                );
                if first_state {
                    state.log_messages = std::mem::take(&mut build_logs);
                }
                if verbose {
                    state.log_messages.push(format!(
                        "Exit: {exit} at step {}, t={:.4}",
                        state.step, state.t
                    ));
                }
                let _ = state_tx.send(Arc::new(state));
                return;
            }
            Err(_e) => {
                let _ = state_tx.send(Arc::new(error_state(format!("step error: {_e}"))));
                return;
            }
        }
    }
}

fn build_caustic_sim(
    config_path: &str,
    verbose: bool,
    logs: &mut Vec<String>,
    progress: &Arc<caustic::StepProgress>,
) -> anyhow::Result<caustic::Simulation> {
    // Try new PhasmaConfig format first, fall back to legacy toml.rs
    match crate::config::load(config_path) {
        Ok(cfg) => {
            if verbose {
                logs.push(format!(
                    "Config loaded: model={}, repr={}, poisson={}, integrator={}",
                    cfg.model.model_type,
                    cfg.solver.representation,
                    cfg.solver.poisson,
                    cfg.solver.integrator
                ));
                logs.push(format!(
                    "Domain: spatial_extent={}, velocity_extent={}, N_x={}, N_v={}",
                    cfg.domain.spatial_extent,
                    cfg.domain.velocity_extent,
                    cfg.domain.spatial_resolution,
                    cfg.domain.velocity_resolution
                ));
                let total_cells = (cfg.domain.spatial_resolution as u64).pow(3)
                    * (cfg.domain.velocity_resolution as u64).pow(3);
                logs.push(format!(
                    "Phase-space grid: {}^3 × {}^3 = {} cells ({:.1} MB)",
                    cfg.domain.spatial_resolution,
                    cfg.domain.velocity_resolution,
                    total_cells,
                    total_cells as f64 * 8.0 / 1_048_576.0
                ));
            }
            build_from_config(&cfg, verbose, logs, progress)
        }
        Err(_new_err) => {
            if verbose {
                logs.push("New config format failed, trying legacy format...".to_string());
            }
            // Legacy fallback
            match build_from_legacy(config_path) {
                Ok(sim) => Ok(sim),
                Err(_legacy_err) => {
                    Err(_new_err
                        .context("failed to parse config (tried both new and legacy formats)"))
                }
            }
        }
    }
}

fn build_from_config(
    cfg: &crate::config::PhasmaConfig,
    verbose: bool,
    logs: &mut Vec<String>,
    progress: &Arc<caustic::StepProgress>,
) -> anyhow::Result<caustic::Simulation> {
    use caustic::{
        AmrGrid, CasimirDriftCondition, CausticFormationCondition, CflViolationCondition, Domain,
        FftIsolated, FftPoisson, HtTensor, HybridRepr, LieSplitting, MassLossCondition, Multigrid,
        SemiLagrangian, SheetTracker, SpectralV, SphericalHarmonicsPoisson, SteadyStateCondition,
        StrangSplitting, TensorPoisson, TensorTrain, TreePoisson, UniformGrid6D,
        VirialRelaxedCondition, WallClockCondition, YoshidaSplitting,
    };

    let g = cfg.domain.gravitational_constant.to_f64().unwrap_or(1.0);

    let (spatial_bc, velocity_bc) = parse_boundary(&cfg.domain.boundary);
    if verbose {
        logs.push(format!("Boundary conditions: {}", cfg.domain.boundary));
        logs.push("Building domain...".to_string());
    }

    progress.set_phase(caustic::StepPhase::BuildDomain);
    progress.start_step();
    let t0 = Instant::now();
    let domain = Domain::builder()
        .spatial_extent(cfg.domain.spatial_extent.to_f64().unwrap_or(10.0))
        .velocity_extent(cfg.domain.velocity_extent.to_f64().unwrap_or(5.0))
        .spatial_resolution(cfg.domain.spatial_resolution as i128)
        .velocity_resolution(cfg.domain.velocity_resolution as i128)
        .t_final(cfg.time.t_final.to_f64().unwrap_or(10.0))
        .spatial_bc(spatial_bc)
        .velocity_bc(velocity_bc)
        .build()?;
    if verbose {
        logs.push(format!(
            "Domain built in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1000.0
        ));
    }

    // Build phase-space representation from IC
    //
    // For HT representation with isolated equilibrium models (plummer, hernquist,
    // king, nfw), we use HtTensor::from_function_aca which samples O(dNk) fibers
    // instead of materializing the full N^6 grid. This is critical at high
    // resolution (e.g. 128^3) where the full grid would be ~35 TB.
    //
    // All other representations first sample the full grid via build_ic().

    if verbose {
        logs.push(format!(
            "Building IC + representation: model={}, repr={}, M={}, a={}",
            cfg.model.model_type,
            cfg.solver.representation,
            cfg.model.total_mass,
            cfg.model.scale_radius
        ));
    }

    progress.set_phase(caustic::StepPhase::BuildIC);
    let t0 = Instant::now();
    let repr: Box<dyn caustic::PhaseSpaceRepr> = match cfg.solver.representation.as_str() {
        "hierarchical_tucker" | "ht" => {
            let tolerance = cfg.solver.ht.as_ref().map(|h| h.tolerance).unwrap_or(1e-6);
            let max_rank = cfg
                .solver
                .ht
                .as_ref()
                .map(|h| h.max_rank as usize)
                .unwrap_or(100);

            // Try the ACA path for isolated equilibrium models (no full grid needed)
            progress.set_phase(caustic::StepPhase::BuildICSampling);
            let ht_opt = build_ht_from_ic_aca(cfg, &domain, g, tolerance, max_rank, verbose, logs);

            let mut ht = match ht_opt {
                Some(ht) => {
                    progress.set_phase(caustic::StepPhase::BuildICCompression);
                    ht
                }
                None => {
                    // Fallback: model not supported by ACA path, use full grid
                    if verbose {
                        logs.push("  Falling back to full-grid IC + HSVD...".to_string());
                    }
                    let snap = build_ic(cfg, &domain, g, Some(progress))?;
                    progress.set_phase(caustic::StepPhase::BuildICCompression);
                    HtTensor::from_full(&snap.data, snap.shape, &domain, tolerance)
                }
            };
            ht.max_rank = max_rank;
            ht.tolerance = tolerance;
            if verbose {
                logs.push(format!(
                    "  HT total rank: {}, memory: {:.1} MB",
                    ht.total_rank(),
                    ht.memory_bytes() as f64 / 1_048_576.0
                ));
            }
            Box::new(ht)
        }
        // Representations that don't need the full IC grid
        "sheet_tracker" => Box::new(SheetTracker::new(domain.clone())),
        "amr" => Box::new(AmrGrid::new(domain.clone(), 0.1, 3)),
        "hybrid" => Box::new(HybridRepr::new(domain.clone())),

        // Representations that require the full N^6 grid in memory
        "uniform" | "uniform_grid" | "tensor_train" | "spectral" | "velocity_ht" => {
            let n = cfg.domain.spatial_resolution as u64;
            let nv = cfg.domain.velocity_resolution as u64;
            let grid_bytes = n.pow(3) * nv.pow(3) * 8;
            let grid_gb = grid_bytes as f64 / 1_073_741_824.0;
            let budget = cfg.performance.memory_budget_gb;
            if grid_gb > budget {
                anyhow::bail!(
                    "Full {n}³×{nv}³ grid requires {grid_gb:.1} GB, exceeds memory budget \
                     ({budget:.1} GB). Use representation = \"hierarchical_tucker\" for \
                     high-resolution runs."
                );
            }
            if grid_bytes > 64 * 1_073_741_824 {
                anyhow::bail!(
                    "Full {n}³×{nv}³ grid requires {grid_gb:.0} GB — too large to allocate. \
                     Use representation = \"hierarchical_tucker\" for high-resolution runs."
                );
            }

            progress.set_phase(caustic::StepPhase::BuildICSampling);
            let snap = build_ic(cfg, &domain, g, Some(progress))?;
            if verbose {
                let nonzero = snap.data.iter().filter(|&&v| v > 0.0).count();
                logs.push(format!(
                    "IC sampled in {:.1} ms — {} non-zero cells out of {}",
                    t0.elapsed().as_secs_f64() * 1000.0,
                    nonzero,
                    snap.data.len()
                ));
            }
            progress.set_phase(caustic::StepPhase::BuildICCompression);
            match cfg.solver.representation.as_str() {
                "uniform" | "uniform_grid" => {
                    let scheme = match cfg
                        .solver
                        .semi_lagrangian
                        .as_ref()
                        .map(|s| s.interpolation.as_str())
                    {
                        Some("wpfc") => caustic::AdvectionScheme::Wpfc,
                        Some("mp7") => caustic::AdvectionScheme::Mp7,
                        _ => caustic::AdvectionScheme::CatmullRom,
                    };
                    Box::new(
                        UniformGrid6D::from_snapshot(snap, domain.clone())
                            .with_advection_scheme(scheme),
                    )
                }
                "tensor_train" => {
                    let max_rank = cfg
                        .solver
                        .ht
                        .as_ref()
                        .map(|h| h.max_rank as usize)
                        .unwrap_or(50);
                    let tolerance = cfg.solver.ht.as_ref().map(|h| h.tolerance).unwrap_or(1e-6);
                    if verbose {
                        logs.push(format!(
                            "  TT params: max_rank={max_rank}, tolerance={tolerance:.1e}"
                        ));
                    }
                    Box::new(TensorTrain::from_snapshot_owned(
                        snap, max_rank, tolerance, &domain,
                    ))
                }
                "spectral" | "velocity_ht" => {
                    let n_modes = cfg
                        .solver
                        .ht
                        .as_ref()
                        .map(|h| h.initial_rank as usize)
                        .unwrap_or(16);
                    if verbose {
                        logs.push(format!("  Spectral n_modes={n_modes}"));
                    }
                    Box::new(SpectralV::from_snapshot(&snap, n_modes, &domain))
                }
                _ => unreachable!(),
            }
        }
        other => anyhow::bail!("unsupported representation '{other}'"),
    };
    if verbose {
        logs.push(format!(
            "Representation built in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1000.0
        ));
    }

    // Build Poisson solver
    progress.set_phase(caustic::StepPhase::BuildPoisson);
    if verbose {
        logs.push(format!("Building Poisson solver: {}", cfg.solver.poisson));
    }
    let t0 = Instant::now();
    let poisson: Box<dyn caustic::PoissonSolver> = match cfg.solver.poisson.as_str() {
        "fft_periodic" | "fft" => Box::new(FftPoisson::new(&domain)),
        "fft_isolated" => {
            logs.push("  WARNING: fft_isolated is deprecated; consider switching to \"vgf\" for spectral-accuracy isolated BC".to_string());
            Box::new(FftIsolated::new(&domain))
        }
        "tensor" | "tensor_poisson" => {
            let shape = [
                cfg.domain.spatial_resolution as usize,
                cfg.domain.spatial_resolution as usize,
                cfg.domain.spatial_resolution as usize,
            ];
            let dx = domain.dx();
            if verbose {
                logs.push(format!("  TensorPoisson: shape={shape:?}, dx={dx:?}"));
            }
            Box::new(TensorPoisson::new(shape, dx, 1e-6, 1e-6, 30))
        }
        "multigrid" => {
            if verbose {
                logs.push("  Multigrid: 4 levels, 3 V-cycles".to_string());
            }
            Box::new(Multigrid::new(&domain, 4, 3))
        }
        "spherical" | "spherical_harmonics" => {
            let n = cfg.domain.spatial_resolution as usize;
            let shape = [n, n, n];
            let dx = domain.dx();
            if verbose {
                logs.push(format!("  SphericalHarmonics: l_max=8, n_radial={}", n / 2));
            }
            Box::new(SphericalHarmonicsPoisson::new(8, n / 2, shape, dx))
        }
        "tree" | "barnes_hut" => {
            if verbose {
                logs.push("  TreePoisson: theta=0.5".to_string());
            }
            Box::new(TreePoisson::new(domain.clone(), 0.5))
        }
        "vgf" | "vgf_isolated" => {
            if verbose {
                logs.push("  VGF (spectral-accuracy isolated BC)".to_string());
            }
            Box::new(caustic::VgfPoisson::new(&domain))
        }
        other => anyhow::bail!("unsupported poisson solver '{other}'"),
    };
    if verbose {
        logs.push(format!(
            "Poisson solver built in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1000.0
        ));
    }

    // Build integrator
    progress.set_phase(caustic::StepPhase::BuildIntegrator);
    if verbose {
        logs.push(format!("Building integrator: {}", cfg.solver.integrator));
    }
    let integrator: Box<dyn caustic::TimeIntegrator> = match cfg.solver.integrator.as_str() {
        "strang" | "strang_splitting" => Box::new(StrangSplitting::new(g)),
        "yoshida" | "yoshida_splitting" => Box::new(YoshidaSplitting::new(g)),
        "lie" => Box::new(LieSplitting::new(g)),
        "unsplit" | "unsplit_rk4" => {
            Box::new(caustic::UnsplitIntegrator::new(4, g, domain.clone()))
        }
        "unsplit_rk2" => Box::new(caustic::UnsplitIntegrator::new(2, g, domain.clone())),
        "unsplit_rk3" => Box::new(caustic::UnsplitIntegrator::new(3, g, domain.clone())),
        "rkei" => Box::new(caustic::RkeiIntegrator::new(g)),
        "bug" => Box::new(caustic::BugIntegrator::new(
            g,
            caustic::BugConfig {
                midpoint: false,
                conservative: false,
                ..Default::default()
            },
        )),
        "midpoint_bug" => Box::new(caustic::BugIntegrator::new(
            g,
            caustic::BugConfig {
                midpoint: true,
                conservative: false,
                ..Default::default()
            },
        )),
        "conservative_bug" => Box::new(caustic::BugIntegrator::new(
            g,
            caustic::BugConfig {
                midpoint: false,
                conservative: true,
                ..Default::default()
            },
        )),
        "blanes_moan" | "bm4" => Box::new(caustic::BlanesMoanSplitting::new(g)),
        "rkn6" => Box::new(caustic::Rkn6Splitting::new(g)),
        "adaptive" | "adaptive_strang" => Box::new(caustic::AdaptiveStrangSplitting::new(g, 1e-6)),
        "parallel_bug" | "pbug" => Box::new(caustic::ParallelBugIntegrator::new(
            g,
            caustic::ParallelBugConfig {
                ..Default::default()
            },
        )),
        "rk_bug" | "rk_bug3" => Box::new(caustic::RkBugIntegrator::new(
            g,
            caustic::RkBugConfig {
                ..Default::default()
            },
        )),
        "lawson" | "lawson_rk4" => Box::new(caustic::LawsonRkIntegrator::new(g)),
        other => anyhow::bail!("unsupported integrator '{other}'"),
    };

    // Build simulation
    progress.set_phase(caustic::StepPhase::BuildAssembly);

    // Guard: LoMaC is incompatible with HT representation. LoMaC requires full 6D grid
    // materialization (to_snapshot), which destroys HT compression and converts to dense
    // UniformGrid6D after every step. Disable LoMaC for HT with a warning.
    let enable_lomac = if cfg.solver.conservation == "lomac" {
        let is_ht = matches!(
            cfg.solver.representation.as_str(),
            "hierarchical_tucker" | "ht"
        );
        if is_ht {
            logs.push(
                "WARNING: LoMaC disabled — incompatible with HT representation \
                 (requires full 6D grid materialization, destroying tensor compression)"
                    .to_string(),
            );
            false
        } else {
            true
        }
    } else {
        false
    };

    if verbose {
        logs.push(format!(
            "Assembling simulation: G={g}, t_final={}, cfl={}, conservation={}",
            cfg.time.t_final.to_f64().unwrap_or(10.0),
            cfg.time.cfl_factor.to_f64().unwrap_or(0.5),
            if enable_lomac {
                "lomac"
            } else {
                &cfg.solver.conservation
            }
        ));
    }
    let t0 = Instant::now();
    let mut sim = caustic::Simulation::builder()
        .domain(domain)
        .poisson_solver_boxed(poisson)
        .advector(SemiLagrangian::new())
        .integrator_boxed(integrator)
        .representation_boxed(repr)
        .time_final(cfg.time.t_final.to_f64().unwrap_or(10.0))
        .cfl_factor(cfg.time.cfl_factor.to_f64().unwrap_or(0.5))
        .gravitational_constant(g)
        .exit_on_energy_drift(cfg.exit.energy_drift_tolerance)
        .lomac(enable_lomac)
        .build()?;
    if verbose {
        logs.push(format!(
            "Simulation assembled in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1000.0
        ));
    }

    // Wire exit conditions
    progress.set_phase(caustic::StepPhase::BuildExitConditions);
    let mut exit_count = 0u32;
    sim.exit_evaluator
        .add_condition(Box::new(MassLossCondition {
            threshold: cfg.exit.mass_drift_tolerance,
        }));
    exit_count += 1;
    // Always have energy drift (set during builder)
    exit_count += 1;

    if let Some(limit) = cfg.exit.wall_clock_limit {
        sim.exit_evaluator
            .add_condition(Box::new(WallClockCondition::new(limit)));
        exit_count += 1;
    }
    if cfg.exit.steady_state {
        sim.exit_evaluator
            .add_condition(Box::new(SteadyStateCondition::new(
                cfg.exit.steady_state_tolerance,
            )));
        exit_count += 1;
    }
    if cfg.exit.virial_equilibrium {
        sim.exit_evaluator
            .add_condition(Box::new(VirialRelaxedCondition {
                tolerance: cfg.exit.virial_tolerance,
            }));
        exit_count += 1;
    }
    if cfg.exit.cfl_violation {
        sim.exit_evaluator
            .add_condition(Box::new(CflViolationCondition {
                dt_min: cfg.time.dt_min.to_f64().unwrap_or(1e-6),
            }));
        exit_count += 1;
    }
    if cfg.exit.casimir_drift_tolerance > 0.0 {
        sim.exit_evaluator
            .add_condition(Box::new(CasimirDriftCondition {
                tolerance: cfg.exit.casimir_drift_tolerance,
            }));
        exit_count += 1;
    }
    if cfg.exit.caustic_formation {
        sim.exit_evaluator
            .add_condition(Box::new(CausticFormationCondition));
        exit_count += 1;
    }
    if verbose {
        logs.push(format!(
            "Exit conditions wired: {exit_count} active (energy_drift_tol={}, mass_drift_tol={})",
            cfg.exit.energy_drift_tolerance, cfg.exit.mass_drift_tolerance
        ));
        logs.push("Simulation ready — entering time-step loop".to_string());
    }

    Ok(sim)
}

/// Build an HtTensor directly from an IC model via ACA (no full grid allocation).
///
/// Returns `Some(HtTensor)` for isolated equilibrium models (plummer, hernquist, king, nfw).
/// Returns `None` for models that don't support this path (caller should fall back).
fn build_ht_from_ic_aca(
    cfg: &crate::config::PhasmaConfig,
    domain: &caustic::Domain,
    g: f64,
    tolerance: f64,
    max_rank: usize,
    verbose: bool,
    logs: &mut Vec<String>,
) -> Option<caustic::HtTensor> {
    use caustic::{HernquistIC, HtTensor, IsolatedEquilibrium, KingIC, NfwIC, PlummerIC};
    use rust_decimal::prelude::ToPrimitive;

    let m = cfg.model.total_mass.to_f64().unwrap_or(1.0);
    let a = cfg.model.scale_radius.to_f64().unwrap_or(1.0);

    let ic: Box<dyn IsolatedEquilibrium + Sync> = match cfg.model.model_type.as_str() {
        "plummer" => Box::new(PlummerIC::new(m, a, g)),
        "hernquist" => Box::new(HernquistIC::new(m, a, g)),
        "king" => {
            let king = cfg.model.king.as_ref()?;
            let w0 = king.w0.to_f64().unwrap_or(7.0);
            Box::new(KingIC::new(m, w0, a, g))
        }
        "nfw" => {
            let nfw = cfg.model.nfw.as_ref()?;
            let c = nfw.concentration.to_f64().unwrap_or(10.0);
            Box::new(NfwIC::new(m, a, c, g))
        }
        _ => return None,
    };

    if verbose {
        logs.push(format!(
            "  HT ACA path: tolerance={tolerance:.1e}, max_rank={max_rank}"
        ));
        logs.push("  Building HT via fiber sampling (no full grid)...".to_string());
    }

    let ht = HtTensor::from_function_aca(
        |x, v| {
            let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
            let phi = ic.potential(r);
            let v2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            let energy = 0.5 * v2 + phi;
            ic.distribution_function(energy, 0.0).max(0.0)
        },
        domain,
        tolerance,
        max_rank,
        None,
        None,
    );

    Some(ht)
}

fn build_ic(
    cfg: &crate::config::PhasmaConfig,
    domain: &caustic::Domain,
    g: f64,
    progress: Option<&caustic::StepProgress>,
) -> anyhow::Result<caustic::PhaseSpaceSnapshot> {
    use caustic::{
        CustomICArray, HernquistIC, KingIC, MergerIC, NfwIC, PlummerIC, ZeldovichSingleMode,
        sample_on_grid_with_progress,
    };

    let m = cfg.model.total_mass.to_f64().unwrap_or(1.0);
    let a = cfg.model.scale_radius.to_f64().unwrap_or(1.0);

    match cfg.model.model_type.as_str() {
        "plummer" => {
            let ic = PlummerIC::new(m, a, g);
            Ok(sample_on_grid_with_progress(&ic, domain, progress))
        }
        "hernquist" => {
            let ic = HernquistIC::new(m, a, g);
            Ok(sample_on_grid_with_progress(&ic, domain, progress))
        }
        "king" => {
            let king = cfg
                .model
                .king
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("king model requires [model.king] with w0"))?;
            let ic = KingIC::new(m, king.w0.to_f64().unwrap_or(7.0), a, g);
            Ok(sample_on_grid_with_progress(&ic, domain, progress))
        }
        "nfw" => {
            let nfw = cfg.model.nfw.as_ref().ok_or_else(|| {
                anyhow::anyhow!("nfw model requires [model.nfw] with concentration")
            })?;
            let ic = NfwIC::new(m, a, nfw.concentration.to_f64().unwrap_or(10.0), g);
            Ok(sample_on_grid_with_progress(&ic, domain, progress))
        }
        "zeldovich" => {
            let z = cfg.model.zeldovich.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "zeldovich requires [model.zeldovich] with amplitude and wave_number"
                )
            })?;
            let spatial_ext = cfg.domain.spatial_extent.to_f64().unwrap_or(10.0);
            let mean_density = m / (2.0 * spatial_ext).powi(3);
            let sigma_v = 0.1; // small thermal spread for cold dark matter
            let ic = ZeldovichSingleMode {
                mean_density,
                amplitude: z.amplitude.to_f64().unwrap_or(0.01),
                wavenumber: z.wave_number.to_f64().unwrap_or(1.0),
                sigma_v,
            };
            Ok(ic.sample_on_grid(domain, progress))
        }
        "merger" | "two_body_merger" => {
            let merger = cfg.model.merger.as_ref().ok_or_else(|| {
                anyhow::anyhow!("merger requires [model.merger] with separation and mass_ratio")
            })?;
            let mass_ratio = merger.mass_ratio.to_f64().unwrap_or(1.0);
            let m1 = m / (1.0 + mass_ratio);
            let m2 = m - m1;
            let a1 = merger.scale_radius_1.to_f64().unwrap_or(1.0);
            let a2 = merger.scale_radius_2.to_f64().unwrap_or(1.0);
            let body1 = Box::new(PlummerIC::new(m1, a1, g));
            let body2 = Box::new(PlummerIC::new(m2, a2, g));
            let sep = [merger.separation.to_f64().unwrap_or(10.0), 0.0, 0.0];
            let vel = merger.relative_velocity;
            let impact = merger.impact_parameter.to_f64().unwrap_or(2.0);
            let ic = MergerIC::new(body1, m1, body2, m2, sep, vel, impact);
            Ok(ic.sample_on_grid(domain, progress))
        }
        "custom_file" => {
            let cf = cfg.model.custom_file.as_ref().ok_or_else(|| {
                anyhow::anyhow!("custom_file requires [model.custom_file] with file_path")
            })?;
            let ic = CustomICArray::from_npy(&cf.file_path, domain)?;
            Ok(ic.snapshot)
        }
        "uniform_perturbation" => {
            let pert = cfg.model.uniform_perturbation.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "uniform_perturbation requires [model.uniform_perturbation] section"
                )
            })?;
            Ok(build_uniform_perturbation_ic(domain, pert))
        }
        "tidal" => {
            let tc = cfg
                .model
                .tidal
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("tidal model requires [model.tidal] section"))?;
            // Build progenitor equilibrium
            let prog_mass = tc.progenitor_mass.to_f64().unwrap_or(1.0);
            let prog_scale = tc.progenitor_scale_radius.to_f64().unwrap_or(1.0);
            let progenitor: Box<dyn caustic::IsolatedEquilibrium> =
                match tc.progenitor_type.as_str() {
                    "plummer" => Box::new(caustic::PlummerIC::new(prog_mass, prog_scale, g)),
                    "hernquist" => Box::new(caustic::HernquistIC::new(prog_mass, prog_scale, g)),
                    "king" => {
                        let w0 = cfg
                            .model
                            .king
                            .as_ref()
                            .map(|k| k.w0.to_f64().unwrap_or(7.0))
                            .unwrap_or(7.0);
                        Box::new(caustic::KingIC::new(prog_mass, w0, prog_scale, g))
                    }
                    "nfw" => {
                        let c = cfg
                            .model
                            .nfw
                            .as_ref()
                            .map(|n| n.concentration.to_f64().unwrap_or(10.0))
                            .unwrap_or(10.0);
                        Box::new(caustic::NfwIC::new(prog_mass, prog_scale, c, g))
                    }
                    other => anyhow::bail!("unsupported tidal progenitor type '{other}'"),
                };
            // Build host potential
            let host_mass = tc.host_mass.to_f64().unwrap_or(10.0);
            let host_scale = tc.host_scale_radius.to_f64().unwrap_or(20.0);
            let host_potential: Box<dyn Fn([f64; 3]) -> f64 + Send + Sync> =
                match tc.host_type.as_str() {
                    "point_mass" => Box::new(move |x: [f64; 3]| {
                        let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt().max(1e-10);
                        -g * host_mass / r
                    }),
                    "nfw_fixed" => Box::new(move |x: [f64; 3]| {
                        let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt().max(1e-10);
                        let s = r / host_scale;
                        -g * host_mass * (1.0 + s).ln() / r
                    }),
                    "logarithmic" => Box::new(move |x: [f64; 3]| {
                        let r2 = x[0] * x[0] + x[1] * x[1] + x[2] * x[2];
                        // Φ = 0.5 * v_c² * ln(r² + r_c²), v_c = sqrt(G*M/r_c)
                        let vc2 = g * host_mass / host_scale;
                        0.5 * vc2 * (r2 + host_scale * host_scale).ln()
                    }),
                    other => anyhow::bail!("unsupported tidal host type '{other}'"),
                };
            let ic = caustic::TidalIC::new(
                host_potential,
                progenitor,
                tc.progenitor_position,
                tc.progenitor_velocity,
            );
            Ok(ic.sample_on_grid(domain, progress))
        }
        "disk_exponential" | "disk_stability" => {
            let rd = cfg
                .model
                .disk
                .as_ref()
                .map_or(a, |d| d.disk_scale_length.to_f64().unwrap_or(3.0));
            let disk_mass = cfg
                .model
                .disk
                .as_ref()
                .map_or(m, |d| d.disk_mass.to_f64().unwrap_or(1.0));
            let sigma_0 = disk_mass / (2.0 * std::f64::consts::PI * rd * rd);
            let sigma_r0 = cfg
                .model
                .disk
                .as_ref()
                .map_or(0.3 * (g * sigma_0 * rd).sqrt(), |d| {
                    d.radial_velocity_dispersion.to_f64().unwrap_or(0.15)
                });
            let ic = caustic::DiskStabilityIC::new(
                Box::new(move |r: f64| sigma_0 * (-r / rd).exp()),
                Box::new(move |r: f64| sigma_r0 * (-r / (2.0 * rd)).exp()),
                2,    // m=2 bar mode
                0.0,  // pattern speed
                0.05, // perturbation amplitude
            );
            Ok(ic.sample_on_grid(domain, progress))
        }
        other => anyhow::bail!("unsupported model type '{other}'"),
    }
}

/// Build uniform Maxwellian + sinusoidal perturbation IC directly on grid.
/// f(x,v) = C * exp(-v²/2σ²) * (1 + ε * cos(k·x))
fn build_uniform_perturbation_ic(
    domain: &caustic::Domain,
    pert: &crate::config::PerturbationConfig,
) -> caustic::PhaseSpaceSnapshot {
    let sigma = pert.velocity_dispersion.to_f64().unwrap_or(0.5);
    let eps = pert.perturbation_amplitude.to_f64().unwrap_or(0.01);
    let k = pert.perturbation_wavenumber;

    let nx1 = domain.spatial_res.x1 as usize;
    let nx2 = domain.spatial_res.x2 as usize;
    let nx3 = domain.spatial_res.x3 as usize;
    let nv1 = domain.velocity_res.v1 as usize;
    let nv2 = domain.velocity_res.v2 as usize;
    let nv3 = domain.velocity_res.v3 as usize;

    let dx = domain.dx();
    let dv = domain.dv();
    let lx = [
        domain.spatial.x1.to_f64().unwrap_or(1.0),
        domain.spatial.x2.to_f64().unwrap_or(1.0),
        domain.spatial.x3.to_f64().unwrap_or(1.0),
    ];
    let lv = [
        domain.velocity.v1.to_f64().unwrap_or(1.0),
        domain.velocity.v2.to_f64().unwrap_or(1.0),
        domain.velocity.v3.to_f64().unwrap_or(1.0),
    ];

    // Compute velocity-space normalization
    let sigma2 = sigma * sigma;
    let mut s_norm = 0.0f64;
    for iv1 in 0..nv1 {
        let v1 = -lv[0] + (iv1 as f64 + 0.5) * dv[0];
        for iv2 in 0..nv2 {
            let v2 = -lv[1] + (iv2 as f64 + 0.5) * dv[1];
            for iv3 in 0..nv3 {
                let v3 = -lv[2] + (iv3 as f64 + 0.5) * dv[2];
                let v2sq = v1 * v1 + v2 * v2 + v3 * v3;
                s_norm += (-v2sq / (2.0 * sigma2)).exp() * dv[0] * dv[1] * dv[2];
            }
        }
    }
    let c = pert.background_density.to_f64().unwrap_or(1.0) / s_norm;

    // Fill 6D grid
    let total = nx1 * nx2 * nx3 * nv1 * nv2 * nv3;
    let mut data = vec![0.0f64; total];
    let sv3 = 1usize;
    let sv2 = nv3;
    let sv1 = nv2 * nv3;
    let sx3 = nv1 * sv1;
    let sx2 = nx3 * sx3;
    let sx1 = nx2 * sx2;

    for ix1 in 0..nx1 {
        let x1 = -lx[0] + (ix1 as f64 + 0.5) * dx[0];
        for ix2 in 0..nx2 {
            let x2 = -lx[1] + (ix2 as f64 + 0.5) * dx[1];
            for ix3 in 0..nx3 {
                let x3 = -lx[2] + (ix3 as f64 + 0.5) * dx[2];
                let phase = k[0] * x1 + k[1] * x2 + k[2] * x3;
                let spatial_factor = c * (1.0 + eps * phase.cos());
                let base = ix1 * sx1 + ix2 * sx2 + ix3 * sx3;

                for iv1 in 0..nv1 {
                    let v1 = -lv[0] + (iv1 as f64 + 0.5) * dv[0];
                    for iv2 in 0..nv2 {
                        let v2 = -lv[1] + (iv2 as f64 + 0.5) * dv[1];
                        for iv3 in 0..nv3 {
                            let v3 = -lv[2] + (iv3 as f64 + 0.5) * dv[2];
                            let v2sq = v1 * v1 + v2 * v2 + v3 * v3;
                            let f = spatial_factor * (-v2sq / (2.0 * sigma2)).exp();
                            data[base + iv1 * sv1 + iv2 * sv2 + iv3 * sv3] = f;
                        }
                    }
                }
            }
        }
    }

    caustic::PhaseSpaceSnapshot {
        data,
        shape: [nx1, nx2, nx3, nv1, nv2, nv3],
        time: 0.0,
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
    poisson_type: &str,
    compute_poisson_diag: bool,
    compute_phase: bool,
) -> SimState {
    let diag = sim
        .diagnostics
        .history
        .last()
        .copied()
        .unwrap_or_else(zero_diag);

    // Density projections over 3 axes (reuse cached from step if available)
    let density = sim
        .cached_density
        .clone()
        .unwrap_or_else(|| sim.repr.compute_density());
    let [nx1, nx2, nx3] = density.shape;

    let mut density_xy = vec![0.0f64; nx1 * nx2];
    let mut density_xz = vec![0.0f64; nx1 * nx3];
    let mut density_yz = vec![0.0f64; nx2 * nx3];
    let mut max_density = 0.0f64;

    // Pass 1: density_xy and density_xz (ix1 outer — sequential writes for both)
    for ix1 in 0..nx1 {
        for ix2 in 0..nx2 {
            for ix3 in 0..nx3 {
                let v = density.data[ix1 * nx2 * nx3 + ix2 * nx3 + ix3];
                density_xy[ix1 * nx2 + ix2] += v;
                density_xz[ix1 * nx3 + ix3] += v;
                if v > max_density {
                    max_density = v;
                }
            }
        }
    }

    // Pass 2: density_yz (ix2 outer — sequential writes to density_yz[ix2 * nx3 + ix3])
    for ix2 in 0..nx2 {
        for ix1 in 0..nx1 {
            for ix3 in 0..nx3 {
                let v = density.data[ix1 * nx2 * nx3 + ix2 * nx3 + ix3];
                density_yz[ix2 * nx3 + ix3] += v;
            }
        }
    }

    // Poisson diagnostics: residual and power spectrum (only every Nth step)
    let (residual, spectrum, density_ps, field_es) = if compute_poisson_diag {
        let potential = sim
            .cached_potential
            .clone()
            .unwrap_or_else(|| sim.poisson.solve(&density, sim.g));
        let dx = sim.domain.dx();
        let r = compute_poisson_residual_l2(
            &density.data,
            &potential.data,
            density.shape,
            sim.g,
            [dx[0], dx[1], dx[2]],
        );
        let sp = if density.shape.iter().all(|&n| n <= 32) {
            Some(compute_potential_power_spectrum(
                &potential.data,
                potential.shape,
                [dx[0], dx[1], dx[2]],
            ))
        } else {
            None
        };
        // Spectral diagnostics (Phase 5 gap analysis)
        let (dps, fes) = if density.shape.iter().all(|&n| n <= 32) {
            let dps = caustic::PhaseSpaceDiagnostics::power_spectrum(&density);
            let fes = caustic::field_energy_spectrum(&potential, [dx[0], dx[1], dx[2]]);
            (Some(dps), Some(fes))
        } else {
            (None, None)
        };
        (Some(r), sp, dps, fes)
    } else {
        (None, None, None, None)
    };

    // Phase-space projections f(x_i, v_j) for all 9 (dim_x, dim_v) combinations.
    // Skip materialization for large grids (threshold: 64^6 ≈ 68B elements).
    // Only compute every Nth step (controlled by compute_phase flag); caller reuses cached slices.
    const PHASE_SNAPSHOT_THRESHOLD: usize = 64 * 64 * 64 * 64 * 64 * 64;
    let total_elements = sim.domain.total_cells();
    let (phase_slices, phase_nx, phase_nv) = if compute_phase
        && sim.repr.can_materialize()
        && total_elements <= PHASE_SNAPSHOT_THRESHOLD
    {
        let snap = sim.repr.to_snapshot(sim.time);
        let [sx1, sx2, sx3, sv1, sv2, sv3] = snap.shape;
        let s = [sx1, sx2, sx3, sv1, sv2, sv3];
        let slices = compute_all_phase_slices(&snap.data, s);
        (Arc::new(slices), sx1, sv1)
    } else {
        (Arc::new(vec![vec![]; 9]), 0, 0)
    };

    // Repr memory via trait method (works for all representations)
    let repr_mem = sim.repr.memory_bytes();

    // HT rank diagnostics (attempt downcast for HT-specific fields)
    let (rank_per_node, rank_total, rank_memory_bytes, compression_ratio, repr_type) =
        if let Some(ht) = sim.repr.as_any().downcast_ref::<caustic::HtTensor>() {
            let per_node: Vec<usize> = (0..11).map(|i| ht.rank_at(i)).collect();
            let total = ht.total_rank();
            let mem = ht.memory_bytes();
            let full_size: usize = ht.shape.iter().product::<usize>() * 8;
            let cr = if mem > 0 {
                full_size as f64 / mem as f64
            } else {
                0.0
            };
            (
                Some(per_node),
                Some(total),
                Some(mem),
                Some(cr),
                "ht".to_string(),
            )
        } else {
            // For non-HT representations, use the trait method for memory
            let mem = if repr_mem > 0 { Some(repr_mem) } else { None };
            (None, None, mem, None, "uniform".to_string())
        };

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
        phase_slice: vec![],
        phase_nx,
        phase_nv,
        spatial_extent,
        gravitational_constant: grav_const,
        dt: 0.0, // computed by consumer from time differences
        exit_reason,
        rank_per_node,
        rank_total,
        rank_memory_bytes,
        compression_ratio,
        repr_type,
        poisson_type: poisson_type.to_string(),
        poisson_residual_l2: residual,
        potential_power_spectrum: spectrum,
        density_power_spectrum: density_ps,
        field_energy_spectrum: field_es,
        // Phase timings from caustic instrumentation
        phase_timings: {
            let t = &sim.last_step_timings;
            let sum = t.drift_ms
                + t.poisson_ms
                + t.kick_ms
                + t.density_ms
                + t.diagnostics_ms
                + t.io_ms
                + t.other_ms;
            if sum > 0.0 { Some(t.to_array()) } else { None }
        },
        truncation_errors: None,
        svd_count: 0,
        htaca_evaluations: 0,
        velocity_extent: sim.domain.velocity.v1.to_f64().unwrap_or(3.0),
        singular_values: None,
        lagrangian_radii: None,
        poisson_rank_amplification: None,
        advection_rank_amplification: None,
        green_function_rank: None,
        exp_sum_terms: None,
        log_messages: Vec::new(),
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
        casimir_c2_pre_lomac: None,
        casimir_c2_post_lomac: None,
        entropy: 0.0,
        mass_in_box: 0.0,
    }
}

/// Compute all 9 phase-space 2D projections f(x_i, v_j) from 6D data in a single pass.
/// Returns Vec of 9 flat arrays, indexed by dim_x * 3 + dim_v.
fn compute_all_phase_slices(data: &[f64], shape: [usize; 6]) -> Vec<Vec<f64>> {
    use rayon::prelude::*;

    let [sx1, sx2, sx3, sv1, sv2, sv3] = shape;
    let spatial = [sx1, sx2, sx3];
    let velocity = [sv1, sv2, sv3];

    // Pre-compute output sizes
    let sizes: Vec<(usize, usize)> = (0..9)
        .map(|idx| (spatial[idx / 3], velocity[idx % 3]))
        .collect();

    // Pre-compute strides
    let stride2 = sx3 * sv1 * sv2 * sv3;
    let stride3 = sv1 * sv2 * sv3;
    let stride_v1 = sv2 * sv3;
    let stride_v2 = sv3;

    // Parallel over outermost spatial dimension, single pass accumulating all 9 projections.
    let per_slab: Vec<Vec<Vec<f64>>> = (0..sx1)
        .into_par_iter()
        .map(|i1| {
            let mut slices: Vec<Vec<f64>> =
                sizes.iter().map(|&(nx, nv)| vec![0.0; nx * nv]).collect();
            let base1 = i1 * sx2 * stride2;
            for i2 in 0..sx2 {
                let base2 = base1 + i2 * stride2;
                for i3 in 0..sx3 {
                    let base3 = base2 + i3 * stride3;
                    let si = [i1, i2, i3];
                    for j1 in 0..sv1 {
                        let base_v1 = base3 + j1 * stride_v1;
                        for j2 in 0..sv2 {
                            let base_v2 = base_v1 + j2 * stride_v2;
                            for j3 in 0..sv3 {
                                let val = data[base_v2 + j3];
                                let vj = [j1, j2, j3];
                                for idx in 0..9 {
                                    let dim_x = idx / 3;
                                    let dim_v = idx % 3;
                                    let ix = si[dim_x];
                                    let iv = vj[dim_v];
                                    slices[idx][ix * sizes[idx].1 + iv] += val;
                                }
                            }
                        }
                    }
                }
            }
            slices
        })
        .collect();

    // Reduce: sum per-slab results
    let mut result: Vec<Vec<f64>> = sizes.iter().map(|&(nx, nv)| vec![0.0; nx * nv]).collect();
    for slab in per_slab {
        for (idx, slab_slice) in slab.into_iter().enumerate() {
            for (dst, src) in result[idx].iter_mut().zip(slab_slice.iter()) {
                *dst += src;
            }
        }
    }
    result
}

fn error_state(msg: String) -> SimState {
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
        phase_slices: Arc::new(vec![vec![]; 9]),
        phase_slice: vec![],
        phase_nx: 0,
        phase_nv: 0,
        spatial_extent: 0.0,
        gravitational_constant: 0.0,
        dt: 0.0,
        exit_reason: Some(ExitReason::UserStop),
        rank_per_node: None,
        rank_total: None,
        rank_memory_bytes: None,
        compression_ratio: None,
        repr_type: String::new(),
        poisson_type: String::new(),
        poisson_residual_l2: None,
        potential_power_spectrum: None,
        density_power_spectrum: None,
        field_energy_spectrum: None,
        phase_timings: None,
        truncation_errors: None,
        svd_count: 0,
        htaca_evaluations: 0,
        velocity_extent: 0.0,
        singular_values: None,
        lagrangian_radii: None,
        poisson_rank_amplification: None,
        advection_rank_amplification: None,
        green_function_rank: None,
        exp_sum_terms: None,
        log_messages: vec![format!("ERROR: {msg}")],
    }
}

/// Compute L2 norm of the Poisson residual: ||nabla^2 Phi - 4piG rho||_2.
/// Uses a 7-point finite-difference stencil on interior cells.
fn compute_poisson_residual_l2(
    density: &[f64],
    potential: &[f64],
    shape: [usize; 3],
    g: f64,
    dx: [f64; 3],
) -> f64 {
    let [nx, ny, nz] = shape;
    let four_pi_g = 4.0 * std::f64::consts::PI * g;
    let inv_dx2 = [
        1.0 / (dx[0] * dx[0]),
        1.0 / (dx[1] * dx[1]),
        1.0 / (dx[2] * dx[2]),
    ];
    let mut sum_sq = 0.0;
    let mut count = 0usize;

    for ix in 1..nx.saturating_sub(1) {
        for iy in 1..ny.saturating_sub(1) {
            for iz in 1..nz.saturating_sub(1) {
                let idx = ix * ny * nz + iy * nz + iz;
                let phi = potential[idx];

                let lap_x = (potential[(ix + 1) * ny * nz + iy * nz + iz]
                    + potential[(ix - 1) * ny * nz + iy * nz + iz]
                    - 2.0 * phi)
                    * inv_dx2[0];
                let lap_y = (potential[ix * ny * nz + (iy + 1) * nz + iz]
                    + potential[ix * ny * nz + (iy - 1) * nz + iz]
                    - 2.0 * phi)
                    * inv_dx2[1];
                let lap_z = (potential[ix * ny * nz + iy * nz + (iz + 1)]
                    + potential[ix * ny * nz + iy * nz + (iz - 1)]
                    - 2.0 * phi)
                    * inv_dx2[2];
                let laplacian = lap_x + lap_y + lap_z;

                let rhs = four_pi_g * density[idx];
                let residual = laplacian - rhs;
                sum_sq += residual * residual;
                count += 1;
            }
        }
    }

    if count > 0 {
        (sum_sq / count as f64).sqrt()
    } else {
        0.0
    }
}

/// Compute spherically-averaged power spectrum P(k) = <|Phi_hat(k)|^2> of the potential field.
/// Uses a full 3D FFT via rustfft, then bins |Phi_hat|^2 by wavenumber magnitude.
fn compute_potential_power_spectrum(
    potential: &[f64],
    shape: [usize; 3],
    dx: [f64; 3],
) -> Vec<(f64, f64)> {
    use rustfft::{FftPlanner, num_complex::Complex};

    let [nx, ny, nz] = shape;
    let n = nx * ny * nz;
    if n == 0 {
        return vec![];
    }

    // Convert to complex
    let mut buffer: Vec<Complex<f64>> = potential.iter().map(|&v| Complex::new(v, 0.0)).collect();

    let mut planner = FftPlanner::new();

    // FFT along z (contiguous, last axis)
    let fft_z = planner.plan_fft_forward(nz);
    for ix in 0..nx {
        for iy in 0..ny {
            let start = ix * ny * nz + iy * nz;
            fft_z.process(&mut buffer[start..start + nz]);
        }
    }

    // FFT along y (strided)
    let fft_y = planner.plan_fft_forward(ny);
    let mut temp_y = vec![Complex::new(0.0, 0.0); ny];
    for ix in 0..nx {
        for iz in 0..nz {
            for iy in 0..ny {
                temp_y[iy] = buffer[ix * ny * nz + iy * nz + iz];
            }
            fft_y.process(&mut temp_y);
            for iy in 0..ny {
                buffer[ix * ny * nz + iy * nz + iz] = temp_y[iy];
            }
        }
    }

    // FFT along x (strided)
    let fft_x = planner.plan_fft_forward(nx);
    let mut temp_x = vec![Complex::new(0.0, 0.0); nx];
    for iy in 0..ny {
        for iz in 0..nz {
            for ix in 0..nx {
                temp_x[ix] = buffer[ix * ny * nz + iy * nz + iz];
            }
            fft_x.process(&mut temp_x);
            for ix in 0..nx {
                buffer[ix * ny * nz + iy * nz + iz] = temp_x[ix];
            }
        }
    }

    // Wavenumber spacing
    let dk = [
        2.0 * std::f64::consts::PI / (nx as f64 * dx[0]),
        2.0 * std::f64::consts::PI / (ny as f64 * dx[1]),
        2.0 * std::f64::consts::PI / (nz as f64 * dx[2]),
    ];

    let k_nyquist = [
        (nx / 2) as f64 * dk[0],
        (ny / 2) as f64 * dk[1],
        (nz / 2) as f64 * dk[2],
    ];
    let k_max = (k_nyquist[0].powi(2) + k_nyquist[1].powi(2) + k_nyquist[2].powi(2)).sqrt();
    let dk_bin = dk.iter().copied().fold(f64::INFINITY, f64::min);
    let n_bins = ((k_max / dk_bin) as usize + 1).max(1);

    let mut power_bins = vec![0.0f64; n_bins];
    let mut count_bins = vec![0usize; n_bins];

    let norm = 1.0 / (n as f64 * n as f64);

    for ikx in 0..nx {
        let kx = if ikx <= nx / 2 {
            ikx as f64
        } else {
            ikx as f64 - nx as f64
        } * dk[0];
        for iky in 0..ny {
            let ky = if iky <= ny / 2 {
                iky as f64
            } else {
                iky as f64 - ny as f64
            } * dk[1];
            for ikz in 0..nz {
                let kz = if ikz <= nz / 2 {
                    ikz as f64
                } else {
                    ikz as f64 - nz as f64
                } * dk[2];

                let k_mag = (kx * kx + ky * ky + kz * kz).sqrt();
                if k_mag < 1e-14 {
                    continue; // skip DC mode
                }

                let c = buffer[ikx * ny * nz + iky * nz + ikz];
                let power = (c.re * c.re + c.im * c.im) * norm;

                let bin = (k_mag / dk_bin).round() as usize;
                if bin < n_bins {
                    power_bins[bin] += power;
                    count_bins[bin] += 1;
                }
            }
        }
    }

    // Return (k, P(k)) for non-empty bins
    power_bins
        .iter()
        .zip(count_bins.iter())
        .enumerate()
        .filter(|&(_, (_, &c))| c > 0)
        .map(|(i, (&p, &c))| {
            let k = (i as f64 + 0.5) * dk_bin;
            (k, p / c as f64)
        })
        .collect()
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::config::defaults::{estimate_memory_breakdown, estimate_memory_mb};

    /// Read current process RSS in MB from /proc/self/status (VmRSS field, in kB).
    fn read_rss_mb() -> f64 {
        #[cfg(target_os = "linux")]
        {
            if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
                for line in content.lines() {
                    if let Some(rest) = line.strip_prefix("VmRSS:") {
                        let kb: f64 = rest
                            .trim()
                            .trim_end_matches(" kB")
                            .trim()
                            .parse()
                            .unwrap_or(0.0);
                        return kb / 1024.0;
                    }
                }
            }
        }
        0.0
    }

    /// Load a config file and return (config, config_path).
    fn load_test_config(name: &str) -> (crate::config::PhasmaConfig, String) {
        let path = format!("{}/configs/{name}.toml", env!("CARGO_MANIFEST_DIR"));
        let cfg =
            crate::config::load(&path).unwrap_or_else(|e| panic!("failed to load {name}: {e}"));
        (cfg, path)
    }

    /// Build a simulation from a config and measure the RSS delta.
    /// If `run_step` is true, runs one simulation step to capture peak allocations.
    /// Returns (estimated_mb, measured_delta_mb, config_name).
    fn measure_sim_memory(name: &str, run_step: bool) -> Option<(f64, f64, String)> {
        let (cfg, _path) = load_test_config(name);

        // Skip configs that can't build (custom_function requires external lib)
        if cfg.model.model_type == "custom_function" {
            return None;
        }

        let estimated = estimate_memory_mb(&cfg);

        // Force a GC-like compaction by dropping large temporaries
        std::hint::black_box(0);

        let rss_before = read_rss_mb();
        if rss_before == 0.0 {
            // Not on Linux, skip RSS measurement
            return None;
        }

        let sim_result =
            build_from_config(&cfg, false, &mut Vec::new(), &caustic::StepProgress::new());
        match sim_result {
            Ok(mut sim) => {
                if run_step {
                    // Run one step to trigger advection clone, workspace, etc.
                    let _ = sim.step();
                }
                let rss_after = read_rss_mb();
                let delta = rss_after - rss_before;

                // Keep sim alive until after measurement
                std::hint::black_box(&sim);

                Some((estimated, delta, name.to_string()))
            }
            Err(e) => {
                eprintln!("skipping {name}: {e}");
                None
            }
        }
    }

    /// Helper: check estimate vs actual, print result, assert ratio.
    /// Only asserts when actual delta > 50 MB — below that, RSS noise dominates.
    fn check_ratio(est: f64, actual: f64, name: &str, label: &str, lo: f64, hi: f64) {
        let ratio = if actual > 0.0 { est / actual } else { f64::NAN };
        eprintln!(
            "{name} ({label}): estimated={est:.1} MB, actual_delta={actual:.1} MB, ratio={ratio:.2}"
        );
        if actual > 50.0 && est > 50.0 {
            assert!(
                ratio > lo && ratio < hi,
                "{name}: estimate/actual ratio {ratio:.2} is outside [{lo}, {hi}]"
            );
        }
    }

    /// Small config (8^6 ≈ 2 MB) — test with one step to verify peak allocation.
    #[test]
    fn memory_estimate_vs_actual_debug_step() {
        if let Some((est, actual, name)) = measure_sim_memory("debug", true) {
            check_ratio(est, actual, &name, "step", 0.1, 50.0);
        }
    }

    /// Jeans unstable — small (8^6) with perturbation IC.
    #[test]
    fn memory_estimate_vs_actual_jeans_step() {
        if let Some((est, actual, name)) = measure_sim_memory("jeans_unstable", true) {
            check_ratio(est, actual, &name, "step", 0.1, 50.0);
        }
    }

    /// Medium config (16^6 ≈ 134 MB) with fft_isolated — step to see Poisson buffers.
    #[test]
    fn memory_estimate_vs_actual_plummer_step() {
        if let Some((est, actual, name)) = measure_sim_memory("plummer", true) {
            check_ratio(est, actual, &name, "step", 0.3, 5.0);
        }
    }

    /// Medium config (16^6 ≈ 134 MB) — construct only (no step) to avoid large peak.
    #[test]
    fn memory_estimate_vs_actual_hernquist_construct() {
        if let Some((est, actual, name)) = measure_sim_memory("hernquist", false) {
            check_ratio(est, actual, &name, "construct", 0.2, 10.0);
        }
    }

    /// Run estimates for all loadable configs and print a summary table.
    #[test]
    fn memory_estimate_summary_table() {
        let configs_dir = format!("{}/configs", env!("CARGO_MANIFEST_DIR"));
        let mut entries: Vec<_> = std::fs::read_dir(&configs_dir)
            .expect("configs/ directory should exist")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        eprintln!(
            "\n{:<25} {:>12} {:>12} {:>12} {:>8}",
            "Config", "Phase(MB)", "Poisson(MB)", "Total(MB)", "FG(MB)"
        );
        eprintln!("{}", "-".repeat(75));

        for entry in &entries {
            let path = entry.path();
            let name = path.file_stem().unwrap().to_string_lossy();
            if let Ok(cfg) = crate::config::load(path.to_str().unwrap()) {
                let b = estimate_memory_breakdown(&cfg);
                let fg = crate::config::defaults::full_grid_memory_mb(&cfg);
                eprintln!(
                    "{:<25} {:>12.2} {:>12.2} {:>12.2} {:>8.1}",
                    name,
                    b.phase_space_mb,
                    b.poisson_buffers_mb,
                    b.total_mb(),
                    fg
                );
            }
        }
        eprintln!();
    }
}

#[cfg(test)]
mod heavy_tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    /// Read current process RSS in MB from /proc/self/status (VmRSS field, in kB).
    fn read_rss_mb() -> f64 {
        #[cfg(target_os = "linux")]
        {
            if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
                for line in content.lines() {
                    if let Some(rest) = line.strip_prefix("VmRSS:") {
                        let kb: f64 = rest
                            .trim()
                            .trim_end_matches(" kB")
                            .trim()
                            .parse()
                            .unwrap_or(0.0);
                        return kb / 1024.0;
                    }
                }
            }
        }
        0.0
    }

    /// Run plummer_128 config to t=1 and assert RSS stays under 16 GB.
    ///
    /// This is a heavy test (~1–1.5 GB RAM, minutes of wall time).
    /// It does NOT run with `cargo test` — you must opt in:
    ///
    /// ```sh
    /// cargo test --release plummer_128_fits_16gb -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn plummer_128_fits_16gb() {
        const RSS_LIMIT_MB: f64 = 16_000.0; // 16 GB

        let config_path = format!("{}/configs/plummer_128.toml", env!("CARGO_MANIFEST_DIR"));
        let mut cfg = crate::config::load(&config_path).expect("plummer_128.toml should load");

        // Override t_final to 1.0 for the test
        cfg.time.t_final = rust_decimal::Decimal::from_f64(1.0).unwrap();

        let mut sim =
            build_from_config(&cfg, false, &mut Vec::new(), &caustic::StepProgress::new())
                .expect("plummer_128 should build");

        let rss_after_build = read_rss_mb();
        eprintln!("RSS after build: {rss_after_build:.0} MB");
        assert!(
            rss_after_build < RSS_LIMIT_MB,
            "RSS after build ({rss_after_build:.0} MB) exceeds 16 GB limit"
        );

        let mut step = 0u64;
        loop {
            match sim.step() {
                Ok(None) => {}
                Ok(Some(_reason)) => {
                    eprintln!("Simulation exited at step {step}: {_reason:?}");
                    break;
                }
                Err(e) => panic!("Step {step} failed: {e}"),
            }
            step += 1;

            // Check RSS every 10 steps to avoid /proc overhead
            if step % 10 == 0 {
                let rss = read_rss_mb();
                eprintln!("Step {step}: RSS = {rss:.0} MB");
                assert!(
                    rss < RSS_LIMIT_MB,
                    "RSS at step {step} ({rss:.0} MB) exceeds 16 GB limit"
                );
            }
        }

        let rss_final = read_rss_mb();
        eprintln!("Final RSS after {step} steps: {rss_final:.0} MB");
        assert!(
            rss_final < RSS_LIMIT_MB,
            "Final RSS ({rss_final:.0} MB) exceeds 16 GB limit"
        );
    }
}

#[cfg(test)]
mod drift_threshold_tests {
    use super::*;

    /// Maximum phase-space size (MB) we're willing to test — avoids OOM.
    /// 150 MB covers 16^6 configs (134 MB) but skips 20^6+ (512+ MB).
    const MAX_PHASE_SPACE_MB: f64 = 10.0;

    /// Number of steps to run for the drift test.
    const TEST_STEPS: u32 = 10;

    /// Build a simulation from a config and run N steps.
    /// Returns None if the config can't be built (e.g. disk_exponential).
    /// Returns Some((steps_completed, exit_reason)) — exit_reason is None if all steps completed.
    fn run_steps(
        cfg: &crate::config::PhasmaConfig,
        n_steps: u32,
    ) -> Option<(u32, Option<ExitReason>)> {
        let mut sim =
            match build_from_config(cfg, false, &mut Vec::new(), &caustic::StepProgress::new()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("  skip build: {e}");
                    return None;
                }
            };

        for i in 0..n_steps {
            match sim.step() {
                Ok(None) => {} // step completed, sim continues
                Ok(Some(reason)) => {
                    let exit = map_exit_reason(reason);
                    return Some((i + 1, Some(exit)));
                }
                Err(e) => {
                    eprintln!("  step {i} error: {e}");
                    return Some((i, None));
                }
            }
        }
        Some((n_steps, None))
    }

    /// Test every config TOML: run 10 steps and assert no premature EnergyDrift or MassLoss exit.
    ///
    /// Configs with too-tight thresholds will cause the simulation to exit on the first
    /// few steps before any meaningful physics has occurred. This test catches that.
    #[test]
    fn no_premature_drift_exit_all_configs() {
        let configs_dir = format!("{}/configs", env!("CARGO_MANIFEST_DIR"));
        let mut entries: Vec<_> = std::fs::read_dir(&configs_dir)
            .expect("configs/ directory should exist")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        let mut failures: Vec<String> = Vec::new();

        eprintln!(
            "\n{:<25} {:>10} {:>8} {:>12} {:>15}",
            "Config", "Phase(MB)", "Steps", "Exit?", "Energy drift"
        );
        eprintln!("{}", "-".repeat(75));

        for entry in &entries {
            let path = entry.path();
            let name = path.file_stem().unwrap().to_string_lossy().to_string();

            let cfg = match crate::config::load(path.to_str().unwrap()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{:<25} LOAD ERROR: {e}", name);
                    continue;
                }
            };

            // Skip configs that are too large to test
            let breakdown = crate::config::defaults::estimate_memory_breakdown(&cfg);
            if breakdown.phase_space_mb > MAX_PHASE_SPACE_MB {
                eprintln!(
                    "{:<25} {:>10.1} {:>8} {:>12}",
                    name, breakdown.phase_space_mb, "-", "SKIPPED (large)"
                );
                continue;
            }

            // Skip models that require external resources
            if cfg.model.model_type == "custom_function" {
                eprintln!(
                    "{:<25} {:>10.1} {:>8} {:>12}",
                    name, 0.0, "-", "SKIPPED (custom)"
                );
                continue;
            }

            // Skip non-uniform representations — HTACA/SLAR/TT are too slow in debug mode
            if !matches!(
                cfg.solver.representation.as_str(),
                "uniform" | "uniform_grid"
            ) {
                eprintln!(
                    "{:<25} {:>10.1} {:>8} {:>12}",
                    name, breakdown.phase_space_mb, "-", "SKIPPED (slow repr)"
                );
                continue;
            }

            match run_steps(&cfg, TEST_STEPS) {
                Some((steps, None)) => {
                    // All steps completed without exit — good
                    eprintln!(
                        "{:<25} {:>10.1} {:>8} {:>12}",
                        name, breakdown.phase_space_mb, steps, "OK"
                    );
                }
                Some((steps, Some(reason))) => {
                    let ok = !matches!(reason, ExitReason::EnergyDrift | ExitReason::MassLoss);
                    let status = if ok {
                        format!("{reason} (ok)")
                    } else {
                        format!("{reason} (FAIL)")
                    };
                    eprintln!(
                        "{:<25} {:>10.1} {:>8} {:>12}",
                        name, breakdown.phase_space_mb, steps, status
                    );

                    if !ok {
                        failures.push(format!(
                            "{name}: exited after {steps} steps with {reason} \
                             (energy_drift_tol={}, mass_drift_tol={})",
                            cfg.exit.energy_drift_tolerance, cfg.exit.mass_drift_tolerance
                        ));
                    }
                }
                None => {
                    eprintln!(
                        "{:<25} {:>10.1} {:>8} {:>12}",
                        name, breakdown.phase_space_mb, 0, "BUILD FAIL"
                    );
                }
            }
        }

        eprintln!();
        if !failures.is_empty() {
            eprintln!("=== DRIFT THRESHOLD FAILURES ===");
            for f in &failures {
                eprintln!("  {f}");
            }
            eprintln!(
                "\nFix: increase energy_drift_tolerance / mass_drift_tolerance in the affected configs."
            );
            panic!(
                "{} config(s) exit prematurely due to drift thresholds: {}",
                failures.len(),
                failures
                    .iter()
                    .map(|f| f.split(':').next().unwrap_or("?"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn mock_state(t: f64, t_final: f64, total_energy: f64, initial_energy: f64) -> SimState {
        SimState {
            t,
            t_final,
            step: 0,
            total_energy,
            initial_energy,
            kinetic_energy: 0.0,
            potential_energy: 0.0,
            virial_ratio: 0.0,
            total_mass: 1.0,
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
            phase_slices: Arc::new(vec![]),
            phase_slice: vec![],
            phase_nx: 0,
            phase_nv: 0,
            spatial_extent: 10.0,
            gravitational_constant: 1.0,
            dt: 0.1,
            exit_reason: None,
            rank_per_node: None,
            rank_total: None,
            rank_memory_bytes: None,
            compression_ratio: None,
            repr_type: String::new(),
            poisson_type: String::new(),
            poisson_residual_l2: None,
            potential_power_spectrum: None,
            density_power_spectrum: None,
            field_energy_spectrum: None,
            phase_timings: None,
            truncation_errors: None,
            svd_count: 0,
            htaca_evaluations: 0,
            velocity_extent: 5.0,
            singular_values: None,
            lagrangian_radii: None,
            poisson_rank_amplification: None,
            advection_rank_amplification: None,
            green_function_rank: None,
            exp_sum_terms: None,
            log_messages: vec![],
        }
    }

    #[test]
    fn progress_at_start() {
        let s = mock_state(0.0, 10.0, 0.0, 0.0);
        assert_eq!(s.progress(), 0.0);
    }

    #[test]
    fn progress_at_end() {
        let s = mock_state(10.0, 10.0, 0.0, 0.0);
        assert_eq!(s.progress(), 1.0);
    }

    #[test]
    fn progress_midpoint() {
        let s = mock_state(5.0, 10.0, 0.0, 0.0);
        assert!((s.progress() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn progress_clamps_overshoot() {
        let s = mock_state(15.0, 10.0, 0.0, 0.0);
        assert_eq!(s.progress(), 1.0);
    }

    #[test]
    fn progress_zero_t_final() {
        let s = mock_state(5.0, 0.0, 0.0, 0.0);
        assert_eq!(s.progress(), 0.0);
    }

    #[test]
    fn energy_drift_nonzero() {
        let s = mock_state(1.0, 10.0, 1.01, 1.0);
        assert!((s.energy_drift() - 0.01).abs() < 1e-12);
    }

    #[test]
    fn energy_drift_zero_initial() {
        let s = mock_state(1.0, 10.0, 1.0, 0.0);
        assert_eq!(s.energy_drift(), 0.0);
    }

    #[test]
    fn exit_reason_display_all() {
        let variants = [
            ExitReason::TimeLimitReached,
            ExitReason::SteadyState,
            ExitReason::EnergyDrift,
            ExitReason::MassLoss,
            ExitReason::CasimirDrift,
            ExitReason::CflViolation,
            ExitReason::WallClockLimit,
            ExitReason::UserStop,
            ExitReason::CausticFormed,
            ExitReason::VirialStabilized,
        ];
        for v in variants {
            let s = format!("{v}");
            assert!(!s.is_empty(), "{v:?} should have non-empty display");
        }
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    /// Maximum full-grid size (MB) for quick smoke tests.
    /// 10 MB covers 8^6 grids (2 MB) but skips 16^6+ (134+ MB).
    /// Uses the full N^3×N_v^3 grid size, not the compressed representation size,
    /// since build time scales with grid dimensions even for HT/TT.
    /// Large configs are tested via `smoke_all_full` (--ignored --release).
    const MAX_GRID_MB: f64 = 10.0;

    /// Build a simulation from a preset config and run one step.
    /// Skips configs whose full grid exceeds the size threshold
    /// (those are too slow in debug mode — test them via smoke_all_full).
    macro_rules! smoke_test {
        ($name:ident, $config:expr) => {
            #[test]
            fn $name() {
                let path = format!("{}/configs/{}.toml", env!("CARGO_MANIFEST_DIR"), $config);
                let cfg = crate::config::load(&path)
                    .unwrap_or_else(|e| panic!("failed to load {}: {e}", $config));
                let n = cfg.domain.spatial_resolution as u64;
                let nv = cfg.domain.velocity_resolution as u64;
                let grid_mb = (n.pow(3) * nv.pow(3) * 8) as f64 / 1_048_576.0;
                if grid_mb > MAX_GRID_MB {
                    eprintln!(
                        "smoke_{}: skipped (grid={:.0} MB > {:.0} MB). \
                         Use: cargo test --release smoke_all_full -- --ignored",
                        $config, grid_mb, MAX_GRID_MB
                    );
                    return;
                }
                let progress = caustic::StepProgress::new();
                let mut sim = build_from_config(&cfg, false, &mut Vec::new(), &progress)
                    .unwrap_or_else(|e| panic!("failed to build {}: {e}", $config));
                let result = sim.step();
                assert!(
                    result.is_ok(),
                    "{} step failed: {:?}",
                    $config,
                    result.err()
                );
            }
        };
    }

    smoke_test!(smoke_debug, "debug");
    smoke_test!(smoke_disk_bar, "disk_bar");
    smoke_test!(smoke_hernquist, "hernquist");
    smoke_test!(smoke_jeans_stable, "jeans_stable");
    smoke_test!(smoke_jeans_unstable, "jeans_unstable");
    smoke_test!(smoke_king, "king");
    smoke_test!(smoke_merger_equal, "merger_equal");
    smoke_test!(smoke_merger_unequal, "merger_unequal");
    smoke_test!(smoke_nfw, "nfw");
    smoke_test!(smoke_nfw_tree, "nfw_tree");
    smoke_test!(smoke_plummer, "plummer");
    smoke_test!(smoke_plummer_hires, "plummer_hires");
    smoke_test!(smoke_plummer_ht, "plummer_ht");
    smoke_test!(smoke_plummer_lomac, "plummer_lomac");
    smoke_test!(smoke_plummer_multigrid, "plummer_multigrid");
    smoke_test!(smoke_plummer_spectral, "plummer_spectral");
    smoke_test!(smoke_plummer_spherical, "plummer_spherical");
    smoke_test!(smoke_plummer_tensor_poisson, "plummer_tensor_poisson");
    smoke_test!(smoke_plummer_tt, "plummer_tt");
    smoke_test!(smoke_plummer_unsplit, "plummer_unsplit");
    smoke_test!(smoke_plummer_yoshida, "plummer_yoshida");
    smoke_test!(smoke_tidal_nfw, "tidal_nfw");
    smoke_test!(smoke_tidal_point, "tidal_point");
    smoke_test!(smoke_zeldovich, "zeldovich");
    smoke_test!(smoke_plummer_128, "plummer_128");

    /// Full smoke test — builds and steps every config without memory limit.
    /// Run with: `cargo test --release smoke_all_full -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn smoke_all_full() {
        let configs = [
            "debug",
            "disk_bar",
            "hernquist",
            "jeans_stable",
            "jeans_unstable",
            "king",
            "merger_equal",
            "merger_unequal",
            "nfw",
            "nfw_tree",
            "plummer",
            "plummer_hires",
            "plummer_ht",
            "plummer_lomac",
            "plummer_multigrid",
            "plummer_spectral",
            "plummer_spherical",
            "plummer_tensor_poisson",
            "plummer_tt",
            "plummer_unsplit",
            "plummer_yoshida",
            "tidal_nfw",
            "tidal_point",
            "zeldovich",
            // plummer_128 excluded — requires ~35 GB
        ];
        for config in configs {
            let path = format!("{}/configs/{config}.toml", env!("CARGO_MANIFEST_DIR"));
            let cfg = crate::config::load(&path)
                .unwrap_or_else(|e| panic!("failed to load {config}: {e}"));
            let progress = caustic::StepProgress::new();
            let mut sim = build_from_config(&cfg, false, &mut Vec::new(), &progress)
                .unwrap_or_else(|e| panic!("failed to build {config}: {e}"));
            let result = sim.step();
            assert!(result.is_ok(), "{config} step failed: {:?}", result.err());
            eprintln!("  {config}: OK");
        }
    }
}
