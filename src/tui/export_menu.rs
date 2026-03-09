use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Clear, List, ListItem},
};

use crate::{export::ExportFormat, themes::ThemeColors, tui::action::Action};

const FORMATS: &[ExportFormat] = &[
    ExportFormat::Csv,
    ExportFormat::Json,
    ExportFormat::Npy,
    ExportFormat::Markdown,
    ExportFormat::Screenshot,
    ExportFormat::Parquet,
    ExportFormat::Vtk,
    ExportFormat::Animation,
    ExportFormat::Zip,
];

#[derive(Default)]
pub struct ExportMenu {
    pub visible: bool,
    selected: usize,
    pub last_result: Option<Result<String, String>>,
}

impl ExportMenu {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.last_result = None;
    }

    pub fn selected_format(&self) -> ExportFormat {
        FORMATS[self.selected]
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(FORMATS.len() - 1);
                None
            }
            KeyCode::Enter => {
                self.visible = false;
                Some(Action::ExportMenuClose)
            }
            KeyCode::Esc => {
                self.visible = false;
                None
            }
            // Number key shortcuts: 1-9 select and immediately export
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < FORMATS.len() {
                    self.selected = idx;
                    self.visible = false;
                    Some(Action::ExportMenuClose)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        if !self.visible {
            return;
        }

        let w = area.width.min(40);
        let h = area.height.min(14);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let overlay = Rect::new(x, y, w, h);

        frame.render_widget(Clear, overlay);

        let block = Block::bordered()
            .title(" Export ")
            .border_style(Style::default().fg(theme.accent))
            .style(Style::default().bg(theme.bg));
        let inner = block.inner(overlay);
        frame.render_widget(block, overlay);

        let items: Vec<ListItem> = FORMATS
            .iter()
            .enumerate()
            .map(|(i, fmt)| {
                let style = if i == self.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.fg)
                };
                let marker = if i == self.selected { "► " } else { "  " };
                let shortcut = i + 1;
                ListItem::new(format!("{marker}[{shortcut}] {}", fmt.name())).style(style)
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, inner);
    }
}
