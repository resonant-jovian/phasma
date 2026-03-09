use crate::{config::PhasmaConfig, sim::SimState};
use super::{DataProvider, DiagnosticsStore, live::LiveDataProvider};

/// Stub playback provider.  All data methods return None until a real
/// HDF5/Parquet backend is implemented.
pub struct PlaybackProvider {
    inner: LiveDataProvider,
    source_dir: String,
}

impl PlaybackProvider {
    pub fn new(source_dir: impl Into<String>) -> Self {
        Self {
            inner: LiveDataProvider::default(),
            source_dir: source_dir.into(),
        }
    }
}

impl DataProvider for PlaybackProvider {
    fn current_state(&self) -> Option<&SimState> { None }

    fn diagnostics(&self) -> &DiagnosticsStore {
        self.inner.diagnostics()
    }

    fn density_projection(&self, _axis: usize) -> Option<(Vec<f64>, usize, usize)> {
        tracing::info!("PlaybackProvider: density_projection — requires HDF5/Parquet backend (dir: {})", self.source_dir);
        None
    }

    fn phase_slice(&self, _dim_x: usize, _dim_v: usize, _fixed: &[(usize, f64)]) -> Option<(Vec<f64>, usize, usize)> {
        None
    }

    fn config(&self) -> Option<&PhasmaConfig> { None }

    fn is_live(&self) -> bool { false }

    fn tick(&mut self) -> bool { false }
}
