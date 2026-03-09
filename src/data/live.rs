use std::collections::VecDeque;

use crate::{config::PhasmaConfig, sim::SimState};
use super::DataProvider;

// ── TimeSeriesStore ───────────────────────────────────────────────────────────

const MAX_RECENT: usize = 10_000;
const HISTORY_CAP: usize = 100;
const SUBSAMPLE: usize = 100;

pub struct TimeSeriesStore {
    /// High-resolution recent window: (time, value) pairs
    recent: VecDeque<(f64, f64)>,
    /// Down-sampled long-term history
    history: VecDeque<(f64, f64)>,
    subsample_count: usize,
}

impl Default for TimeSeriesStore {
    fn default() -> Self {
        Self {
            recent: VecDeque::with_capacity(MAX_RECENT),
            history: VecDeque::with_capacity(HISTORY_CAP),
            subsample_count: 0,
        }
    }
}

impl TimeSeriesStore {
    pub fn push(&mut self, t: f64, v: f64) {
        if self.recent.len() >= MAX_RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back((t, v));

        self.subsample_count += 1;
        if self.subsample_count >= SUBSAMPLE {
            if self.history.len() >= HISTORY_CAP {
                self.history.pop_front();
            }
            self.history.push_back((t, v));
            self.subsample_count = 0;
        }
    }

    /// Chart data as (time, value) pairs from the recent window.
    pub fn iter_chart_data(&self) -> Vec<(f64, f64)> {
        self.recent.iter().copied().collect()
    }

    pub fn last_value(&self) -> Option<f64> {
        self.recent.back().map(|(_, v)| *v)
    }

    pub fn first_value(&self) -> Option<f64> {
        self.recent.front().map(|(_, v)| *v)
    }

    pub fn len(&self) -> usize {
        self.recent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recent.is_empty()
    }
}

// ── DiagnosticsStore ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct DiagnosticsStore {
    pub total_energy:     TimeSeriesStore,
    pub kinetic_energy:   TimeSeriesStore,
    pub potential_energy: TimeSeriesStore,
    pub total_mass:       TimeSeriesStore,
    pub momentum_x:       TimeSeriesStore,
    pub momentum_y:       TimeSeriesStore,
    pub momentum_z:       TimeSeriesStore,
    pub casimir_c2:       TimeSeriesStore,
    pub entropy:          TimeSeriesStore,
    pub virial_ratio:     TimeSeriesStore,
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

    pub fn energy_drift_series(&self) -> Vec<(f64, f64)> {
        let Some(e0) = self.total_energy.first_value() else { return Vec::new() };
        if e0 == 0.0 { return Vec::new(); }
        self.total_energy.recent.iter()
            .map(|&(t, e)| (t, (e - e0) / e0.abs()))
            .collect()
    }

    pub fn mass_drift_series(&self) -> Vec<(f64, f64)> {
        let Some(m0) = self.total_mass.first_value() else { return Vec::new() };
        if m0 == 0.0 { return Vec::new(); }
        self.total_mass.recent.iter()
            .map(|&(t, m)| (t, (m - m0) / m0.abs()))
            .collect()
    }

    pub fn c2_drift_series(&self) -> Vec<(f64, f64)> {
        let Some(c0) = self.casimir_c2.first_value() else { return Vec::new() };
        if c0 == 0.0 { return Vec::new(); }
        self.casimir_c2.recent.iter()
            .map(|&(t, c)| (t, (c - c0) / c0.abs()))
            .collect()
    }
}

// ── LiveDataProvider ─────────────────────────────────────────────────────────

pub struct LiveDataProvider {
    current: Option<SimState>,
    pub diagnostics: DiagnosticsStore,
    config: Option<PhasmaConfig>,
}

impl Default for LiveDataProvider {
    fn default() -> Self {
        Self {
            current: None,
            diagnostics: DiagnosticsStore::default(),
            config: None,
        }
    }
}

impl LiveDataProvider {
    pub fn new(config: Option<PhasmaConfig>) -> Self {
        Self { current: None, diagnostics: DiagnosticsStore::default(), config }
    }

    /// Ingest a new SimState (called from App when SimUpdate arrives).
    pub fn update(&mut self, state: &SimState) {
        self.diagnostics.push_state(state);
        self.current = Some(state.clone());
    }
}

impl DataProvider for LiveDataProvider {
    fn current_state(&self) -> Option<&SimState> {
        self.current.as_ref()
    }

    fn diagnostics(&self) -> &DiagnosticsStore {
        &self.diagnostics
    }

    fn density_projection(&self, axis: usize) -> Option<(Vec<f64>, usize, usize)> {
        let s = self.current.as_ref()?;
        match axis {
            0 => Some((s.density_yz.clone(), s.density_ny, s.density_nz)),
            1 => Some((s.density_xz.clone(), s.density_nx, s.density_nz)),
            _ => Some((s.density_xy.clone(), s.density_nx, s.density_ny)),
        }
    }

    fn phase_slice(
        &self,
        _dim_x: usize,
        _dim_v: usize,
        _fixed: &[(usize, f64)],
    ) -> Option<(Vec<f64>, usize, usize)> {
        let s = self.current.as_ref()?;
        Some((s.phase_slice.clone(), s.phase_nx, s.phase_nv))
    }

    fn config(&self) -> Option<&PhasmaConfig> {
        self.config.as_ref()
    }

    fn is_live(&self) -> bool {
        true
    }

    fn tick(&mut self) -> bool {
        false // caller drains channel and calls update()
    }
}
