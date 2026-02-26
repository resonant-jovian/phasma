use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{
    sim::SimState,
    tui::{action::Action, config::Config},
};

pub struct ExitTab {
    sim_state: Option<SimState>,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Default for ExitTab {
    fn default() -> Self {
        Self {
            sim_state: None,
            start_time: None,
            end_time: None,
            command_tx: None,
            config: Config::default(),
        }
    }
}

impl Component for ExitTab {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::SimStart => {
                self.start_time = Some(Instant::now());
                self.end_time = None;
            }
            Action::SimUpdate(state) => {
                if state.exit_reason.is_some() {
                    self.end_time = Some(Instant::now());
                }
                self.sim_state = Some(state);
            }
            Action::SimStop => {
                self.end_time = Some(Instant::now());
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let outer = Block::bordered()
            .title(" Exit ")
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let [status_area, table_area] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .areas(inner);

        // --- Status / exit reason ---
        let status_text = match &self.sim_state {
            None => Line::from(Span::styled(
                "Simulation not started",
                Style::default().fg(Color::DarkGray),
            )),
            Some(state) => match state.exit_reason {
                None => Line::from(Span::styled(
                    "Simulation running…",
                    Style::default().fg(Color::Yellow),
                )),
                Some(reason) => Line::from(vec![
                    Span::styled("Exit: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        reason.to_string(),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                ]),
            },
        };

        let wall_clock = match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => format!("{:.1} s", (end - start).as_secs_f64()),
            (Some(start), None) => format!("{:.1} s (running)", start.elapsed().as_secs_f64()),
            _ => "—".to_string(),
        };

        let status = Paragraph::new(vec![
            status_text,
            Line::from(format!("Wall-clock: {wall_clock}")),
        ])
        .block(Block::new().borders(Borders::NONE));
        frame.render_widget(status, status_area);

        // --- Conservation table ---
        if let Some(state) = &self.sim_state {
            let header = Row::new(vec!["Quantity", "Initial", "Final", "Error"])
                .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

            let e_err = state.energy_drift();
            let m_err = (state.total_mass - 1.0).abs();
            let p_mag = (state.momentum[0].powi(2)
                + state.momentum[1].powi(2)
                + state.momentum[2].powi(2))
            .sqrt();

            let rows = vec![
                Row::new(vec![
                    Cell::from("Energy E"),
                    Cell::from(format!("{:.6}", state.initial_energy)),
                    Cell::from(format!("{:.6}", state.total_energy)),
                    Cell::from(format!("|ΔE/E| = {e_err:.2e}"))
                        .style(drift_style(e_err.abs(), 1e-4, 1e-2)),
                ]),
                Row::new(vec![
                    Cell::from("Mass M"),
                    Cell::from("1.000000"),
                    Cell::from(format!("{:.6}", state.total_mass)),
                    Cell::from(format!("|ΔM/M| = {m_err:.2e}"))
                        .style(drift_style(m_err, 1e-6, 1e-3)),
                ]),
                Row::new(vec![
                    Cell::from("Momentum |P|"),
                    Cell::from("0.000000"),
                    Cell::from(format!("{p_mag:.2e}")),
                    Cell::from(format!("|P| = {p_mag:.2e}"))
                        .style(drift_style(p_mag, 1e-8, 1e-4)),
                ]),
                Row::new(vec![
                    Cell::from("Casimir C₂"),
                    Cell::from("0.998000"),
                    Cell::from(format!("{:.6}", state.casimir_c2)),
                    Cell::from(format!("ΔC₂ = {:.2e}", (state.casimir_c2 - 0.998).abs()))
                        .style(drift_style((state.casimir_c2 - 0.998).abs(), 1e-5, 1e-2)),
                ]),
            ];

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ],
            )
            .header(header)
            .block(Block::bordered().title(" Conservation audit "));

            frame.render_widget(table, table_area);
        } else {
            let msg = Paragraph::new("No simulation data yet.")
                .block(Block::bordered().title(" Conservation audit "))
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(msg, table_area);
        }

        Ok(())
    }
}

/// Returns a style coloured green / yellow / red depending on drift magnitude.
fn drift_style(val: f64, warn: f64, bad: f64) -> Style {
    if val < warn {
        Style::default().fg(Color::Green)
    } else if val < bad {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    }
}
