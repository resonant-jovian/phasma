use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, Gauge, GraphType, Paragraph},
};
use std::collections::VecDeque;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    colormaps::Colormap,
    data::DataProvider,
    data::live::LiveDataProvider,
    sim::SimState,
    themes::ThemeColors,
    tui::{
        action::Action,
        config::Config,
        widgets::{
            heatmap::HeatmapWidget,
            sparkline_table::{SparklineRow, SparklineTable},
        },
    },
};

const MAX_ENERGY_HISTORY: usize = 500;

pub struct RunControlTab {
    sim_state: Option<SimState>,
    initial_energy: f64,
    initial_mass: f64,
    initial_c2: f64,
    energy_history: Vec<(f64, f64)>,
    log_stream: VecDeque<(Level, String)>,
    log_filter: LogFilter,
    paused: bool,
    command_tx: Option<UnboundedSender<Action>>,
    sim_start_time: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFilter {
    All,
    WarnPlus,
    ErrorOnly,
}

impl Default for RunControlTab {
    fn default() -> Self {
        Self {
            sim_state: None,
            initial_energy: 0.0,
            initial_mass: 0.0,
            initial_c2: 0.0,
            energy_history: Vec::new(),
            log_stream: VecDeque::with_capacity(200),
            log_filter: LogFilter::All,
            paused: false,
            command_tx: None,
            sim_start_time: None,
        }
    }
}

impl RunControlTab {
    pub fn register_action_handler(&mut self, tx: UnboundedSender<Action>) {
        self.command_tx = Some(tx);
    }

    pub fn register_config_handler(&mut self, _config: Config) {}

    fn push_log(&mut self, level: Level, msg: impl Into<String>) {
        if self.log_stream.len() >= 200 {
            self.log_stream.pop_front();
        }
        self.log_stream.push_back((level, msg.into()));
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('p') | KeyCode::Char(' ') => Some(if self.paused {
                Action::SimResume
            } else {
                Action::SimPause
            }),
            KeyCode::Char('s') => Some(Action::SimStop),
            KeyCode::Char('r') => Some(Action::SimRestart),
            KeyCode::Char('1') => {
                self.log_filter = LogFilter::All;
                None
            }
            KeyCode::Char('2') => {
                self.log_filter = LogFilter::WarnPlus;
                None
            }
            KeyCode::Char('3') => {
                self.log_filter = LogFilter::ErrorOnly;
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::SimUpdate(state) => {
                // Track initial values
                if self.initial_energy == 0.0 && state.total_energy != 0.0 {
                    self.initial_energy = state.total_energy;
                    self.initial_mass = state.total_mass;
                    self.initial_c2 = state.casimir_c2;
                    self.push_log(
                        Level::Info,
                        format!(
                            "Sim started: E₀={:.3e}, M₀={:.4}",
                            state.total_energy, state.total_mass
                        ),
                    );
                }

                // Energy history
                if self.initial_energy != 0.0 {
                    let e_ratio = state.total_energy / self.initial_energy;
                    if self.energy_history.len() >= MAX_ENERGY_HISTORY {
                        self.energy_history.remove(0);
                    }
                    self.energy_history.push((state.t, e_ratio));
                }

                if let Some(reason) = state.exit_reason {
                    self.sim_start_time = None;
                    self.push_log(Level::Info, format!("Exit: {reason}"));
                }

                self.sim_state = Some((**state).clone());
            }
            Action::SimPause => {
                self.paused = true;
                self.push_log(Level::Info, "Simulation paused".to_string());
            }
            Action::SimResume => {
                self.paused = false;
                self.push_log(Level::Info, "Simulation resumed".to_string());
            }
            Action::SimStop => {
                self.paused = false;
                self.sim_start_time = None;
                self.push_log(Level::Warn, "Simulation stopped by user".to_string());
            }
            Action::SimStart | Action::SimRestart => {
                self.energy_history.clear();
                self.initial_energy = 0.0;
                self.initial_mass = 0.0;
                self.initial_c2 = 0.0;
                self.sim_state = None;
                self.sim_start_time = Some(std::time::Instant::now());
                self.paused = false;
                if matches!(action, Action::SimRestart) {
                    self.push_log(Level::Info, "Restarting simulation…".to_string());
                } else {
                    self.push_log(Level::Info, "Starting simulation…".to_string());
                }
            }
            Action::StatusMsg(msg) => {
                self.push_log(Level::Info, msg.clone());
            }
            _ => {}
        }
        None
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        colormap: Colormap,
        data_provider: &LiveDataProvider,
    ) {
        let ideal_map_h = (area.width / 4).clamp(8, area.height.saturating_sub(16));

        let [top_area, maps_area, bottom_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(ideal_map_h),
            Constraint::Min(12),
        ])
        .areas(area);

