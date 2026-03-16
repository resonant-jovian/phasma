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
    // Config
    ConfigLoaded(String),
    // Simulation control
    SimStart,
    SimPause,
    SimResume,
    SimStop,
    SimRestart,
    #[strum(to_string = "SimUpdate")]
    SimUpdate(Box<SimState>),
    StatusMsg(String),
    // Scrubbing (time navigation)
    ScrubBackward,
    ScrubForward,
    ScrubToLive,
    ScrubJumpBackward,
    ScrubJumpForward,
    ScrubToStart,
    ScrubToEnd,
    // Visualization
    VizCycleColormap,
    VizToggleLog,
    VizCycleProjection,
    // Export
    ExportMenuOpen,
    ExportMenuClose,
    // Help
    HelpToggle,
    // Theme
    ThemeCycle,
}
