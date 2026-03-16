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
    config::{PhasmaConfig, defaults, validate},
    themes::ThemeColors,
    tui::{action::Action, config::Config},
};

const PRESET_NAMES: &[&str] = &[
    // ── Basics ──
    "default",
    "balanced",
    "speed_priority",
    "resolution_priority",
    "conservation_priority",
    "debug",
    // ── Equilibrium models ──
    "isolated_plummer",
    "hernquist_galaxy",
    "king_equilibrium",
    "nfw_dark_matter",
    "nfw_high_res",
    // ── Advanced representations ──
    "ht_plummer",
    "tensor_train_plummer",
    "spectral_plummer",
    // ── Solver variants ──
    "yoshida_plummer",
    "multigrid_plummer",
    "tree_nfw",
    "spherical_harmonics_plummer",
    "tensor_poisson_plummer",
    "unsplit_rk4_plummer",
    "lomac_plummer",
    // ── Physics scenarios ──
    "cosmological",
    "jeans_instability",
    "jeans_stability",
    "merger",
    "merger_demo",
    "merger_unequal",
    "tidal_stream",
    "tidal_nfw_host",
    "disk_exponential",
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
        let block = Block::bordered()
            .title(" Config Summary ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if !self.config_loaded {
            frame.render_widget(
                Paragraph::new(
                    "Select a config and press Enter to load,\nor pass --config path/to/run.toml.",
                )
                .style(Style::default().fg(theme.dim)),
                inner,
            );
            return;
        }

        let cfg = &self.phasma_config;
        let section = |title: &'static str| -> Line<'static> {
            Line::from(Span::styled(
                format!("─── {title} ───"),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
        };
        let kv = |key: &'static str, val: String| -> Line<'static> {
            Line::from(vec![
                Span::styled(format!("  {key:<20} "), Style::default().fg(theme.dim)),
                Span::styled(val, Style::default().fg(theme.fg)),
            ])
        };
        let fmt_mem = |mb: f64| -> String {
            if mb >= 1000.0 {
                format!("{:.2} GB", mb / 1000.0)
            } else {
                format!("{mb:.1} MB")
            }
        };

        let breakdown = defaults::estimate_memory_breakdown(cfg);
        let mem_mb = breakdown.total_mb();
        let peak_mb = defaults::estimate_peak_memory_mb(cfg);
        let full_mb = defaults::full_grid_memory_mb(cfg);

        // §2.2: Live validation — show warnings below the form
        let warnings = validate::validate(cfg);
        let warn_height = if warnings.is_empty() {
            0
        } else {
            (warnings.len() as u16 + 1).min(5)
        };
        let [form_area, warn_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(warn_height)]).areas(inner);

        if !warnings.is_empty() {
            let warn_lines: Vec<Line> = warnings
                .iter()
                .take(4)
                .map(|w| {
                    Line::from(Span::styled(
                        format!("  ⚠ {w}"),
                        Style::default().fg(theme.warn),
                    ))
                })
                .collect();
            frame.render_widget(Paragraph::new(warn_lines), warn_area);
        }

        let inner = form_area;

        // §2.2: Two-column form — left: model + domain, right: solver + output
        // If wide enough, split into two columns; otherwise single column fallback
        if inner.width >= 60 {
            let [left_area, right_area] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .areas(inner);

            // Left column: Model + Domain
            let left_lines: Vec<Line> = vec![
                section("Model"),
                kv("Type", cfg.model.model_type.clone()),
                kv("Total mass", format!("{}", cfg.model.total_mass)),
                kv("Scale radius", format!("{}", cfg.model.scale_radius)),
                Line::from(""),
                section("Domain"),
                kv("Spatial extent", format!("{}", cfg.domain.spatial_extent)),
                kv("Velocity extent", format!("{}", cfg.domain.velocity_extent)),
                kv(
                    "Spatial resolution",
                    format!("{}", cfg.domain.spatial_resolution),
                ),
                kv(
                    "Velocity resolution",
                    format!("{}", cfg.domain.velocity_resolution),
                ),
                kv("Boundary", cfg.domain.boundary.clone()),
                Line::from(""),
                section("Exit Conditions"),
                kv(
                    "Energy drift tol.",
                    format!("{}", cfg.exit.energy_drift_tolerance),
                ),
                kv(
                    "Mass drift tol.",
                    format!("{}", cfg.exit.mass_drift_tolerance),
                ),
            ];

            frame.render_widget(Paragraph::new(left_lines), left_area);

            // Right column: Solver + Time + Output + Memory
            let mut right_lines: Vec<Line> = vec![
                section("Solver"),
                kv("Representation", cfg.solver.representation.clone()),
                kv("Poisson solver", cfg.solver.poisson.clone()),
                kv("Advection", cfg.solver.advection.clone()),
                kv("Integrator", cfg.solver.integrator.clone()),
            ];
            if cfg.solver.conservation != "none" {
                right_lines.push(kv("Conservation", cfg.solver.conservation.clone()));
            }
            if let Some(ref ht) = cfg.solver.ht {
                right_lines.push(kv("HT max rank", format!("{}", ht.max_rank)));
                right_lines.push(kv("HT tolerance", format!("{:.1e}", ht.tolerance)));
            }
            right_lines.extend([
                Line::from(""),
                section("Time"),
                kv("t_final", format!("{}", cfg.time.t_final)),
                kv("dt mode", cfg.time.dt_mode.clone()),
                kv("CFL factor", format!("{}", cfg.time.cfl_factor)),
                Line::from(""),
                section("Output"),
                kv("Directory", cfg.output.directory.clone()),
                kv("Format", cfg.output.format.clone()),
                kv(
                    "Snapshot interval",
                    format!("{}", cfg.output.snapshot_interval),
                ),
                Line::from(""),
                section("Memory Estimate"),
                kv("Resident", fmt_mem(breakdown.resident_mb())),
                kv("Peak (step)", fmt_mem(mem_mb)),
            ]);
            // Breakdown items (only show if > 0.1 MB)
            if breakdown.phase_space_mb > 0.1 {
                right_lines.push(kv("  Phase space", fmt_mem(breakdown.phase_space_mb)));
            }
            if breakdown.poisson_buffers_mb > 0.1 {
                right_lines.push(kv("  Poisson bufs", fmt_mem(breakdown.poisson_buffers_mb)));
            }
            if breakdown.workspace_mb > 0.1 {
                right_lines.push(kv("  Workspace", fmt_mem(breakdown.workspace_mb)));
            }
            if breakdown.advection_clone_mb > 0.1 {
                right_lines.push(kv("  Advect clone*", fmt_mem(breakdown.advection_clone_mb)));
            }
            if breakdown.lomac_mb > 0.1 {
                right_lines.push(kv("  LoMaC", fmt_mem(breakdown.lomac_mb)));
            }
            if breakdown.ht_recompression_mb > 0.1 {
                right_lines.push(kv(
                    "  HT recompress*",
                    fmt_mem(breakdown.ht_recompression_mb),
                ));
            }
            if (peak_mb - mem_mb).abs() > 0.01 {
                right_lines.push(kv("Peak (max rank)", fmt_mem(peak_mb)));
            }
            right_lines.push(kv("Full grid equiv.", fmt_mem(full_mb)));

            frame.render_widget(Paragraph::new(right_lines), right_area);
        } else {
            // Narrow fallback: single column
            let mut lines: Vec<Line> = Vec::new();
            lines.push(section("Model"));
            lines.push(kv("Type", cfg.model.model_type.clone()));
            lines.push(kv("Total mass", format!("{}", cfg.model.total_mass)));
            lines.push(kv("Scale radius", format!("{}", cfg.model.scale_radius)));
            lines.push(Line::from(""));
            lines.push(section("Domain"));
            lines.push(kv(
                "Spatial extent",
                format!("{}", cfg.domain.spatial_extent),
            ));
            lines.push(kv(
                "Velocity extent",
                format!("{}", cfg.domain.velocity_extent),
            ));
            lines.push(kv(
                "Spatial res.",
                format!("{}", cfg.domain.spatial_resolution),
            ));
            lines.push(kv(
                "Velocity res.",
                format!("{}", cfg.domain.velocity_resolution),
            ));
            lines.push(kv("Boundary", cfg.domain.boundary.clone()));
            lines.push(Line::from(""));
            lines.push(section("Solver"));
            lines.push(kv("Repr", cfg.solver.representation.clone()));
            lines.push(kv("Poisson", cfg.solver.poisson.clone()));
            lines.push(kv("Advection", cfg.solver.advection.clone()));
            lines.push(kv("Integrator", cfg.solver.integrator.clone()));
            lines.push(Line::from(""));
            lines.push(section("Time"));
            lines.push(kv("t_final", format!("{}", cfg.time.t_final)));
            lines.push(kv("dt mode", cfg.time.dt_mode.clone()));
            lines.push(Line::from(""));
            lines.push(section("Memory"));
            lines.push(kv("Est.", format!("{mem_mb:.1} MB")));
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}
