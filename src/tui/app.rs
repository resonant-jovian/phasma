use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Size;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{
    annotations::AnnotationStore,
    colormaps::Colormap,
    data::DataProvider,
    data::comparison::ComparisonDataProvider,
    data::live::LiveDataProvider,
    data::playback::PlaybackDataProvider,
    export, notifications,
    runner::monitor::MonitorHandle,
    session,
    sim::{SimControl, SimHandle},
    themes::Theme,
    tui::{
        action::Action,
        command_palette::{Command, CommandPalette},
        config::Config,
        export_menu::ExportMenu,
        guard::TerminalGuard,
        help::HelpOverlay,
        layout::ResponsiveLayout,
        status_bar::StatusBar,
        tabs::TabView,
        {Event, Tui},
    },
};

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    should_quit: bool,
    should_suspend: bool,
    mode: Mode,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    sim_handle: Option<SimHandle>,
    monitor_handle: Option<MonitorHandle>,
    config_path: Option<String>,
    auto_run: bool,
    tab_view: TabView,
    data_provider: LiveDataProvider,
    /// Alternative data provider for playback/comparison modes (overrides data_provider for rendering).
    alt_provider: AltProvider,
    status_bar: StatusBar,
    guard: TerminalGuard,
    theme: Theme,
    colormap: Colormap,
    help: HelpOverlay,
    export_menu: ExportMenu,
    quit_confirm: bool,
    sim_paused: bool,
    command_palette: CommandPalette,
    annotations: AnnotationStore,
    needs_redraw: bool,
}

/// Alternative data provider modes.
pub enum AltProvider {
    None,
    Playback(Box<PlaybackDataProvider>),
    Comparison(Box<ComparisonDataProvider>),
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
}

impl App {
    pub fn new(
        tick_rate: f64,
        frame_rate: f64,
        config_path: Option<String>,
        auto_run: bool,
    ) -> color_eyre::Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();

        // Load TOML config to extract appearance settings
        let phasma_config = config_path
            .as_deref()
            .and_then(|p| crate::config::load(p).ok());

        // Restore session state (session overrides defaults but TOML appearance overrides session)
        let saved = session::load();
        let (theme, colormap, guard) = if let Some(ref cfg) = phasma_config {
            let app = &cfg.appearance;
            let t = if app.theme != "dark" {
                Theme::from_name(&app.theme)
            } else {
                Theme::from_name(&saved.theme)
            };
            let c = if app.colormap_default != "viridis" {
                Colormap::from_name(&app.colormap_default)
            } else {
                Colormap::from_name(&saved.colormap)
            };
            let g = TerminalGuard::new(app.min_columns, app.min_rows);
            (t, c, g)
        } else {
            (
                Theme::from_name(&saved.theme),
                Colormap::from_name(&saved.colormap),
                TerminalGuard::default(),
            )
        };

        let mut tab_view = TabView::new(config_path.clone());
        tab_view.restore_tab(0); // Always start on F1 Setup

        let config_name = config_path
            .as_deref()
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or("—")
            .to_string();

        let mut status_bar = StatusBar::default();
        status_bar.set_config_name(config_name);

        let mut data_provider = LiveDataProvider::default();
        if let Some(cfg) = phasma_config {
            data_provider.set_config(cfg);
        }

