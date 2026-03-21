use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    config::{PhasmaConfig, defaults, validate},
    themes::ThemeColors,
    tui::{action::Action, config::Config},
};

const PRESET_NAMES: &[&str] = &[
    // ── Basics ──
    "plummer",
    "debug",
    "plummer_hires",
    // ── Equilibrium models ──
    "hernquist",
    "king",
    "nfw",
    // ── Advanced representations ──
    "plummer_ht",
    "plummer_tt",
    "plummer_spectral",
    // ── Integrator variants ──
    "plummer_yoshida",
    "plummer_unsplit",
    "plummer_lomac",
    // ── Solver variants ──
    "plummer_multigrid",
    "plummer_spherical",
    "plummer_tensor_poisson",
    "nfw_tree",
    // ── Physics scenarios ──
    "zeldovich",
    "disk_bar",
    "jeans_unstable",
    "jeans_stable",
    "merger_equal",
    "merger_unequal",
    "tidal_point",
    "tidal_nfw",
];

pub struct SetupTab {
    config_path: Option<String>,
    phasma_config: PhasmaConfig,
    config_loaded: bool,
    config_lines: Vec<String>,
    available_configs: Vec<String>,
    browser_selected: usize,
    sim_running: bool,
    command_tx: Option<UnboundedSender<Action>>,
    /// Preset selection popup state.
    preset_popup: bool,
    preset_selected: usize,
    toml_scroll: u16,
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
            preset_popup: false,
            preset_selected: 0,
            toml_scroll: 0,
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
        self.toml_scroll = 0;
        match std::fs::read_to_string(&path) {
            Ok(raw) => {
                let parsed: Result<PhasmaConfig, _> = toml::from_str(&raw);
                match parsed {
                    Ok(mut cfg) => {
                        crate::config::defaults::apply_smart_defaults(&mut cfg);
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

    /// Toggle the preset selection popup.
    pub fn toggle_preset_popup(&mut self) {
        self.preset_popup = !self.preset_popup;
        self.preset_selected = 0;
    }

    /// Reset config to defaults.
    pub fn reset_to_defaults(&mut self) {
        self.phasma_config = PhasmaConfig::default();
        self.config_loaded = true;
        self.config_lines = vec!["# Default configuration".to_string()];
        self.toml_scroll = 0;
    }

    /// Load a built-in preset by name from configs/ directory.
    fn load_preset(&mut self, name: &str) {
        let path = format!("configs/{name}.toml");
        self.try_load_config(path);
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        // Preset popup intercepts keys when visible
        if self.preset_popup {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.preset_selected = (self.preset_selected + 1) % PRESET_NAMES.len();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.preset_selected =
                        (self.preset_selected + PRESET_NAMES.len() - 1) % PRESET_NAMES.len();
                }
                KeyCode::Enter => {
                    let name = PRESET_NAMES[self.preset_selected];
                    self.load_preset(name);
                    self.preset_popup = false;
                    if self.config_loaded {
                        return Some(Action::ConfigLoaded(
                            self.config_path.clone().unwrap_or_default(),
                        ));
                    }
                }
                KeyCode::Esc => {
                    self.preset_popup = false;
                }
                _ => {}
            }
            return None;
        }

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
            KeyCode::Char('[') => {
                self.toml_scroll = self.toml_scroll.saturating_sub(1);
                None
            }
            KeyCode::Char(']') => {
                let max = self.config_lines.len().saturating_sub(1) as u16;
                self.toml_scroll = self.toml_scroll.saturating_add(1).min(max);
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::SimStart => self.sim_running = true,
            Action::SimStop => self.sim_running = false,
            Action::SimUpdate(state) if state.exit_reason.is_some() => {
                self.sim_running = false;
            }
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let mem_breakdown = defaults::estimate_memory_breakdown(&self.phasma_config);
        let mem_estimate = mem_breakdown.resident_mb();

        let [main_area, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);

        let [browser_area, preview_area] =
            Layout::horizontal([Constraint::Length(32), Constraint::Min(0)]).areas(main_area);

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

        // Preset selection popup overlay
        if self.preset_popup {
            self.draw_preset_popup(frame, area, theme);
        }
    }

    fn draw_preset_popup(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        use ratatui::widgets::Clear;

        let w = area.width.min(34);
        let h = area.height.min((PRESET_NAMES.len() as u16) + 3);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let overlay = Rect::new(x, y, w, h);

        frame.render_widget(Clear, overlay);
        let block = Block::bordered()
            .title(" Presets [Ctrl+P] ")
            .border_style(Style::default().fg(theme.accent))
            .style(Style::default().bg(theme.bg));
        let inner = block.inner(overlay);
        frame.render_widget(block, overlay);

        let items: Vec<ListItem> = PRESET_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if i == self.preset_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.fg)
                };
                let marker = if i == self.preset_selected {
                    "\u{25b6} "
                } else {
                    "  "
                };
                ListItem::new(format!("{marker}{name}")).style(style)
            })
            .collect();

