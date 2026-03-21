//! ComparisonDataProvider — holds two PlaybackDataProviders for side-by-side comparison.
//! The `c` key cycles between showing Run A, Run B, or Diff.

use std::borrow::Cow;

use super::DataProvider;
use super::live::DiagnosticsStore;
use super::playback::PlaybackDataProvider;
use crate::config::PhasmaConfig;
use crate::sim::SimState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonView {
    RunA,
    RunB,
    Diff,
}

impl ComparisonView {
    pub fn cycle(self) -> Self {
        match self {
            Self::RunA => Self::RunB,
            Self::RunB => Self::Diff,
            Self::Diff => Self::RunA,
        }
    }
}

pub struct ComparisonDataProvider {
    pub a: PlaybackDataProvider,
    pub b: PlaybackDataProvider,
    pub view: ComparisonView,
    /// Empty diagnostics store (comparison doesn't track time series).
    diagnostics: DiagnosticsStore,
}

impl ComparisonDataProvider {
    pub fn new(a: PlaybackDataProvider, b: PlaybackDataProvider) -> Self {
        Self {
            a,
            b,
            view: ComparisonView::RunA,
            diagnostics: DiagnosticsStore::default(),
        }
    }

    pub fn cycle_view(&mut self) {
        self.view = self.view.cycle();
    }

    /// Tick both providers.
    pub fn tick(&mut self) {
        self.a.tick();
        self.b.tick();
    }

    fn active_provider(&self) -> &PlaybackDataProvider {
        match self.view {
            ComparisonView::RunA => &self.a,
            ComparisonView::RunB | ComparisonView::Diff => &self.b,
        }
    }
}

impl DataProvider for ComparisonDataProvider {
    fn current_state(&self) -> Option<&SimState> {
        match self.view {
            ComparisonView::RunA => self.a.current_state(),
            ComparisonView::RunB => self.b.current_state(),
            ComparisonView::Diff => {
                // Can't return reference to computed value; use active provider as fallback
                self.a.current_state()
            }
        }
    }

    fn density_projection(&self, axis: usize) -> Option<(Cow<'_, [f64]>, usize, usize)> {
        match self.view {
            ComparisonView::RunA => self.a.density_projection(axis),
            ComparisonView::RunB => self.b.density_projection(axis),
            ComparisonView::Diff => {
                let (da, nx, ny) = self.a.density_projection(axis)?;
                let (db, _, _) = self.b.density_projection(axis)?;
                Some((Cow::Owned(diff_vec(&da, &db)), nx, ny))
            }
        }
    }

    fn phase_slice(
        &self,
        dim_x: usize,
        dim_v: usize,
        fixed: &[(usize, f64)],
    ) -> Option<(Cow<'_, [f64]>, usize, usize)> {
        match self.view {
            ComparisonView::RunA => self.a.phase_slice(dim_x, dim_v, fixed),
            ComparisonView::RunB => self.b.phase_slice(dim_x, dim_v, fixed),
            ComparisonView::Diff => {
                let (da, nx, nv) = self.a.phase_slice(dim_x, dim_v, fixed)?;
                let (db, _, _) = self.b.phase_slice(dim_x, dim_v, fixed)?;
                Some((Cow::Owned(diff_vec(&da, &db)), nx, nv))
            }
        }
    }

    fn config(&self) -> Option<&PhasmaConfig> {
        self.active_provider().config()
    }

    fn diagnostics(&self) -> &DiagnosticsStore {
        match self.view {
            ComparisonView::RunA => self.a.diagnostics(),
            ComparisonView::RunB => self.b.diagnostics(),
            ComparisonView::Diff => &self.diagnostics,
        }
    }

    fn scrub_position(&self) -> Option<(usize, usize)> {
        self.a.scrub_position()
    }

    fn scrub_backward(&mut self) {
        self.a.scrub_backward();
        self.b.scrub_backward();
    }

    fn scrub_forward(&mut self) {
        self.a.scrub_forward();
        self.b.scrub_forward();
    }

    fn scrub_to_live(&mut self) {
        self.a.scrub_to_live();
        self.b.scrub_to_live();
    }

    fn scrub_to_start(&mut self) {
        self.a.scrub_to_start();
        self.b.scrub_to_start();
    }

    fn scrub_to_end(&mut self) {
        self.a.scrub_to_end();
        self.b.scrub_to_end();
    }
}

fn diff_vec(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b.iter()).map(|(&x, &y)| x - y).collect()
}
