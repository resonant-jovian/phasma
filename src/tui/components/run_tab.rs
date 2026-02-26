use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, Gauge, GraphType, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, density_map::DensityMap};
use crate::{
    sim::SimState,
    tui::{action::Action, config::Config},
};

/// Maximum number of energy history points to keep (caps memory use).
const MAX_HISTORY: usize = 500;

pub struct RunTab {
    sim_state: Option<SimState>,
    /// Accumulated (t, E/E₀) pairs for the energy chart.
    energy_history: Vec<(f64, f64)>,
    paused: bool,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for RunTab {
    fn default() -> Self {
        Self {
            sim_state: None,
            energy_history: Vec::new(),
            paused: false,
            command_tx: None,
            config: Config::default(),
        }
    }
}

impl Component for RunTab {
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
            KeyCode::Char('p') | KeyCode::Char(' ') => {
                return Ok(Some(if self.paused {
                    Action::SimResume
                } else {
                    Action::SimPause
                }));
            }
            KeyCode::Char('s') => {
                return Ok(Some(Action::SimStop));
            }
            _ => {}
        }
        Ok(None)
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::SimUpdate(state) => {
                // Push energy history point
                if state.initial_energy != 0.0 {
                    let e_ratio = state.total_energy / state.initial_energy;
                    if self.energy_history.len() >= MAX_HISTORY {
                        self.energy_history.remove(0);
                    }
                    self.energy_history.push((state.t, e_ratio));
                }
                self.sim_state = Some(state);
            }
            Action::SimPause => self.paused = true,
            Action::SimResume => self.paused = false,
            Action::SimStop => {
                self.paused = false;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let outer = Block::bordered()
            .title(" Run ")
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        // Vertical split: gauges | maps | chart+diag
        let [gauge_area, maps_area, bottom_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(10),
        ])
        .areas(inner);

        self.draw_gauges(frame, gauge_area);
        self.draw_maps(frame, maps_area);
        self.draw_bottom(frame, bottom_area);

        Ok(())
    }
}

impl RunTab {
    fn draw_gauges(&self, frame: &mut Frame, area: Rect) {
        let [prog_area, energy_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).areas(area);

        match &self.sim_state {
            None => {
                let msg = Paragraph::new("No simulation running. Press [r] on the Prep tab to start.")
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(msg, area);
            }
            Some(state) => {
                let progress = state.progress();
                let paused_tag = if self.paused { " [PAUSED]" } else { "" };
                let prog_gauge = Gauge::default()
                    .block(Block::default())
                    .gauge_style(Style::default().fg(Color::Green))
                    .ratio(progress)
                    .label(format!(
                        "t = {:.3} / {:.1}   step {}   {:.1}%{paused_tag}",
                        state.t,
                        state.t_final,
                        state.step,
                        progress * 100.0
                    ));
                frame.render_widget(prog_gauge, prog_area);

                let e_drift = state.energy_drift().abs();
                // Gauge shows 1 - clamp(drift / threshold, 0, 1) so it fills green while good
                let conservation_ratio = (1.0 - (e_drift / 1e-3).min(1.0)).clamp(0.0, 1.0);
                let cons_color = if e_drift < 1e-5 {
                    Color::Green
                } else if e_drift < 1e-3 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                let energy_gauge = Gauge::default()
                    .block(Block::default())
                    .gauge_style(Style::default().fg(cons_color))
                    .ratio(conservation_ratio)
                    .label(format!("|ΔE/E| = {e_drift:.2e}   conservation"));
                frame.render_widget(energy_gauge, energy_area);
            }
        }
    }

    fn draw_maps(&self, frame: &mut Frame, area: Rect) {
        let [density_area, phase_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);

        match &self.sim_state {
            None => {
                let placeholder = Paragraph::new("—")
                    .block(Block::bordered().title(" ρ(x,y) "))
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(placeholder, density_area);
                let placeholder2 = Paragraph::new("—")
                    .block(Block::bordered().title(" f(x,vx) "))
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(placeholder2, phase_area);
            }
            Some(state) => {
                let density = DensityMap::new(
                    &state.density_xy,
                    state.density_nx,
                    state.density_ny,
                    " ρ(x,y) density projection ",
                );
                frame.render_widget(density, density_area);

                let phase = DensityMap::new(
                    &state.phase_slice,
                    state.phase_nx,
                    state.phase_nv,
                    " f(x,vx) phase-space slice ",
                );
                frame.render_widget(phase, phase_area);
            }
        }
    }

    fn draw_bottom(&self, frame: &mut Frame, area: Rect) {
        let [chart_area, diag_area] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .areas(area);

        // Energy chart
        if self.energy_history.is_empty() {
            let placeholder = Paragraph::new("Energy history will appear here once the sim starts.")
                .block(Block::bordered().title(" Energy E(t)/E₀ "))
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(placeholder, chart_area);
        } else {
            let t_min = self.energy_history.first().map(|(t, _)| *t).unwrap_or(0.0);
            let t_max = self
                .energy_history
                .last()
                .map(|(t, _)| *t)
                .unwrap_or(1.0)
                .max(t_min + 0.001);

            let (e_min, e_max) = self.energy_history.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, e)| {
                (lo.min(*e), hi.max(*e))
            });
            let e_lo = (e_min - 0.001).min(0.99);
            let e_hi = (e_max + 0.001).max(1.01);

            let data: &[(f64, f64)] = &self.energy_history;
            let datasets = vec![Dataset::default()
                .name("E/E₀")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(data)];

            let chart = Chart::new(datasets)
                .block(Block::bordered().title(" Energy E(t)/E₀ "))
                .x_axis(
                    Axis::default()
                        .title("t")
                        .style(Style::default().fg(Color::DarkGray))
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
                        .style(Style::default().fg(Color::DarkGray))
                        .bounds([e_lo, e_hi])
                        .labels(vec![
                            format!("{e_lo:.4}"),
                            format!("{:.4}", (e_lo + e_hi) / 2.0),
                            format!("{e_hi:.4}"),
                        ]),
                );
            frame.render_widget(chart, chart_area);
        }

        // Diagnostics sidebar
        let diag_text = match &self.sim_state {
            None => vec![
                Line::from(Span::styled("Diagnostics", Style::default().add_modifier(Modifier::BOLD))),
                Line::from("—"),
            ],
            Some(state) => vec![
                Line::from(Span::styled(
                    "Diagnostics",
                    Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
                )),
                Line::from(format!("M   = {:.6}", state.total_mass)),
                Line::from(format!(
                    "P   = [{:.1e}, {:.1e}, {:.1e}]",
                    state.momentum[0], state.momentum[1], state.momentum[2]
                )),
                Line::from(format!("C₂  = {:.6}", state.casimir_c2)),
                Line::from(format!("S   = {:.4}", state.entropy)),
                Line::from(""),
                Line::from(format!("t   = {:.3}", state.t)),
                Line::from(format!("step = {}", state.step)),
            ],
        };
        let diag = Paragraph::new(diag_text).block(Block::bordered().title(" Diagnostics "));
        frame.render_widget(diag, diag_area);
    }
}