        Ok(Self {
            tick_rate,
            frame_rate,
            should_quit: false,
            should_suspend: false,
            config: Config::new()?,
            mode: Mode::Home,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            sim_handle: None,
            monitor_handle: None,
            config_path,
            auto_run,
            tab_view,
            data_provider,
            alt_provider: AltProvider::None,
            status_bar,
            guard,
            theme,
            colormap,
            help: HelpOverlay::default(),
            export_menu: ExportMenu::default(),
            quit_confirm: false,
            sim_paused: false,
            command_palette: CommandPalette::default(),
            annotations: AnnotationStore::new(),
            needs_redraw: true,
        })
    }

    /// Create an App in playback mode.
    pub fn new_with_playback(
        tick_rate: f64,
        frame_rate: f64,
        provider: PlaybackDataProvider,
    ) -> color_eyre::Result<Self> {
        let mut app = Self::new(tick_rate, frame_rate, None, false)?;
        app.alt_provider = AltProvider::Playback(Box::new(provider));
        app.tab_view.restore_tab(2); // Start on Density tab
        Ok(app)
    }

    /// Create an App in comparison mode.
    pub fn new_with_comparison(
        tick_rate: f64,
        frame_rate: f64,
        provider: ComparisonDataProvider,
    ) -> color_eyre::Result<Self> {
        let mut app = Self::new(tick_rate, frame_rate, None, false)?;
        app.alt_provider = AltProvider::Comparison(Box::new(provider));
        app.tab_view.restore_tab(2); // Start on Density tab
        Ok(app)
    }

    /// Set a monitor handle for --monitor / --tail modes.
    pub fn set_monitor_handle(&mut self, handle: MonitorHandle) {
        self.monitor_handle = Some(handle);
    }

    /// Get the active data provider (alt if set, otherwise live).
    fn active_provider(&self) -> &dyn DataProvider {
        match &self.alt_provider {
            AltProvider::Playback(p) => &**p,
            AltProvider::Comparison(p) => &**p,
            AltProvider::None => &self.data_provider,
        }
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate)
            .mouse(true);
        tui.enter()?;

        self.tab_view
            .register_action_handler(self.action_tx.clone());
        self.tab_view.register_config_handler(self.config.clone());

        // Auto-start sim if --run was passed
        if self.auto_run {
            self.action_tx.send(Action::SimStart)?;
        }

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui)?;
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                tui.enter()?;
            } else if self.should_quit {
                if let Some(ref handle) = self.sim_handle {
                    let _ = handle.control_tx.send(SimControl::Stop);
                }
                // Save session state and annotations
                self.save_session();
                let _ = self.annotations.save();
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
            Event::Key(key) => {
                self.handle_key_event(key)?;
                self.needs_redraw = true;
            }
            Event::Mouse(mouse) => {
                self.handle_mouse_event(mouse)?;
                self.needs_redraw = true;
            }
            _ => {}
        }

        // Drain sim state updates to latest (non-blocking)
        if let Some(ref mut handle) = self.sim_handle {
            let mut latest = None;
            while let Ok(state) = handle.state_rx.try_recv() {
                latest = Some(state);
            }
            if let Some(state) = latest {
                self.action_tx.send(Action::SimUpdate(state))?;
            }
        }

        // Drain monitor handle updates to latest (non-blocking)
        if let Some(ref mut handle) = self.monitor_handle {
            let mut latest = None;
            while let Ok(state) = handle.state_rx.try_recv() {
                latest = Some(state);
            }
            if let Some(state) = latest {
                self.action_tx.send(Action::SimUpdate(state))?;
            }
        }

        // Tick playback provider
        if let AltProvider::Playback(ref mut p) = self.alt_provider {
            p.tick();
        }
        if let AltProvider::Comparison(ref mut p) = self.alt_provider {
            p.tick();
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<()> {
        use crossterm::event::KeyCode;
        let action_tx = self.action_tx.clone();

        // Quit confirmation dialog intercepts all keys when visible
        if self.quit_confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.quit_confirm = false;
                    action_tx.send(Action::Quit)?;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.quit_confirm = false;
                }
                _ => {}
            }
            return Ok(());
        }

        // Help overlay intercepts all keys when visible
        if self.help.visible {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.help.toggle(),
                KeyCode::Down | KeyCode::Char('j') => self.help.scroll_down(),
                KeyCode::Up | KeyCode::Char('k') => self.help.scroll_up(),
                _ => {}
            }
            return Ok(());
        }

        // Export menu intercepts all keys when visible
        if self.export_menu.visible {
            if let Some(action) = self.export_menu.handle_key_event(key)
                && matches!(action, Action::ExportMenuClose)
            {
                // Perform the export
                let fmt = self.export_menu.selected_format();
                let stem = self.export_stem();
                let dir = std::path::PathBuf::from(format!("./{stem}"));
                let provider = self.active_provider();
                let state = provider.current_state();
                let cfg = provider.config();
                let result = export::export_diagnostics(
                    &dir,
                    fmt,
                    provider.diagnostics(),
                    state,
                    cfg,
                    &stem,
                );
                match &result {
                    Ok(path) => {
                        notifications::notify(
                            notifications::NotificationKind::ExportComplete,
                            &format!("Exported {} to {path}", fmt.name()),
                        );
                    }
                    Err(e) => {
                        notifications::notify(
                            notifications::NotificationKind::SimError,
                            &format!("Export failed: {e}"),
                        );
                    }
                }
                self.export_menu.last_result = Some(result);
            }
            return Ok(());
        }

        // Command palette intercepts all keys when visible
        if self.command_palette.visible {
            match self.command_palette.handle_key(key) {
                Ok(Some(cmd)) => {
                    self.execute_command(cmd)?;
                    return Ok(());
                }
                Ok(None) => return Ok(()),
                Err(()) => return Ok(()), // Closed without executing
            }
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') => {
                // Show confirmation if sim is running
                if self.sim_handle.is_some() && self.status_bar.is_sim_active() {
                    self.quit_confirm = true;
                } else {
                    action_tx.send(Action::Quit)?;
                }
                return Ok(());
            }
            KeyCode::Char(' ') => {
                // Global pause/resume — works for sim and playback
                if let AltProvider::Playback(ref mut p) = self.alt_provider {
                    p.toggle_play();
                } else if self.sim_handle.is_some() {
                    if self.sim_paused {
                        action_tx.send(Action::SimResume)?;
                    } else {
                        action_tx.send(Action::SimPause)?;
                    }
                }
                return Ok(());
            }
            KeyCode::Char('c') => {
                // Comparison view cycle
                if let AltProvider::Comparison(ref mut p) = self.alt_provider {
                    p.cycle_view();
                    return Ok(());
                }
            }
            KeyCode::Char('?') => {
                self.help.toggle();
                return Ok(());
            }
            KeyCode::Char('e') => {
                self.export_menu.toggle();
                return Ok(());
            }
            KeyCode::Char('T') => {
                action_tx.send(Action::ThemeCycle)?;
                return Ok(());
            }
            KeyCode::Char('C') => {
                action_tx.send(Action::VizCycleColormap)?;
                return Ok(());
            }
            KeyCode::Char(':') => {
                self.command_palette.open();
                return Ok(());
            }
            KeyCode::Char('a') => {
                // Add bookmark at current simulation time
                if let Some(state) = self.active_provider().current_state() {
                    let label = format!("t={:.4}", state.t);
                    self.annotations.add(state.t, label);
                }
                return Ok(());
            }
            _ => {}
        }

        // Ctrl+B: navigate to next bookmark
        if let crossterm::event::KeyCode::Char('b') = key.code
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            if let Some(state) = self.active_provider().current_state() {
                let current_t = state.t;
                if let Some(ann) = self.annotations.next_after(current_t) {
                    let target_t = ann.time;
                    // Scrub to the annotation time
                    self.data_provider.scrub_to_time(target_t);
                    self.status_bar
                        .set_scrub_position(self.data_provider.scrub_position());
                }
            }
            return Ok(());
        }

        // Ctrl+S: save config to file (§2.3)
        if let crossterm::event::KeyCode::Char('s') = key.code
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            if let Some(ref path) = self.config_path
                && let Some(cfg) = self.active_provider().config()
            {
                match crate::config::save(path, cfg) {
                    Ok(()) => {
                        action_tx.send(Action::StatusMsg(format!("Config saved to {path}")))?;
                    }
                    Err(e) => {
                        action_tx.send(Action::StatusMsg(format!("Save failed: {e}")))?;
                    }
                }
            }
            return Ok(());
        }

        // Ctrl+O: load config / switch to Setup tab (§2.3)
        if let crossterm::event::KeyCode::Char('o') = key.code
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            action_tx.send(Action::SelectTab(0))?; // Jump to Setup tab
            return Ok(());
        }

        // Ctrl+P: preset selection (§2.2) — only on Setup tab
        if let crossterm::event::KeyCode::Char('p') = key.code
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            if self.tab_view.selected == crate::tui::tabs::Tab::Setup {
                self.tab_view.setup_toggle_presets();
            }
            return Ok(());
        }

        // Ctrl+D: reset to defaults (§2.2) — only on Setup tab
        if let crossterm::event::KeyCode::Char('d') = key.code
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            if self.tab_view.selected == crate::tui::tabs::Tab::Setup {
                self.tab_view.setup_reset_defaults();
            }
            return Ok(());
        }

        // `/`: jump-to-time shortcut — opens command palette with "jump " pre-filled
        if let crossterm::event::KeyCode::Char('/') = key.code {
            self.command_palette.open_with("jump ");
            return Ok(());
        }

        // Let the tab view handle keys (F1-F9, Tab, BackTab, tab-specific)
        if let Some(action) = self.tab_view.handle_key_event(key) {
            action_tx.send(action)?;
            return Ok(());
        }

        // Scrubbing / playback keys (§2.3, §3.2)
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                action_tx.send(Action::ScrubBackward)?;
                return Ok(());
            }
            KeyCode::Right | KeyCode::Char(']') => {
                action_tx.send(Action::ScrubForward)?;
                return Ok(());
            }
            KeyCode::Char('{') => {
                action_tx.send(Action::ScrubJumpBackward)?;
                return Ok(());
            }
            KeyCode::Char('}') => {
                action_tx.send(Action::ScrubJumpForward)?;
                return Ok(());
            }
            KeyCode::Home => {
                action_tx.send(Action::ScrubToStart)?;
                return Ok(());
            }
            KeyCode::End => {
                action_tx.send(Action::ScrubToEnd)?;
                return Ok(());
            }
            KeyCode::Backspace => {
                action_tx.send(Action::ScrubToLive)?;
                return Ok(());
            }
            // Playback speed control (§3.2)
            KeyCode::Char('<') => {
                if let AltProvider::Playback(ref mut p) = self.alt_provider {
                    p.decrease_speed();
                    let _ = action_tx.send(Action::StatusMsg(format!(
                        "Playback speed: {:.1} fps",
                        p.fps()
                    )));
                }
                return Ok(());
            }
            KeyCode::Char('>') => {
                if let AltProvider::Playback(ref mut p) = self.alt_provider {
                    p.increase_speed();
                    let _ = action_tx.send(Action::StatusMsg(format!(
                        "Playback speed: {:.1} fps",
                        p.fps()
                    )));
                }
                return Ok(());
            }
            // Fine-step scrub (§3.2) — `,`/`.` act as single-frame step in playback mode
            KeyCode::Char(',') => {
                if let AltProvider::Playback(ref mut p) = self.alt_provider {
                    p.step_backward();
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                return Ok(());
            }
            KeyCode::Char('.') => {
                if let AltProvider::Playback(ref mut p) = self.alt_provider {
                    p.step_forward();
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                return Ok(());
            }
            _ => {}
        }

        // Then check keybinding config
        let Some(keymap) = self.config.keybindings.0.get(&self.mode) else {
            return Ok(());
        };
        match keymap.get(&vec![key]) {
            Some(action) => {
                info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
            }
            _ => {
                self.last_tick_key_events.push(key);
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                }
            }
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> color_eyre::Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                // Zoom in on density/phase space tabs
                self.tab_view.handle_scroll(-1);
            }
            MouseEventKind::ScrollDown => {
                self.tab_view.handle_scroll(1);
            }
            MouseEventKind::Moved => {
                self.tab_view.handle_mouse_move(mouse.column, mouse.row);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_actions(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                match &action {
                    Action::SimUpdate(_) => debug!("SimUpdate(...)"),
                    _ => debug!("{action:?}"),
                }
            }
            match &action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => {
                    tui.resize(ratatui::prelude::Rect::new(0, 0, *w, *h))?;
                    self.needs_redraw = true;
                }
                Action::Render => {
                    // Force continuous redraws while sim is running (progress panel polls atomics)
                    if self.sim_handle.is_some() {
                        self.needs_redraw = true;
                    }
                    if self.needs_redraw {
                        self.render(tui)?;
                        self.needs_redraw = false;
                    }
                }
                Action::ConfigLoaded(path) => {
                    self.config_path = Some(path.clone());
                    let name = path.rsplit('/').next().unwrap_or(path).to_string();
                    self.status_bar.set_config_name(name);
                    // Load config into data provider for analytic overlays etc.
                    if let Ok(cfg) = crate::config::load(path) {
                        self.data_provider.set_config(cfg);
                    }
                }
                Action::SimStart => {
                    // Stop any existing sim first
                    if let Some(ref handle) = self.sim_handle {
                        let _ = handle.control_tx.send(SimControl::Stop);
                        self.sim_handle = None;
                    }
                    // Clear all diagnostics/history from previous run
                    self.data_provider.reset();
                    if let Some(ref path) = self.config_path {
                        self.sim_handle = Some(SimHandle::spawn(path.clone()));
                        if let Some(ref handle) = self.sim_handle {
                            self.tab_view.set_step_progress(handle.progress.clone());
                        }
                        self.sim_paused = false;
                        self.status_bar.on_sim_start();
                    }
                }
                Action::SimStop => {
                    if let Some(ref handle) = self.sim_handle {
                        let _ = handle.control_tx.send(SimControl::Stop);
                    }
                    self.sim_handle = None;
                    self.sim_paused = false;
                    self.tab_view.clear_step_progress();
                    self.status_bar.on_sim_stop();
                }
                Action::SimPause => {
                    if let Some(ref handle) = self.sim_handle {
                        let _ = handle.control_tx.send(SimControl::Pause);
                    }
                    self.sim_paused = true;
                    self.status_bar.on_sim_pause();
                }
                Action::SimResume => {
                    if let Some(ref handle) = self.sim_handle {
                        let _ = handle.control_tx.send(SimControl::Resume);
                    }
                    self.sim_paused = false;
                    self.status_bar.on_sim_resume();
                }
                Action::SimRestart => {
                    // Stop current sim if any, then start fresh
                    if let Some(ref handle) = self.sim_handle {
                        let _ = handle.control_tx.send(SimControl::Stop);
                    }
                    self.sim_handle = None;
                    self.sim_paused = false;
                    self.status_bar.on_sim_stop();
                    // Clear all diagnostics/history from previous run
                    self.data_provider.reset();
                    // Now start
                    if let Some(ref path) = self.config_path {
                        self.sim_handle = Some(SimHandle::spawn(path.clone()));
                        if let Some(ref handle) = self.sim_handle {
                            self.tab_view.set_step_progress(handle.progress.clone());
                        }
                        self.sim_paused = false;
                        self.status_bar.on_sim_start();
                    }
                }
                Action::SimUpdate(state) => {
                    self.data_provider.update(state);
                    self.status_bar.on_state_update(state);
                    self.needs_redraw = true;
                    // Notify on sim completion and clear handle so re-run is possible
                    if let Some(ref reason) = state.exit_reason {
                        let msg = format!("Exit: {reason} at t={:.4}", state.t);
                        notifications::notify(notifications::NotificationKind::SimComplete, &msg);
                        self.sim_handle = None;
                    }
                }
                Action::ScrubBackward => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_backward(),
                        AltProvider::Comparison(p) => p.scrub_backward(),
                        AltProvider::None => self.data_provider.scrub_backward(),
                    }
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                Action::ScrubForward => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_forward(),
                        AltProvider::Comparison(p) => p.scrub_forward(),
                        AltProvider::None => self.data_provider.scrub_forward(),
                    }
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                Action::ScrubJumpBackward => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_jump_backward(10),
                        AltProvider::Comparison(p) => p.scrub_jump_backward(10),
                        AltProvider::None => self.data_provider.scrub_jump_backward(10),
                    }
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                Action::ScrubJumpForward => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_jump_forward(10),
                        AltProvider::Comparison(p) => p.scrub_jump_forward(10),
                        AltProvider::None => self.data_provider.scrub_jump_forward(10),
                    }
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                Action::ScrubToStart => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_to_start(),
                        AltProvider::Comparison(p) => p.scrub_to_start(),
                        AltProvider::None => self.data_provider.scrub_to_start(),
                    }
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                Action::ScrubToEnd => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_to_end(),
                        AltProvider::Comparison(p) => p.scrub_to_end(),
                        AltProvider::None => self.data_provider.scrub_to_end(),
                    }
                    self.status_bar
                        .set_scrub_position(self.active_provider().scrub_position());
                }
                Action::ScrubToLive => {
                    match &mut self.alt_provider {
                        AltProvider::Playback(p) => p.scrub_to_live(),
                        AltProvider::Comparison(p) => p.scrub_to_live(),
                        AltProvider::None => self.data_provider.scrub_to_live(),
                    }
                    self.status_bar.set_scrub_position(None);
                }
                Action::VizCycleColormap => {
                    // If the settings tab triggered it, use its value; otherwise cycle.
                    if self.tab_view.selected == crate::tui::tabs::Tab::Settings {
                        self.colormap = self.tab_view.settings_colormap();
                    } else {
                        self.colormap = self.colormap.next();
                    }
                }
                Action::ThemeCycle => {
                    if self.tab_view.selected == crate::tui::tabs::Tab::Settings {
                        self.theme = self.tab_view.settings_theme();
                    } else {
                        self.theme = self.theme.next();
                    }
                }
                _ => {}
            }
            // Forward to tab view
            self.tab_view.update(&action);
        }
        Ok(())
    }

    fn export_stem(&self) -> String {
        let config_name = self
            .config_path
            .as_deref()
            .and_then(|p| p.rsplit('/').next())
            .and_then(|f| f.strip_suffix(".toml"))
            .unwrap_or("phasma");
        let now = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
        format!("{config_name}_{now}")
    }

    fn execute_command(&mut self, cmd: Command) -> color_eyre::Result<()> {
        let action_tx = self.action_tx.clone();
        match cmd {
            Command::Quit => {
                action_tx.send(Action::Quit)?;
            }
            Command::JumpToTime(t) => {
                // Scrub-to-time: find nearest snapshot in history
                match &mut self.alt_provider {
                    AltProvider::Playback(p) => {
                        p.scrub_to_time(t);
                    }
                    _ => {
                        self.data_provider.scrub_to_time(t);
                    }
                }
                self.status_bar
                    .set_scrub_position(self.active_provider().scrub_position());
            }
            Command::Export(fmt) => {
                let stem = self.export_stem();
                let dir = std::path::PathBuf::from(format!("./{stem}"));
                let provider = self.active_provider();
                let state = provider.current_state();
                let format = crate::export::ExportFormat::from_name(&fmt);
                let cfg = provider.config();
                let _ = export::export_diagnostics(
                    &dir,
                    format,
                    provider.diagnostics(),
                    state,
                    cfg,
                    &stem,
                );
            }
            Command::SetColormap(name) => {
                self.colormap = Colormap::from_name(&name);
            }
            Command::SetTheme(name) => {
                self.theme = Theme::from_name(&name);
            }
        }
        Ok(())
    }

    fn save_session(&self) {
        let s = session::Session {
            config_path: self.config_path.clone(),
            active_tab: self.tab_view.selected as usize,
            colormap: self.colormap.name().to_string(),
            projection_axis: 2,
            theme: self.theme.name().to_string(),
        };
        session::save(&s);
    }

    fn render(&mut self, tui: &mut Tui) -> color_eyre::Result<()> {
        // Pre-sync settings outside the draw closure
        self.tab_view.sync_settings(self.theme, self.colormap);
        let theme = self.theme.colors();
        let colormap = self.colormap;
        let quit_confirm = self.quit_confirm;

        tui.draw(|frame| {
            // Wrap all rendering in catch_unwind for crash safety
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let size = frame.area();

                // Terminal too small guard
                if self.guard.too_small(Size::new(size.width, size.height)) {
                    self.guard.draw(frame, size);
                    return;
                }

                let layout = ResponsiveLayout::compute(Size::new(size.width, size.height));

                // Status bar — refresh RSS periodically
                self.status_bar.tick_rss();
                self.status_bar.draw(frame, layout.status_area, &theme);

                // Tab bar + content + footer
                // Split borrows to satisfy the borrow checker
                let provider: &dyn DataProvider = match &self.alt_provider {
                    AltProvider::Playback(p) => &**p,
                    AltProvider::Comparison(p) => &**p,
                    AltProvider::None => &self.data_provider,
                };
                self.tab_view.draw(
                    frame,
                    crate::tui::tabs::TabAreas {
                        tab_bar: layout.tab_bar_area,
                        content: layout.content_area,
                        footer: layout.footer_area,
                        layout_mode: layout.mode,
                    },
                    &theme,
                    colormap,
                    provider,
                );

                // Overlays (rendered on top)
                self.help.draw(frame, size, &theme);
                self.export_menu.draw(frame, size, &theme);

                // Command palette (rendered on top of everything)
                self.command_palette.draw(frame, size, &theme);

                // Quit confirmation dialog (topmost overlay)
                if quit_confirm {
                    draw_quit_confirm(frame, size, &theme);
                }
            }));
        })?;
        Ok(())
    }
}

fn draw_quit_confirm(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    theme: &crate::themes::ThemeColors,
) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Clear, Paragraph},
    };

    let w = area.width.min(44);
    let h = area.height.min(7);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let overlay = ratatui::layout::Rect::new(x, y, w, h);

    frame.render_widget(Clear, overlay);
    let block = Block::bordered()
        .title(" Quit? ")
        .border_style(Style::default().fg(theme.warn))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Simulation is running. Quit anyway?",
            Style::default().fg(theme.fg),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [y/Enter]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Yes  ", Style::default().fg(theme.fg)),
            Span::styled(
                "[n/Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" No", Style::default().fg(theme.fg)),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
