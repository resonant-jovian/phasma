pub mod density;
pub mod energy;
pub mod performance;
pub mod phase_space;
pub mod poisson_detail;
pub mod profiles;
pub mod rank;
pub mod run_control;
pub mod settings;
pub mod setup;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Block,
};
use strum::{Display, EnumCount, EnumIter, FromRepr, IntoEnumIterator};
use tokio::sync::mpsc::UnboundedSender;

use std::sync::Arc;

use ratatui_plt::prelude::Theme;

use crate::{
    data::DataProvider,
    tui::{action::Action, config::Config, layout::LayoutMode, plt_bridge::PhasmaThemeExt},
};

use density::DensityTab;
use energy::EnergyTab;
use performance::PerformanceTab;
use phase_space::PhaseSpaceTab;
use poisson_detail::PoissonDetailTab;
use profiles::ProfilesTab;
use rank::RankTab;
use run_control::RunControlTab;
use settings::SettingsTab;
use setup::SetupTab;

#[derive(Default, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumCount, FromRepr)]
pub enum Tab {
    #[default]
    #[strum(to_string = "F1 Setup")]
    Setup,
    #[strum(to_string = "F2 Run")]
    RunControl,
    #[strum(to_string = "F3 Density")]
    Density,
    #[strum(to_string = "F4 Phase")]
    PhaseSpace,
    #[strum(to_string = "F5 Energy")]
    Energy,
    #[strum(to_string = "F6 Rank")]
    Rank,
    #[strum(to_string = "F7 Profiles")]
    Profiles,
    #[strum(to_string = "F8 Perf")]
    Performance,
    #[strum(to_string = "F9 Poisson")]
    PoissonDetail,
    #[strum(to_string = "F10 Settings")]
    Settings,
}

pub struct TabAreas {
    pub tab_bar: Rect,
    pub content: Rect,
    pub layout_mode: LayoutMode,
}

pub struct TabView {
    pub selected: Tab,
    setup: SetupTab,
    run_control: RunControlTab,
    density: DensityTab,
    phase_space: PhaseSpaceTab,
    energy: EnergyTab,
    rank: RankTab,
    profiles: ProfilesTab,
    performance: PerformanceTab,
    poisson_detail: PoissonDetailTab,
    settings: SettingsTab,
    command_tx: Option<UnboundedSender<Action>>,
}

impl TabView {
    pub fn new(config_path: Option<String>) -> Self {
        Self {
            selected: Tab::default(),
            setup: SetupTab::new(config_path),
            run_control: RunControlTab::default(),
            density: DensityTab::default(),
            phase_space: PhaseSpaceTab::default(),
            energy: EnergyTab::default(),
            rank: RankTab::default(),
            profiles: ProfilesTab::default(),
            performance: PerformanceTab::default(),
            poisson_detail: PoissonDetailTab::default(),
            settings: SettingsTab::default(),
            command_tx: None,
        }
    }

    pub fn set_step_progress(&mut self, p: Arc<caustic::StepProgress>) {
        self.run_control.set_progress(p);
    }

    pub fn clear_step_progress(&mut self) {
        self.run_control.clear_progress();
    }

    pub fn restore_tab(&mut self, index: usize) {
        self.selected = Tab::from_repr(index).unwrap_or_default();
    }

    /// Sync settings tab state from app-level theme/colormap.
    pub fn sync_settings(&mut self, theme_name: &str, colormap_name: &str) {
        self.settings.sync(theme_name, colormap_name);
    }

    /// Read the theme name chosen in the settings tab.
    pub fn settings_theme_name(&self) -> String {
        self.settings.current_theme_name()
    }

    /// Read the colormap name chosen in the settings tab.
    pub fn settings_colormap_name(&self) -> String {
        self.settings.current_colormap_name()
    }

    /// Toggle the preset popup on the Setup tab (§2.2).
    pub fn setup_toggle_presets(&mut self) {
        self.setup.toggle_preset_popup();
    }

    /// Reset Setup tab config to defaults (§2.2).
    pub fn setup_reset_defaults(&mut self) {
        self.setup.reset_to_defaults();
    }

    pub fn register_action_handler(&mut self, tx: UnboundedSender<Action>) {
        self.command_tx = Some(tx.clone());
        self.setup.register_action_handler(tx.clone());
        self.run_control.register_action_handler(tx.clone());
    }

    pub fn register_config_handler(&mut self, config: Config) {
        self.setup.register_config_handler(config.clone());
        self.run_control.register_config_handler(config);
    }

