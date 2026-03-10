use ratatui::{
    Frame,
    layout::{Alignment, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

pub struct TerminalGuard {
    min_cols: u16,
    min_rows: u16,
}

impl Default for TerminalGuard {
    fn default() -> Self {
        Self {
            min_cols: 80,
            min_rows: 24,
        }
    }
}

impl TerminalGuard {
    pub fn new(min_cols: u16, min_rows: u16) -> Self {
        Self { min_cols, min_rows }
    }

    pub fn too_small(&self, size: Size) -> bool {
        size.width < self.min_cols || size.height < self.min_rows
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                " Terminal too small ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(Span::styled(
                format!("  Minimum: {}×{}", self.min_cols, self.min_rows),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                format!("  Current: {}×{}", area.width, area.height),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Please resize or zoom out.",
                Style::default().fg(Color::Gray),
            )),
        ])
        .block(Block::bordered())
        .alignment(Alignment::Center);
        frame.render_widget(msg, area);
    }
}
