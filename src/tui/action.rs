use serde::{Deserialize, Serialize};
use strum::Display;

use crate::sim::SimState;

#[derive(Debug, Clone, PartialEq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
    // Tab navigation
    TabNext,
    TabPrev,
    SelectTab(usize),
    // Simulation control
    SimStart,
    SimPause,
    SimResume,
    SimStop,
    #[strum(to_string = "SimUpdate")]
    SimUpdate(SimState),
}
