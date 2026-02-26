use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::tui::{action::Action, config::Config};

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
}

impl Default for PrepTab {
    fn default() -> Self {
        Self {
            config_path: None,
            config_loaded: false,
            config_lines: Vec::new(),
            sim_running: false,
            command_tx: None,
            config: Config::default(),
        }
    }
}

impl PrepTab {
    pub fn set_config_path(&mut self, path: Option<String>) {
        self.config_path = path.clone();
        if let Some(ref p) = path {
            self.try_load_config(p.clone());
        }
    }

    fn try_load_config(&mut self, path: String) {
        match crate::toml::read_config(&path) {
            Ok(_cfg) => {
                // TODO: store typed config for display
                self.config_loaded = true;
                self.config_lines = vec![
                    format!("[model]   type=plummer  M=1.0  a=0.1"),
                    format!("[domain]  Lx=10  Lv=5  Nx=32  Nv=32  bc=open"),
                    format!("[solver]  repr=uniform  poisson=fft  adv=semi"),
                    format!("[time]    t_final=10  dt=adaptive  cfl=0.5"),
                    format!("[exit]    |ΔE/E|<1e-4  mass_thr=0.01"),
                ];
            }
            Err(e) => {
                self.config_loaded = false;
                self.config_lines = vec![format!("Error loading config: {e}")];
            }
        }
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

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> color_eyre::Result<Option<Action>> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('r') => {
                if !self.sim_running {
                    return Ok(Some(Action::SimStart));
                }
            }
            KeyCode::Char('l') => {
                // TODO: open a file picker / load-config dialog
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
            Action::SimUpdate(ref state) => {
                if state.exit_reason.is_some() {
                    self.sim_running = false;
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let outer = Block::bordered()
            .title(" Prep ")
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let [header_area, config_area, status_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .areas(inner);

        // --- Header: config path + run hint ---
        let path_display = self
            .config_path
            .as_deref()
            .unwrap_or("<none — pass --config path/to/run.toml>");
        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Config: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(path_display),
                Span::styled(
                    "   [r] Run",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("[l] Load config   ", Style::default().fg(Color::DarkGray)),
            ]),
        ]);
        frame.render_widget(header, header_area);

        // --- Config display ---
        let config_block = Block::bordered().title(" Config preview ");
        let config_inner = config_block.inner(config_area);
        frame.render_widget(config_block, config_area);

        if self.config_lines.is_empty() {
            let hint = Paragraph::new("No config loaded. Load a TOML file with [l] or pass --config.")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(hint, config_inner);
        } else {
            let lines: Vec<Line> = self
                .config_lines
                .iter()
                .map(|l| {
                    // Highlight section headers in cyan
                    if l.starts_with('[') {
                        Line::from(Span::styled(l.clone(), Style::default().fg(Color::Cyan)))
                    } else {
                        Line::from(l.clone())
                    }
                })
                .collect();
            let para = Paragraph::new(lines);
            frame.render_widget(para, config_inner);
        }

        // --- Status bar ---
        let status_str = if self.sim_running {
            Span::styled(
                "● Running — switch to [F2 Run] to watch live diagnostics",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )
        } else if self.config_loaded {
            Span::styled("Ready — press [r] to start", Style::default().fg(Color::Yellow))
        } else {
            Span::styled("Config not loaded", Style::default().fg(Color::DarkGray))
        };
        let status = Paragraph::new(Line::from(status_str))
            .block(Block::bordered().title(" Status "));
        frame.render_widget(status, status_area);

        Ok(())
    }
}
