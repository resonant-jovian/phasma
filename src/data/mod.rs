pub mod cache;
pub mod comparison;
pub mod live;
pub mod playback;

pub use live::DiagnosticsStore;

use crate::{config::PhasmaConfig, sim::SimState};

/// Uniform interface for both live and playback data sources.
pub trait DataProvider: Send {
    fn current_state(&self) -> Option<&SimState>;
    fn diagnostics(&self) -> &DiagnosticsStore;
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
    fn is_live(&self) -> bool;
    /// Poll for new data (drain channels etc.).  Returns true if new data arrived.
    fn tick(&mut self) -> bool;
}