    /// Handle mouse scroll events (zoom on density/phase tabs)
    pub fn handle_scroll(&mut self, delta: i32) {
        match self.selected {
            Tab::Density => self.density.handle_scroll(delta),
            Tab::PhaseSpace => self.phase_space.handle_scroll(delta),
            _ => {}
        }
    }

    /// Handle mouse move events (data cursor on density/phase tabs)
    pub fn handle_mouse_move(&mut self, col: u16, row: u16) {
        match self.selected {
            Tab::Density => self.density.handle_mouse_move(col, row),
            Tab::PhaseSpace => self.phase_space.handle_mouse_move(col, row),
            _ => {}
        }
    }

    pub fn handle_mouse_down(&mut self, col: u16, row: u16) {
        match self.selected {
            Tab::Energy => self.energy.handle_mouse_down(col, row),
            Tab::PhaseSpace => self.phase_space.handle_mouse_down(col, row),
            _ => {}
        }
    }

    pub fn handle_mouse_drag(&mut self, col: u16, row: u16) {
        match self.selected {
            Tab::Energy => self.energy.handle_mouse_drag(col, row),
            Tab::PhaseSpace => self.phase_space.handle_mouse_drag(col, row),
            _ => {}
        }
    }

    pub fn handle_mouse_up(&mut self, col: u16, row: u16) {
        match self.selected {
            Tab::Energy => self.energy.handle_mouse_up(col, row),
            Tab::PhaseSpace => self.phase_space.handle_mouse_up(col, row),
            _ => {}
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::F(1) => return Some(Action::SelectTab(0)),
            KeyCode::F(2) => return Some(Action::SelectTab(1)),
            KeyCode::F(3) => return Some(Action::SelectTab(2)),
            KeyCode::F(4) => return Some(Action::SelectTab(3)),
            KeyCode::F(5) => return Some(Action::SelectTab(4)),
            KeyCode::F(6) => return Some(Action::SelectTab(5)),
            KeyCode::F(7) => return Some(Action::SelectTab(6)),
            KeyCode::F(8) => return Some(Action::SelectTab(7)),
            KeyCode::F(9) => return Some(Action::SelectTab(8)),
            KeyCode::F(10) => return Some(Action::SelectTab(9)),
            KeyCode::Tab => return Some(Action::TabNext),
            KeyCode::BackTab => return Some(Action::TabPrev),
            _ => {}
        }
        match self.selected {
            Tab::Setup => self.setup.handle_key_event(key),
            Tab::RunControl => self.run_control.handle_key_event(key),
            Tab::Density => self.density.handle_key_event(key),
            Tab::PhaseSpace => self.phase_space.handle_key_event(key),
            Tab::Energy => self.energy.handle_key_event(key),
            Tab::Rank => self.rank.handle_key_event(key),
            Tab::Profiles => self.profiles.handle_key_event(key),
            Tab::Performance => self.performance.handle_key_event(key),
            Tab::PoissonDetail => self.poisson_detail.handle_key_event(key),
            Tab::Settings => self.settings.handle_key_event(key),
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::SelectTab(n) => {
                self.selected = Tab::from_repr(*n).unwrap_or_default();
            }
            Action::TabNext => {
                let next = (self.selected as usize + 1) % Tab::COUNT;
                self.selected = Tab::from_repr(next).unwrap_or_default();
            }
            Action::TabPrev => {
                let prev = (self.selected as usize + Tab::COUNT - 1) % Tab::COUNT;
                self.selected = Tab::from_repr(prev).unwrap_or_default();
            }
            Action::SimStart | Action::SimRestart => {
                self.selected = Tab::RunControl;
                self.performance.reset();
            }
            _ => {}
        }

        let mut result = None;
        if let Some(a) = self.setup.update(action) {
            result = Some(a);
        }
        if let Some(a) = self.run_control.update(action) {
            result = result.or(Some(a));
        }
        if let Some(a) = self.density.update(action) {
            result = result.or(Some(a));
        }
        if let Some(a) = self.phase_space.update(action) {
            result = result.or(Some(a));
        }
        if let Some(a) = self.energy.update(action) {
            result = result.or(Some(a));
        }
        if let Some(a) = self.rank.update(action) {
            result = result.or(Some(a));
        }
        if let Some(a) = self.profiles.update(action) {
            result = result.or(Some(a));
        }
        self.performance.update(action);
        result
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        areas: TabAreas,
        theme: &Theme,
        colormap_name: &str,
        data_provider: &dyn DataProvider,
    ) {
        // Tab bar — manual rendering for per-tab dimming
        let repr_type = data_provider
            .current_state()
            .map(|s| s.repr_type.as_str())
            .unwrap_or("");
        let poisson_type = data_provider
            .current_state()
            .map(|s| s.poisson_type.as_str())
            .unwrap_or("");
        let is_ht = repr_type == "ht";
        let is_fft_isolated = poisson_type == "fft_isolated";

        let compact = areas.layout_mode == LayoutMode::Compact;

        let mut tab_spans: Vec<Span> = Vec::new();
        for (i, tab) in Tab::iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::styled(
                    if compact { "|" } else { " | " },
                    Style::default().fg(theme.dim()),
                ));
            }
            let label = if compact {
                compact_tab_label(tab)
            } else {
                tab.to_string()
            };
            let is_selected = i == self.selected as usize;
            let is_dimmed = (matches!(tab, Tab::Rank) && !is_ht)
                || (matches!(tab, Tab::PoissonDetail) && !is_fft_isolated);

            let style = if is_selected {
                Style::default()
                    .fg(theme.foreground)
                    .bg(theme.surface)
                    .add_modifier(Modifier::BOLD)
            } else if is_dimmed {
                Style::default().fg(theme.dim()).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(theme.dim())
            };
            tab_spans.push(Span::styled(label, style));
        }
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Line::from(tab_spans)),
            areas.tab_bar,
        );

        // Bordered content block
        let tab_title = match self.selected {
            Tab::Setup => " ► Setup ",
            Tab::RunControl => " ► Run Control ",
            Tab::Density => " ► Density ",
            Tab::PhaseSpace => " ► Phase Space ",
            Tab::Energy => " ► Energy Conservation ",
            Tab::Rank => " ► Rank Monitor ",
            Tab::Profiles => " ► Profiles ",
            Tab::Performance => " ► Performance ",
            Tab::PoissonDetail => " ► Poisson Detail ",
            Tab::Settings => " ► Settings ",
        };
        let content_block = Block::bordered()
            .title(tab_title)
            .border_style(Style::default().fg(theme.border_color()));
        let inner = content_block.inner(areas.content);
        frame.render_widget(content_block, areas.content);

        // Check if tab is unavailable per §2.6
        let unavailable_msg = match self.selected {
            Tab::Rank if !is_ht => Some("Rank Monitor requires hierarchical_tucker representation"),
            Tab::PoissonDetail if !is_fft_isolated => {
                Some("Poisson Detail requires fft_isolated solver")
            }
            _ => None,
        };

        if let Some(msg) = unavailable_msg {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(vec![Span::styled(
                    format!("  {msg}"),
                    Style::default().fg(theme.dim()),
                )])),
                inner,
            );
        } else {
            match self.selected {
                Tab::Setup => self.setup.draw(frame, inner, theme),
                Tab::RunControl => {
                    self.run_control
                        .draw(frame, inner, theme, colormap_name, data_provider)
                }
                Tab::Density => {
                    self.density
                        .draw(frame, inner, theme, colormap_name, data_provider)
                }
                Tab::PhaseSpace => {
                    self.phase_space
                        .draw(frame, inner, theme, colormap_name, data_provider)
                }
                Tab::Energy => self.energy.draw(frame, inner, theme, data_provider),
                Tab::Rank => self.rank.draw(frame, inner, theme, data_provider),
                Tab::Profiles => self.profiles.draw(frame, inner, theme, data_provider),
                Tab::Performance => self.performance.draw(frame, inner, theme, data_provider),
                Tab::PoissonDetail => self.poisson_detail.draw(frame, inner, theme, data_provider),
                Tab::Settings => self.settings.draw(frame, inner, theme),
            }
        }
    }
}

/// Abbreviated tab labels for compact mode (§2.5).
fn compact_tab_label(tab: Tab) -> String {
    match tab {
        Tab::Setup => "1".to_string(),
        Tab::RunControl => "2".to_string(),
        Tab::Density => "3".to_string(),
        Tab::PhaseSpace => "4".to_string(),
        Tab::Energy => "5".to_string(),
        Tab::Rank => "6".to_string(),
        Tab::Profiles => "7".to_string(),
        Tab::Performance => "8".to_string(),
        Tab::PoissonDetail => "9".to_string(),
        Tab::Settings => "10".to_string(),
    }
}
