pub mod cache;
pub mod comparison;
pub mod live;
pub mod playback;

use crate::{config::PhasmaConfig, sim::SimState};

use live::DiagnosticsStore;

/// Uniform interface for both live and playback data sources.
pub trait DataProvider: Send {
    fn current_state(&self) -> Option<&SimState>;
    /// Returns (data, nx, ny) for a density projection along the given axis (0=x, 1=y, 2=z).
    fn density_projection(&self, axis: usize) -> Option<(Vec<f64>, usize, usize)>;
    /// Returns (data, nx, nv) for a phase-space slice.
    fn phase_slice(
        &self,
        dim_x: usize,
        dim_v: usize,
        fixed: &[(usize, f64)],
    ) -> Option<(Vec<f64>, usize, usize)>;
    fn config(&self) -> Option<&PhasmaConfig>;
    fn diagnostics(&self) -> &DiagnosticsStore;
    fn scrub_position(&self) -> Option<(usize, usize)> {
        None
    }
    fn scrub_backward(&mut self) {}
    fn scrub_forward(&mut self) {}
    fn scrub_to_live(&mut self) {}
    /// Jump backward N frames (default 10).
    fn scrub_jump_backward(&mut self, n: usize) {
        for _ in 0..n {
            self.scrub_backward();
        }
    }
    /// Jump forward N frames (default 10).
    fn scrub_jump_forward(&mut self, n: usize) {
        for _ in 0..n {
            self.scrub_forward();
        }
    }
    /// Jump to the first frame.
    fn scrub_to_start(&mut self) {}
    /// Jump to the last frame (same as scrub_to_live for live providers).
    fn scrub_to_end(&mut self) {
        self.scrub_to_live();
    }
}
