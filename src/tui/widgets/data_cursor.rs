/// Floating data-cursor tooltip (stub).
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph},
};

#[derive(Default)]
pub struct DataCursor {
    pub visible: bool,
    pub x: u16,
    pub y: u16,
    pub label: String,
}

impl DataCursor {
    pub fn show(&mut self, x: u16, y: u16, label: impl Into<String>) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.label = label.into();
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn draw(&self, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let w = (self.label.len() as u16 + 4).min(40);
        let area = Rect::new(
            self.x.min(frame.area().width.saturating_sub(w)),
            self.y,
            w,
            3,
        );
        frame.render_widget(Clear, area);
        let tooltip = Paragraph::new(self.label.as_str())
            .block(Block::bordered().style(Style::default().fg(Color::Cyan)));
        frame.render_widget(tooltip, area);
    }
}