        self.draw_controls(frame, top_area, theme);
        self.draw_maps(frame, maps_area, theme, colormap, data_provider);
        self.draw_bottom(frame, bottom_area, theme, data_provider);
    }

    fn draw_controls(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let [prog_area, energy_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).areas(area);

        match &self.sim_state {
            None => {
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(Span::styled(
                            "No simulation running.",
                            Style::default().fg(theme.dim),
                        )),
                        Line::from(vec![
                            Span::styled("  → Go to Setup ", Style::default().fg(theme.dim)),
                            Span::styled(
                                "[F1]",
                                Style::default()
                                    .fg(theme.accent)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(" and press ", Style::default().fg(theme.dim)),
                            Span::styled(
                                "[r]",
                                Style::default()
                                    .fg(theme.accent)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(" to start", Style::default().fg(theme.dim)),
                        ]),
                    ]),
                    area,
                );
            }
            Some(state) => {
                let progress = state.progress();
                let paused_tag = if self.paused { " [PAUSED]" } else { "" };
                let eta = if let Some(start) = self.sim_start_time {
                    if progress > 0.01 && progress < 1.0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let remaining = elapsed * (1.0 - progress) / progress;
                        format!("  ETA {}", format_eta(remaining))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                frame.render_widget(
                    Gauge::default()
                        .gauge_style(Style::default().fg(theme.ok))
                        .ratio(progress)
                        .label(format!(
                            "t = {:.3}/{:.1}  step {}  {:.1}%{paused_tag}{eta}",
                            state.t,
                            state.t_final,
                            state.step,
                            progress * 100.0
                        )),
                    prog_area,
                );

                let e_drift = state.energy_drift().abs();
                let cons_ratio = (1.0 - (e_drift / 1e-3).min(1.0)).clamp(0.0, 1.0);
                let cons_color = if e_drift < 1e-5 {
                    theme.ok
                } else if e_drift < 1e-3 {
                    theme.warn
                } else {
                    theme.error
                };

                frame.render_widget(
                    Gauge::default()
                        .gauge_style(Style::default().fg(cons_color))
                        .ratio(cons_ratio)
                        .label(format!(
                            "|ΔE/E| = {e_drift:.2e}  dt={:.3e}  ρ_max={:.2e}",
                            state.step_wall_ms / 1000.0,
                            state.max_density
                        )),
                    energy_area,
                );
            }
        }
    }

    fn draw_maps(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        colormap: Colormap,
        data_provider: &LiveDataProvider,
    ) {
        let [density_area, phase_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);

        match data_provider.current_state() {
            None => {
                for (a, t) in [(density_area, " ρ(x,y) "), (phase_area, " f(x,vx) ")] {
                    frame.render_widget(
                        Paragraph::new("—")
                            .block(Block::bordered().title(t))
                            .style(Style::default().fg(theme.dim)),
                        a,
                    );
                }
            }
            Some(state) => {
                frame.render_widget(
                    HeatmapWidget::new(
                        &state.density_xy,
                        state.density_nx,
                        state.density_ny,
                        " ρ(x,y) density ",
                    )
                    .colormap(colormap),
                    density_area,
                );
                frame.render_widget(
                    HeatmapWidget::new(
                        &state.phase_slice,
                        state.phase_nx,
                        state.phase_nv,
                        " f(x,vx) phase-space ",
                    )
                    .colormap(colormap),
                    phase_area,
                );
            }
        }
    }

