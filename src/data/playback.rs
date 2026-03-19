//! PlaybackDataProvider — reads snapshots from a completed run directory for TUI replay.

use std::borrow::Cow;

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

        // Apply playback config settings
        let (fps, looping, start_idx) = if let Some(ref cfg) = config {
            let pb = &cfg.playback;
            let fps = if pb.fps > 0.0 { pb.fps } else { 10.0 };
            let looping = pb.loop_playback;
            // Find start index from start_time
            let start_idx = if let Some(start_t) = pb.start_time {
                snapshots.iter().position(|s| s.t >= start_t).unwrap_or(0)
            } else {
                0
            };
            (fps, looping, start_idx)
        } else {
            (10.0, false, 0)
        };

        // Filter snapshots to [start_time, end_time] range if specified
        let filtered: Vec<SimState> = if let Some(ref cfg) = config {
            let pb = &cfg.playback;
            let start = pb.start_time.unwrap_or(f64::NEG_INFINITY);
            let end = pb.end_time.unwrap_or(f64::INFINITY);
            snapshots
                .into_iter()
                .filter(|s| s.t >= start && s.t <= end)
                .collect()
        } else {
            snapshots
        };

        Self {
            snapshots: filtered,
            current_index: start_idx,
            diagnostics,
            config,
            playing: false,
            fps,
            looping,
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

    /// Increase playback speed (up to 120 fps).
    pub fn increase_speed(&mut self) {
        self.fps = (self.fps * 1.5).min(120.0);
    }

    /// Decrease playback speed (down to 0.5 fps).
    pub fn decrease_speed(&mut self) {
        self.fps = (self.fps / 1.5).max(0.5);
    }

    /// Current playback FPS.
    pub fn fps(&self) -> f64 {
        self.fps
    }

    /// Scrub to the nearest snapshot at or after the given simulation time.
    pub fn scrub_to_time(&mut self, t: f64) {
        if let Some(idx) = self.snapshots.iter().position(|s| s.t >= t) {
            self.current_index = idx;
        } else if !self.snapshots.is_empty() {
            self.current_index = self.snapshots.len() - 1;
        }
    }
}

impl DataProvider for PlaybackDataProvider {
    fn current_state(&self) -> Option<&SimState> {
        self.snapshots.get(self.current_index)
    }

    fn density_projection(&self, axis: usize) -> Option<(Cow<'_, [f64]>, usize, usize)> {
        let s = self.snapshots.get(self.current_index)?;
        match axis {
            0 => Some((Cow::Borrowed(&s.density_yz), s.density_ny, s.density_nz)),
            1 => Some((Cow::Borrowed(&s.density_xz), s.density_nx, s.density_nz)),
            _ => Some((Cow::Borrowed(&s.density_xy), s.density_nx, s.density_ny)),
        }
    }

    fn phase_slice(
        &self,
        dim_x: usize,
        dim_v: usize,
        _fixed: &[(usize, f64)],
    ) -> Option<(Cow<'_, [f64]>, usize, usize)> {
        let s = self.snapshots.get(self.current_index)?;
        let idx = dim_x.min(2) * 3 + dim_v.min(2);
        if let Some(slice) = s.phase_slices.get(idx) {
            if !slice.is_empty() {
                Some((Cow::Borrowed(slice), s.phase_nx, s.phase_nv))
            } else {
                None
            }
        } else {
            None
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

    fn scrub_to_start(&mut self) {
        self.current_index = 0;
    }

    fn scrub_to_end(&mut self) {
        if !self.snapshots.is_empty() {
            self.current_index = self.snapshots.len() - 1;
        }
    }
}
