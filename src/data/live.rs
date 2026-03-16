use std::collections::VecDeque;

use super::DataProvider;
use crate::{config::PhasmaConfig, sim::SimState};

// ── TimeSeriesStore ───────────────────────────────────────────────────────────

const MAX_RECENT: usize = 10_000;
const SUBSAMPLE: usize = 100;

pub struct TimeSeriesStore {
    /// High-resolution recent window: (time, value) pairs
    recent: VecDeque<(f64, f64)>,
    /// Down-sampled long-term history (unbounded — ~16 bytes per entry, grows slowly)
    history: Vec<(f64, f64)>,
    /// The very first value ever pushed (for drift calculations)
    initial: Option<(f64, f64)>,
    subsample_count: usize,
}

impl Default for TimeSeriesStore {
    fn default() -> Self {
        Self {
            recent: VecDeque::with_capacity(MAX_RECENT),
            history: Vec::new(),
            initial: None,
            subsample_count: 0,
        }
    }
}

impl TimeSeriesStore {
    pub fn push(&mut self, t: f64, v: f64) {
        if self.initial.is_none() {
            self.initial = Some((t, v));
        }

        if self.recent.len() >= MAX_RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back((t, v));

        self.subsample_count += 1;
        if self.subsample_count >= SUBSAMPLE {
            self.history.push((t, v));
            self.subsample_count = 0;
        }
    }

    /// Chart data covering the full simulation from t=0 to now.
    /// Returns downsampled history for the older part, then high-resolution recent data.
    pub fn iter_chart_data(&self) -> Vec<(f64, f64)> {
        let recent_start_t = self
            .recent
            .front()
            .map(|(t, _)| *t)
            .unwrap_or(f64::INFINITY);

        // History points that are older than the recent window
        let mut data: Vec<(f64, f64)> = self
            .history
            .iter()
            .copied()
            .filter(|(t, _)| *t < recent_start_t)
            .collect();

        // Then append the full recent window
        data.extend(self.recent.iter().copied());
        data
    }

    pub fn last_value(&self) -> Option<f64> {
        self.recent.back().map(|(_, v)| *v)
    }

    /// The very first value ever recorded (t=0).
    pub fn first_value(&self) -> Option<f64> {
        self.initial.map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.recent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recent.is_empty() && self.history.is_empty()
    }
}

// ── DiagnosticsStore ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct DiagnosticsStore {
    pub total_energy: TimeSeriesStore,
    pub kinetic_energy: TimeSeriesStore,
    pub potential_energy: TimeSeriesStore,
    pub total_mass: TimeSeriesStore,
    pub momentum_x: TimeSeriesStore,
    pub momentum_y: TimeSeriesStore,
    pub momentum_z: TimeSeriesStore,
    pub casimir_c2: TimeSeriesStore,
    pub entropy: TimeSeriesStore,
    pub virial_ratio: TimeSeriesStore,
}

impl DiagnosticsStore {
    pub fn push_state(&mut self, state: &SimState) {
        let t = state.t;
        self.total_energy.push(t, state.total_energy);
        self.kinetic_energy.push(t, state.kinetic_energy);
        self.potential_energy.push(t, state.potential_energy);
        self.total_mass.push(t, state.total_mass);
        self.momentum_x.push(t, state.momentum[0]);
        self.momentum_y.push(t, state.momentum[1]);
        self.momentum_z.push(t, state.momentum[2]);
        self.casimir_c2.push(t, state.casimir_c2);
        self.entropy.push(t, state.entropy);
        self.virial_ratio.push(t, state.virial_ratio);
    }

    pub fn is_empty(&self) -> bool {
        self.total_energy.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn energy_drift_series(&self) -> Vec<(f64, f64)> {
        let Some(e0) = self.total_energy.first_value() else {
            return Vec::new();
        };
        if e0 == 0.0 {
            return Vec::new();
        }
        self.total_energy
            .iter_chart_data()
            .into_iter()
            .map(|(t, e)| (t, (e - e0) / e0.abs()))
            .collect()
    }

    pub fn mass_drift_series(&self) -> Vec<(f64, f64)> {
        let Some(m0) = self.total_mass.first_value() else {
            return Vec::new();
        };
        if m0 == 0.0 {
            return Vec::new();
        }
        self.total_mass
            .iter_chart_data()
            .into_iter()
            .map(|(t, m)| (t, (m - m0) / m0.abs()))
            .collect()
    }

    pub fn c2_drift_series(&self) -> Vec<(f64, f64)> {
        let Some(c0) = self.casimir_c2.first_value() else {
            return Vec::new();
        };
        if c0 == 0.0 {
            return Vec::new();
        }
        self.casimir_c2
            .iter_chart_data()
            .into_iter()
            .map(|(t, c)| (t, (c - c0) / c0.abs()))
            .collect()
    }
}

// ── LiveDataProvider ─────────────────────────────────────────────────────────

const SNAPSHOT_HISTORY_CAP: usize = 100;

pub struct LiveDataProvider {
    current: Option<SimState>,
    pub diagnostics: DiagnosticsStore,
    config: Option<PhasmaConfig>,
    /// Ring buffer of recent SimState snapshots for scrubbing.
    snapshot_history: VecDeque<SimState>,
    /// Index into snapshot_history for scrubbing. None = live (latest).
    scrub_index: Option<usize>,
    /// Subsample counter to avoid storing every single step.
    snap_subsample: usize,
}

impl Default for LiveDataProvider {
    fn default() -> Self {
        Self {
            current: None,
            diagnostics: DiagnosticsStore::default(),
            config: None,
            snapshot_history: VecDeque::with_capacity(SNAPSHOT_HISTORY_CAP),
            scrub_index: None,
            snap_subsample: 0,
        }
    }
}

impl LiveDataProvider {
    pub fn set_config(&mut self, config: PhasmaConfig) {
        self.config = Some(config);
    }

