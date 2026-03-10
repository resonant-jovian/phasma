//! PlaybackDataProvider — reads snapshots from a completed run directory for TUI replay.

use super::DataProvider;
use super::live::DiagnosticsStore;
use crate::config::PhasmaConfig;
use crate::sim::SimState;

pub struct PlaybackDataProvider {
    snapshots: Vec<SimState>,
    current_index: usize,
    diagnostics: DiagnosticsStore,
    config: Option<PhasmaConfig>,
    playing: bool,
    fps: f64,
    looping: bool,
    last_advance: std::time::Instant,
}

impl PlaybackDataProvider {
    pub fn new(snapshots: Vec<SimState>, config: Option<PhasmaConfig>) -> Self {
        let mut diagnostics = DiagnosticsStore::default();
        for s in &snapshots {
            diagnostics.push_state(s);
        }

        Self {
            snapshots,
            current_index: 0,
            diagnostics,
            config,
            playing: false,
            fps: 10.0,
            looping: false,
            last_advance: std::time::Instant::now(),
        }
    }

    /// Call each tick — auto-advances if playing.
    pub fn tick(&mut self) {
        if !self.playing || self.snapshots.is_empty() {
            return;
        }
        let interval = std::time::Duration::from_secs_f64(1.0 / self.fps);
        if self.last_advance.elapsed() >= interval {
            if self.current_index + 1 < self.snapshots.len() {
                self.current_index += 1;
            } else if self.looping {
                self.current_index = 0;
            } else {
                self.playing = false;
            }
            self.last_advance = std::time::Instant::now();
        }
    }

    pub fn play(&mut self) {
        self.playing = true;
        self.last_advance = std::time::Instant::now();
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle_play(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn step_forward(&mut self) {
        if self.current_index + 1 < self.snapshots.len() {
            self.current_index += 1;
        }
    }

    pub fn step_backward(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
    }
}

impl DataProvider for PlaybackDataProvider {
    fn current_state(&self) -> Option<&SimState> {
        self.snapshots.get(self.current_index)
    }

    fn density_projection(&self, axis: usize) -> Option<(Vec<f64>, usize, usize)> {
        let s = self.snapshots.get(self.current_index)?;
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
        let s = self.snapshots.get(self.current_index)?;
        let idx = dim_x.min(2) * 3 + dim_v.min(2);
        if let Some(slice) = s.phase_slices.get(idx) {
            if !slice.is_empty() {
                Some((slice.clone(), s.phase_nx, s.phase_nv))
            } else {
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
        if self.snapshots.is_empty() {
            None
        } else {
            Some((self.current_index, self.snapshots.len()))
        }
    }

    fn scrub_backward(&mut self) {
        self.step_backward();
    }

    fn scrub_forward(&mut self) {
        self.step_forward();
    }

    fn scrub_to_live(&mut self) {
        if !self.snapshots.is_empty() {
            self.current_index = self.snapshots.len() - 1;
        }
    }
}
