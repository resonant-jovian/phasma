use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    config::{PhasmaConfig, defaults},
    themes::ThemeColors,
    tui::{action::Action, config::Config},
};

pub struct SetupTab {
    config_path: Option<String>,
    phasma_config: PhasmaConfig,
    config_loaded: bool,
    config_lines: Vec<String>,
    available_configs: Vec<String>,
    browser_selected: usize,
    sim_running: bool,
    command_tx: Option<UnboundedSender<Action>>,
}

impl SetupTab {
    pub fn new(config_path: Option<String>) -> Self {
        let mut tab = Self {
            config_path: config_path.clone(),
            phasma_config: PhasmaConfig::default(),
            config_loaded: false,
            config_lines: Vec::new(),
            available_configs: Vec::new(),
            browser_selected: 0,
            sim_running: false,
            command_tx: None,
        };
        if let Some(ref p) = config_path {
            tab.try_load_config(p.clone());
        }
        tab.refresh_config_list();
        tab
    }

    pub fn register_action_handler(&mut self, tx: UnboundedSender<Action>) {
        self.command_tx = Some(tx);
    }

    pub fn register_config_handler(&mut self, _config: Config) {}

    fn try_load_config(&mut self, path: String) {
        match std::fs::read_to_string(&path) {
            Ok(raw) => {
                let parsed: Result<PhasmaConfig, _> = toml::from_str(&raw);
                match parsed {
                    Ok(cfg) => {
                        self.phasma_config = cfg;
                        self.config_loaded = true;
                        self.config_path = Some(path.clone());
                        self.config_lines = raw.lines().map(|l| l.to_string()).collect();
                    }
                    Err(_e) => {
                        // Fall back to old-format toml.rs parser for backward compat
                        if crate::toml::read_config(&path).is_ok() {
                            self.config_loaded = true;
                            self.config_path = Some(path);
                            self.config_lines = raw.lines().map(|l| l.to_string()).collect();
                        } else {
                            self.config_loaded = false;
                            self.config_lines = raw.lines().map(|l| l.to_string()).collect();
                            self.config_lines.push(format!("✗ Parse error: {_e}"));
                        }
                    }
                }
            }
            Err(e) => {
                self.config_loaded = false;
                self.config_lines = vec![format!("Cannot read file: {e}")];
            }
        }
    }

    fn refresh_config_list(&mut self) {
        let mut entries: Vec<String> = std::fs::read_dir("configs")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().map(|x| x == "toml").unwrap_or(false) {
                    Some(p.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();
        entries.sort();
        self.available_configs = entries;
        if !self.available_configs.is_empty() {
            self.browser_selected = self.browser_selected.min(self.available_configs.len() - 1);
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.available_configs.is_empty() {
                    self.browser_selected =
                        (self.browser_selected + 1) % self.available_configs.len();
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.available_configs.is_empty() {
                    self.browser_selected = (self.browser_selected + self.available_configs.len()
                        - 1)
                        % self.available_configs.len();
                }
                None
            }
            KeyCode::Enter => {
                if let Some(path) = self.available_configs.get(self.browser_selected).cloned() {
                    self.try_load_config(path.clone());
                    if self.config_loaded {
                        return Some(Action::ConfigLoaded(
                            self.config_path.clone().unwrap_or(path),
                        ));
                    }
                }
                None
            }
            KeyCode::Char('r') if self.config_loaded => Some(Action::SimStart),
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::SimStart => self.sim_running = true,
            Action::SimStop => self.sim_running = false,
            Action::SimUpdate(state) => {
                if state.exit_reason.is_some() {
                    self.sim_running = false;
                }
            }
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let mem_estimate = defaults::estimate_memory_mb(&self.phasma_config);

        let [main_area, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);

        let [browser_area, preview_area] =
            Layout::horizontal([Constraint::Length(26), Constraint::Min(0)]).areas(main_area);

        // Config browser
        self.draw_browser(frame, browser_area, theme);

        // Config preview (full right side)
        self.draw_preview(frame, preview_area, theme);

        // Status bar
        let status = if self.sim_running {
            Line::from(vec![
                Span::styled(
                    "● Running — ",
                    Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "[F2]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" live view  ", Style::default().fg(theme.ok)),
                Span::styled(
                    "[r]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" restart", Style::default().fg(theme.ok)),
            ])
        } else if self.config_loaded {
            Line::from(vec![
                Span::styled(
                    format!("Ready — {mem_estimate:.1} MB est. — press "),
                    Style::default().fg(theme.ok),
                ),
                Span::styled(
                    "[r]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to start", Style::default().fg(theme.ok)),
            ])
        } else {
            Line::from(Span::styled(
                "Select a config and press Enter to load.",
                Style::default().fg(theme.dim),
            ))
        };
        frame.render_widget(
            Paragraph::new(status).block(Block::bordered().title(" Status ")),
            status_area,
        );
    }

    fn draw_browser(&mut self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Configs ")
            .border_style(Style::default().fg(theme.accent));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.available_configs.is_empty() {
            frame.render_widget(
                Paragraph::new("No configs found.\nPlace .toml files\nin ./configs/")
                    .style(Style::default().fg(theme.dim)),
                inner,
            );
            return;
        }

        let items: Vec<ListItem> = self
            .available_configs
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path);
                let label = name.strip_suffix(".toml").unwrap_or(name);
                if i == self.browser_selected {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            "► ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            label.to_string(),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]))
                } else {
                    ListItem::new(Line::from(Span::styled(
                        format!("  {label}"),
                        Style::default().fg(theme.fg),
                    )))
                }
            })
            .collect();

        let mut state = ListState::default().with_selected(Some(self.browser_selected));
        frame.render_stateful_widget(List::new(items), inner, &mut state);
    }

    fn draw_preview(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Config preview ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.config_lines.is_empty() {
            frame.render_widget(
                Paragraph::new(
                    "Select a config and press Enter to load,\nor pass --config path/to/run.toml.",
                )
                .style(Style::default().fg(theme.dim)),
                inner,
            );
            return;
        }

        let lines: Vec<Line> = self
            .config_lines
            .iter()
            .map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with('#') {
                    Line::from(Span::styled(l.clone(), Style::default().fg(theme.dim)))
                } else if trimmed.starts_with('[') {
                    Line::from(Span::styled(
                        l.clone(),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ))
                } else if trimmed.starts_with('✗') {
                    Line::from(Span::styled(l.clone(), Style::default().fg(theme.error)))
                } else if trimmed.is_empty() {
                    Line::from("")
                } else {
                    Line::from(Span::styled(l.clone(), Style::default().fg(theme.fg)))
                }
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), inner);
    }
}