        frame.render_widget(List::new(items), inner);
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
        if !self.config_loaded {
            let block = Block::bordered()
                .title(" Config ")
                .border_style(Style::default().fg(theme.border));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(
                    "Select a config and press Enter to load,\nor pass --config path/to/run.toml.",
                )
                .style(Style::default().fg(theme.dim)),
                inner,
            );
            return;
        }

        // Validation warnings at bottom
        let warnings = validate::validate(&self.phasma_config);
        let warn_height = if warnings.is_empty() {
            0
        } else {
            (warnings.len() as u16 + 1).min(5)
        };
        let [main_area, warn_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(warn_height)]).areas(area);

        if !warnings.is_empty() {
            let warn_lines: Vec<Line> = warnings
                .iter()
                .take(4)
                .map(|w| {
                    Line::from(Span::styled(
                        format!("  \u{26a0} {w}"),
                        Style::default().fg(theme.warn),
                    ))
                })
                .collect();
            frame.render_widget(Paragraph::new(warn_lines), warn_area);
        }

        // Two-column split: TOML source | Notes
        let [toml_area, notes_area] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(main_area);

        self.draw_toml_source(frame, toml_area, theme);
        self.draw_comment_notes(frame, notes_area, theme);
    }

    fn draw_toml_source(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let total = self.config_lines.len();
        let scroll_info = if total > 0 {
            format!(
                " TOML [{}/{}] ",
                (self.toml_scroll as usize + 1).min(total),
                total
            )
        } else {
            " TOML ".to_string()
        };
        let block = Block::bordered()
            .title(scroll_info)
            .title_bottom(" [/] scroll ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = self
            .config_lines
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.trim().starts_with('#'))
            .map(|(i, line)| {
                let num = Span::styled(format!("{:>3} ", i + 1), Style::default().fg(theme.dim));
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    Line::from(vec![
                        num,
                        Span::styled(
                            line.to_string(),
                            Style::default()
                                .fg(theme.accent)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ])
                } else if let Some(eq_idx) = line.find(" = ") {
                    Line::from(vec![
                        num,
                        Span::styled(line[..eq_idx].to_string(), Style::default().fg(theme.fg)),
                        Span::styled(" = ".to_string(), Style::default().fg(theme.dim)),
                        Span::styled(
                            line[eq_idx + 3..].to_string(),
                            Style::default().fg(Color::Cyan),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        num,
                        Span::styled(line.to_string(), Style::default().fg(theme.fg)),
                    ])
                }
            })
            .collect();

        frame.render_widget(Paragraph::new(lines).scroll((self.toml_scroll, 0)), inner);
    }

    fn draw_comment_notes(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Notes ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();
        let mut first_comment = true;
        let mut last_was_section = false;

        for raw_line in &self.config_lines {
            let trimmed = raw_line.trim();

            if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
                last_was_section = true;
                continue;
            }

            if !trimmed.starts_with('#') {
                last_was_section = false;
                continue;
            }

            // Strip leading '#' and optional space
            let text = trimmed
                .trim_start_matches('#')
                .strip_prefix(' ')
                .unwrap_or(trimmed.trim_start_matches('#'));

            // If this comment follows a [section] header, show a section divider
            if last_was_section {
                lines.push(Line::from(""));
            }
            last_was_section = false;

            if first_comment && !text.is_empty() {
                // Title line — first non-empty comment
                lines.push(Line::from(Span::styled(
                    text.to_string(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )));
                first_comment = false;
            } else if text.is_empty() {
                // Skip blank comment lines before the title
                if first_comment {
                    continue;
                }
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(Span::styled(
                    text.to_string(),
                    Style::default().fg(theme.fg),
                )));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No comments in this config.",
                Style::default().fg(theme.dim),
            )));
        }

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }
}
