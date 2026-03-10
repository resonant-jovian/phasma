use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Cell, Row, Table},
};

use crate::themes::ThemeColors;

#[derive(Debug, Clone)]
pub struct SparklineRow {
    pub label: String,
    pub value: f64,
    pub drift: f64, // (current - initial) / |initial|
    pub warn_threshold: f64,
    pub error_threshold: f64,
    pub unit: String,
}

impl SparklineRow {
    pub fn new(label: impl Into<String>, value: f64, drift: f64) -> Self {
        Self {
            label: label.into(),
            value,
            drift,
            warn_threshold: 1e-3,
            error_threshold: 1e-1,
            unit: String::new(),
        }
    }

    pub fn thresholds(mut self, warn: f64, error: f64) -> Self {
        self.warn_threshold = warn;
        self.error_threshold = error;
        self
    }

    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }

    fn status_symbol(&self, theme: &ThemeColors) -> (&'static str, ratatui::style::Color) {
        let d = self.drift.abs();
        if d >= self.error_threshold {
            ("✗", theme.error)
        } else if d >= self.warn_threshold {
            ("⚠", theme.warn)
        } else {
            ("✓", theme.ok)
        }
    }
}

pub struct SparklineTable<'a> {
    pub rows: &'a [SparklineRow],
    pub title: &'a str,
}

impl<'a> SparklineTable<'a> {
    pub fn new(rows: &'a [SparklineRow], title: &'a str) -> Self {
        Self { rows, title }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let header = Row::new(vec!["Quantity", "Value", "Drift", ""]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.accent),
        );

        let table_rows: Vec<Row> = self
            .rows
            .iter()
            .map(|r| {
                let (sym, sym_color) = r.status_symbol(theme);
                let drift_color = sym_color;

                let value_str =
                    if r.value.abs() >= 1000.0 || (r.value.abs() < 0.001 && r.value != 0.0) {
                        format!("{:.3e}", r.value)
                    } else {
                        format!("{:.6}", r.value)
                    };

                let drift_str = if r.drift.abs() >= 1e-10 {
                    format!("{:+.2e}", r.drift)
                } else {
                    "~0".to_string()
                };

                Row::new(vec![
                    Cell::from(r.label.clone()).style(Style::default().fg(theme.fg)),
                    Cell::from(format!(
                        "{value_str}{}",
                        if r.unit.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", r.unit)
                        }
                    ))
                    .style(Style::default().fg(theme.dim)),
                    Cell::from(drift_str).style(Style::default().fg(drift_color)),
                    Cell::from(sym)
                        .style(Style::default().fg(sym_color).add_modifier(Modifier::BOLD)),
                ])
            })
            .collect();

        let table = Table::new(
            table_rows,
            [
                Constraint::Min(14),
                Constraint::Min(14),
                Constraint::Min(10),
                Constraint::Length(2),
            ],
        )
        .header(header)
        .block(Block::bordered().title(self.title));

        frame.render_widget(table, area);
    }
}