    /// Reset all data for a new simulation run (preserves config).
    pub fn reset(&mut self) {
        self.current = None;
        self.diagnostics.clear();
        self.snapshot_history.clear();
        self.scrub_index = None;
        self.snap_subsample = 0;
        // config is preserved — it's set by ConfigLoaded action
    }

    /// Ingest a new SimState (called from App when SimUpdate arrives).
    pub fn update(&mut self, state: &SimState) {
        self.diagnostics.push_state(state);

        // Store snapshots for scrubbing (every 5th step to limit memory)
        self.snap_subsample += 1;
        if self.snap_subsample >= 5 {
            if self.snapshot_history.len() >= SNAPSHOT_HISTORY_CAP {
                self.snapshot_history.pop_front();
                // Adjust scrub index if it was pointing into the removed region
                if let Some(ref mut idx) = self.scrub_index {
                    if *idx > 0 {
                        *idx -= 1;
                    } else {
                        self.scrub_index = None;
                    }
                }
            }
            self.snapshot_history.push_back(state.clone());
            self.snap_subsample = 0;
        }

        self.current = Some(state.clone());
    }

    /// Scrub backward one snapshot in history.
    pub fn scrub_backward(&mut self) {
        if self.snapshot_history.is_empty() {
            return;
        }
        match self.scrub_index {
            None => {
                // Go from live to the last snapshot
                self.scrub_index = Some(self.snapshot_history.len().saturating_sub(1));
            }
            Some(idx) => {
                if idx > 0 {
                    self.scrub_index = Some(idx - 1);
                }
            }
        }
    }

    /// Scrub forward one snapshot toward live.
    pub fn scrub_forward(&mut self) {
        if let Some(idx) = self.scrub_index {
            if idx + 1 >= self.snapshot_history.len() {
                self.scrub_index = None; // back to live
            } else {
                self.scrub_index = Some(idx + 1);
            }
        }
    }

    /// Jump back to live (latest state).
    pub fn scrub_to_live(&mut self) {
        self.scrub_index = None;
    }

    /// Scrub to the snapshot nearest to the given time.
    pub fn scrub_to_time(&mut self, time: f64) {
        if self.snapshot_history.is_empty() {
            return;
        }
        let mut best_idx = 0;
        let mut best_dist = f64::INFINITY;
        for (i, snap) in self.snapshot_history.iter().enumerate() {
            let dist = (snap.t - time).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }
        self.scrub_index = Some(best_idx);
    }

    /// Get the effective state (scrubbed or live).
    fn effective_state(&self) -> Option<&SimState> {
        if let Some(idx) = self.scrub_index {
            self.snapshot_history.get(idx)
        } else {
            self.current.as_ref()
        }
    }
}

impl DataProvider for LiveDataProvider {
    fn current_state(&self) -> Option<&SimState> {
        self.effective_state()
    }

    fn density_projection(&self, axis: usize) -> Option<(Vec<f64>, usize, usize)> {
        let s = self.effective_state()?;
        match axis {
            0 => Some((s.density_yz.clone(), s.density_ny, s.density_nz)),
            1 => Some((s.density_xz.clone(), s.density_nx, s.density_nz)),
            _ => Some((s.density_xy.clone(), s.density_nx, s.density_ny)),
        }
    }

    fn phase_slice(
        &self,
        dim_x: usize,
        dim_v: usize,
        _fixed: &[(usize, f64)],
    ) -> Option<(Vec<f64>, usize, usize)> {
        let s = self.effective_state()?;
        let idx = dim_x.min(2) * 3 + dim_v.min(2);
        if let Some(slice) = s.phase_slices.get(idx) {
            if !slice.is_empty() {
                // Infer nx/nv from the slice: phase_nx/phase_nv are for dim 0.
                // All spatial dims have same resolution and all velocity dims have same resolution.
                Some((slice.clone(), s.phase_nx, s.phase_nv))
            } else {
                // Fallback to legacy single slice
                Some((s.phase_slice.clone(), s.phase_nx, s.phase_nv))
            }
        } else {
            Some((s.phase_slice.clone(), s.phase_nx, s.phase_nv))
        }
    }

    fn config(&self) -> Option<&PhasmaConfig> {
        self.config.as_ref()
    }

    fn diagnostics(&self) -> &DiagnosticsStore {
        &self.diagnostics
    }

    fn scrub_position(&self) -> Option<(usize, usize)> {
        self.scrub_index
            .map(|idx| (idx, self.snapshot_history.len()))
    }

    fn scrub_backward(&mut self) {
        LiveDataProvider::scrub_backward(self);
    }

    fn scrub_forward(&mut self) {
        LiveDataProvider::scrub_forward(self);
    }

    fn scrub_to_live(&mut self) {
        LiveDataProvider::scrub_to_live(self);
    }

    fn scrub_to_start(&mut self) {
        if !self.snapshot_history.is_empty() {
            self.scrub_index = Some(0);
        }
    }

    fn scrub_to_end(&mut self) {
        if !self.snapshot_history.is_empty() {
            self.scrub_index = Some(self.snapshot_history.len() - 1);
        }
    }
}
