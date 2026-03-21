use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::tui::{action::Action, config::Config};

#[derive(Default)]
pub struct PrepTab {
    config_path: Option<String>,
    /// Whether the config has been loaded successfully.
    config_loaded: bool,
    /// Cached display lines from the parsed config.
    config_lines: Vec<String>,
    /// Whether a simulation is currently running.
    sim_running: bool,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    /// .toml files found in ./configs/
    available_configs: Vec<String>,
    /// Highlighted index in the browser list.
    browser_selected: usize,
}

impl PrepTab {
    pub fn set_config_path(&mut self, path: Option<String>) {
        self.config_path = path.clone();
        if let Some(ref p) = path {
            self.try_load_config(p.clone());
        }
    }

    fn try_load_config(&mut self, path: String) {
        // Read the raw file so we can show comments (purpose) and actual values.
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.config_loaded = false;
                self.config_lines = vec![format!("Cannot read file: {e}")];
                return;
            }
        };

        // Validate that it parses correctly before accepting it.
        match crate::toml::read_config(&path) {
            Ok(_) => {
                self.config_loaded = true;
                self.config_path = Some(path);
                // Store every non-blank line from the raw file.
                self.config_lines = raw
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.to_string())
                    .collect();
            }
            Err(e) => {
                self.config_loaded = false;
                // Still show the raw content so the user can see what's wrong.
                let mut lines: Vec<String> = raw
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.to_string())
                    .collect();
                lines.push(String::new());
                lines.push(format!("✗ Parse error: {e}"));
                self.config_lines = lines;
            }
        }
    }

    fn refresh_config_list(&mut self) {
        let dir = std::path::Path::new("configs");
        let mut entries: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().map(|x| x == "toml").unwrap_or(false) {
                    // Show just the filename (stem), store full path
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
        } else {
            self.browser_selected = 0;
        }
    }

    /// Display name for a config path (filename without directory prefix).
    fn display_name(path: &str) -> &str {
        std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
    }
}

impl Component for PrepTab {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn init(&mut self, _area: ratatui::layout::Size) -> color_eyre::Result<()> {
        self.refresh_config_list();
        // If a CLI config was provided, try to select it in the browser too
        if let Some(ref path) = self.config_path.clone()
            && let Some(idx) = self.available_configs.iter().position(|p| p == path)
        {
            self.browser_selected = idx;
        }
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> color_eyre::Result<Option<Action>> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Up => {
                self.browser_selected = self.browser_selected.saturating_sub(1);
            }
            KeyCode::Down if !self.available_configs.is_empty() => {
                self.browser_selected =
                    (self.browser_selected + 1).min(self.available_configs.len() - 1);
            }
            KeyCode::Enter => {
                if let Some(path) = self.available_configs.get(self.browser_selected).cloned() {
                    self.try_load_config(path);
                }
            }
            KeyCode::Char('l') => {
                self.refresh_config_list();
            }
            KeyCode::Char('r') if !self.sim_running && self.config_loaded => {
                return Ok(Some(Action::SimStart));
            }
            _ => {}
        }
        Ok(None)
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::SimStart => {
                self.sim_running = true;
            }
            Action::SimStop => {
                self.sim_running = false;
            }
            Action::SimUpdate(state) if state.exit_reason.is_some() => {
                self.sim_running = false;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let [main_area, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);

        let [browser_area, preview_area] =
            Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(main_area);

        // --- Config browser (left panel) ---
        let browser_block = Block::bordered().title(" Configs ");
        let browser_inner = browser_block.inner(browser_area);
        frame.render_widget(browser_block, browser_area);

        if self.available_configs.is_empty() {
            let hint = Paragraph::new("No configs found.\nPlace .toml files in\n./configs/")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(hint, browser_inner);
        } else {
            let items: Vec<ListItem> = self
                .available_configs
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    let name = Self::display_name(path);
                    // Strip .toml suffix for cleaner display
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
                        ListItem::new(Line::from(vec![
                            Span::raw("  "),
                            Span::raw(label.to_string()),
                        ]))
                    }
                })
                .collect();

            let mut list_state = ListState::default().with_selected(Some(self.browser_selected));
            let list = List::new(items);
            frame.render_stateful_widget(list, browser_inner, &mut list_state);
        }

        // --- Config preview (right panel) ---
        let preview_block = Block::bordered().title(" Config preview ");
        let preview_inner = preview_block.inner(preview_area);
        frame.render_widget(preview_block, preview_area);

        if self.config_lines.is_empty() {
            let hint = Paragraph::new(
                "Select a preset with ↑/↓ and press Enter,\nor pass --config path/to/run.toml.",
            )
            .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(hint, preview_inner);
        } else {
            let lines: Vec<Line> = self
                .config_lines
                .iter()
                .map(|l| {
                    if l.starts_with('#') {
                        // Comment lines — show in dark gray, slightly indented
                        Line::from(Span::styled(
                            l.clone(),
                            Style::default().fg(Color::DarkGray),
                        ))
                    } else if l.starts_with('[') {
                        // Section headers — cyan bold
                        Line::from(Span::styled(
                            l.clone(),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ))
                    } else if l.starts_with('✗') {
                        Line::from(Span::styled(l.clone(), Style::default().fg(Color::Red)))
                    } else {
                        Line::from(l.clone())
                    }
                })
                .collect();
            let para = Paragraph::new(lines);
            frame.render_widget(para, preview_inner);
        }

        // --- Status bar ---
        let status_line = if self.sim_running {
            Line::from(vec![
                Span::styled(
                    "● Running — ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "[F2]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " to watch live diagnostics",
                    Style::default().fg(Color::Green),
                ),
            ])
        } else if self.config_loaded {
            Line::from(vec![
                Span::styled("Ready — press ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "[r]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to start", Style::default().fg(Color::Yellow)),
            ])
        } else {
            Line::from(Span::styled(
                "Select a config above and press Enter to load",
                Style::default().fg(Color::DarkGray),
            ))
        };
        let status = Paragraph::new(status_line).block(Block::bordered().title(" Status "));
        frame.render_widget(status, status_area);

        Ok(())
    }
}
