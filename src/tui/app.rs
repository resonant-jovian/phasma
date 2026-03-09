use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Size;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    data::live::LiveDataProvider,
    export,
    notifications,
    session,
    sim::{SimControl, SimHandle},
    themes::Theme,
    tui::{
        action::Action,
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
    config_path: Option<String>,
    auto_run: bool,
    // New 9-tab system
    tab_view: TabView,
    data_provider: LiveDataProvider,
    status_bar: StatusBar,
    guard: TerminalGuard,
    theme: Theme,
    colormap: Colormap,
    help: HelpOverlay,
    export_menu: ExportMenu,
    quit_confirm: bool,
    sim_paused: bool,
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

        // Restore session state
        let saved = session::load();
        let theme = Theme::from_name(&saved.theme);
        let colormap = Colormap::from_name(&saved.colormap);

        let mut tab_view = TabView::new(config_path.clone());
        tab_view.restore_tab(saved.active_tab);

        let config_name = config_path
            .as_deref()
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or("—")
            .to_string();

        let mut status_bar = StatusBar::default();
        status_bar.set_config_name(config_name);

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
            config_path,
            auto_run,
            tab_view,
            data_provider: LiveDataProvider::default(),
            status_bar,
            guard: TerminalGuard::default(),
            theme,
            colormap,
            help: HelpOverlay::default(),
            export_menu: ExportMenu::default(),
            quit_confirm: false,
            sim_paused: false,
        })
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate)
            .mouse(true);
        tui.enter()?;

        self.tab_view.register_action_handler(self.action_tx.clone());
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
                // Save session state
                self.save_session();
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
            Event::Key(key) => self.handle_key_event(key)?,
            Event::Mouse(mouse) => self.handle_mouse_event(mouse)?,
            _ => {}
        }

        // Drain sim state updates (non-blocking)
        if let Some(ref mut handle) = self.sim_handle {
            while let Ok(state) = handle.state_rx.try_recv() {
                self.action_tx.send(Action::SimUpdate(state))?;
            }
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
            if let Some(action) = self.export_menu.handle_key_event(key) {
                if matches!(action, Action::ExportMenuClose) {
                    // Perform the export
                    let fmt = self.export_menu.selected_format();
                    let dir = std::path::PathBuf::from("./phasma_export");
                    let state = self.data_provider.current_state();
                    let result = export::export_diagnostics(
                        &dir,
                        fmt,
                        &self.data_provider.diagnostics,
                        state,
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
            }
            return Ok(());
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
                // Global pause/resume
                if self.sim_handle.is_some() {
                    if self.sim_paused {
                        action_tx.send(Action::SimResume)?;
                    } else {
                        action_tx.send(Action::SimPause)?;
                    }
                }
                return Ok(());
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
            _ => {}
        }

        // Let the tab view handle keys (F1-F9, Tab, BackTab, tab-specific)
        if let Some(action) = self.tab_view.handle_key_event(key) {
            action_tx.send(action)?;
            return Ok(());
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
                    self.render(tui)?;
                }
                Action::Render => self.render(tui)?,
                Action::ConfigLoaded(path) => {
                    self.config_path = Some(path.clone());
                    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
                    self.status_bar.set_config_name(name);
                }
                Action::SimStart => {
                    if self.sim_handle.is_none() {
                        if let Some(ref path) = self.config_path {
                            self.sim_handle = Some(SimHandle::spawn(path.clone()));
                            self.sim_paused = false;
                            self.status_bar.on_sim_start();
                        }
                    }
                }
                Action::SimStop => {
                    if let Some(ref handle) = self.sim_handle {
                        let _ = handle.control_tx.send(SimControl::Stop);
                    }
                    self.sim_handle = None;
                    self.sim_paused = false;
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
                Action::SimUpdate(state) => {
                    self.data_provider.update(state);
                    self.status_bar.on_state_update(state);
                    // Notify on sim completion
                    if let Some(ref reason) = state.exit_reason {
                        let msg = format!("Exit: {reason} at t={:.4}", state.t);
                        notifications::notify(notifications::NotificationKind::SimComplete, &msg);
                    }
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

                // Status bar
                self.status_bar.draw(frame, layout.status_area, &theme);

                // Tab bar + content + footer
                self.tab_view.draw(
                    frame,
                    layout.tab_bar_area,
                    layout.content_area,
                    layout.footer_area,
                    &theme,
                    colormap,
                    &self.data_provider,
                );

                // Overlays (rendered on top)
                self.help.draw(frame, size, &theme);
                self.export_menu.draw(frame, size, &theme);

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
            Span::styled("  [y/Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" Yes  ", Style::default().fg(theme.fg)),
            Span::styled("[n/Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" No", Style::default().fg(theme.fg)),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