    fn draw_bottom(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &LiveDataProvider,
    ) {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Min(40), Constraint::Length(36)]).areas(area);

        let [chart_area, log_area] =
            Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                .areas(left_area);

        // Energy chart
        if self.energy_history.is_empty() {
            frame.render_widget(
                Paragraph::new("Energy history will appear once the sim starts.")
                    .block(Block::bordered().title(" E(t)/E₀ "))
                    .style(Style::default().fg(theme.dim)),
                chart_area,
            );
        } else {
            let t_min = self.energy_history.first().map(|(t, _)| *t).unwrap_or(0.0);
            let t_max = self
                .energy_history
                .last()
                .map(|(t, _)| *t)
                .unwrap_or(1.0)
                .max(t_min + 0.001);
            let (e_min, e_max) = self
                .energy_history
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, e)| {
                    (lo.min(*e), hi.max(*e))
                });
            let e_lo = (e_min - 0.001).min(0.99);
            let e_hi = (e_max + 0.001).max(1.01);

            let chart = Chart::new(vec![
                Dataset::default()
                    .name("E/E₀")
                    .marker(symbols::Marker::Dot)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(theme.chart[0]))
                    .data(&self.energy_history),
            ])
            .block(Block::bordered().title(" E(t)/E₀ "))
            .x_axis(
                Axis::default()
                    .title("t")
                    .style(Style::default().fg(theme.dim))
                    .bounds([t_min, t_max])
                    .labels(vec![
                        format!("{t_min:.1}"),
                        format!("{:.1}", (t_min + t_max) / 2.0),
                        format!("{t_max:.1}"),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .title("E/E₀")
                    .style(Style::default().fg(theme.dim))
                    .bounds([e_lo, e_hi])
                    .labels(vec![
                        format!("{e_lo:.4}"),
                        format!("{:.4}", (e_lo + e_hi) / 2.0),
                        format!("{e_hi:.4}"),
                    ]),
            );

            frame.render_widget(chart, chart_area);
        }

        // Log stream
        let visible: Vec<_> = self
            .log_stream
            .iter()
            .filter(|(lvl, _)| match self.log_filter {
                LogFilter::All => true,
                LogFilter::WarnPlus => matches!(lvl, Level::Warn | Level::Error),
                LogFilter::ErrorOnly => matches!(lvl, Level::Error),
            })
            .rev()
            .take(log_area.height as usize)
            .collect();

        let lines: Vec<Line> = visible
            .iter()
            .rev()
            .map(|(lvl, msg)| {
                let color = match lvl {
                    Level::Info => theme.dim,
                    Level::Warn => theme.warn,
                    Level::Error => theme.error,
                };
                Line::from(Span::styled(msg.clone(), Style::default().fg(color)))
            })
            .collect();

        let filter_hint = match self.log_filter {
            LogFilter::All => "[1]All [2]Warn+ [3]Error",
            LogFilter::WarnPlus => "[1]All [2]Warn+✓ [3]Error",
            LogFilter::ErrorOnly => "[1]All [2]Warn+ [3]Error✓",
        };

        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title(format!(" Log  {filter_hint} "))),
            log_area,
        );

        // Diagnostics sidebar (right) — uses scrub-aware state
        match data_provider.current_state() {
            None => {
                frame.render_widget(
                    Paragraph::new("—").block(Block::bordered().title(" Diagnostics ")),
                    right_area,
                );
            }
            Some(state) => {
                let init_e = self.initial_energy;
                let init_m = self.initial_mass;
                let init_c = self.initial_c2;

                let rows: Vec<SparklineRow> = vec![
                    SparklineRow::new(
                        "Energy E",
                        state.total_energy,
                        if init_e != 0.0 {
                            (state.total_energy - init_e) / init_e.abs()
                        } else {
                            0.0
                        },
                    )
                    .thresholds(1e-4, 1e-2),
                    SparklineRow::new("Kinetic T", state.kinetic_energy, 0.0),
                    SparklineRow::new("Potential W", state.potential_energy, 0.0),
                    SparklineRow::new(
                        "Virial 2T/|W|",
                        state.virial_ratio,
                        state.virial_ratio - 1.0,
                    )
                    .thresholds(0.1, 0.5),
                    SparklineRow::new(
                        "Mass M",
                        state.total_mass,
                        if init_m != 0.0 {
                            (state.total_mass - init_m) / init_m.abs()
                        } else {
                            0.0
                        },
                    )
                    .thresholds(1e-6, 1e-3),
                    SparklineRow::new(
                        "Casimir C₂",
                        state.casimir_c2,
                        if init_c != 0.0 {
                            (state.casimir_c2 - init_c) / init_c.abs()
                        } else {
                            0.0
                        },
                    )
                    .thresholds(1e-5, 1e-2),
                    SparklineRow::new("Entropy S", state.entropy, 0.0),
                ];

                SparklineTable::new(&rows, " Diagnostics ").draw(frame, right_area, theme);
            }
        }
    }
}

fn format_eta(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.0}s")
    } else if secs < 3600.0 {
        format!("{}m{:02}s", secs as u64 / 60, secs as u64 % 60)
    } else {
        format!("{}h{:02}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
    }
}
