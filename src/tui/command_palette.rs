//! Modal command input (`:` key) for quick actions.
//!
//! Supports commands like:
//!   :jump 50.0    — jump to simulation time
//!   :export csv   — trigger export
//!   :cmap viridis — change colormap
//!   :theme dark   — change theme
//!   :q            — quit

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Clear, Paragraph},
};

use crate::themes::ThemeColors;

#[derive(Debug, Clone)]
pub enum Command {
    JumpToTime(f64),
    Export(String),
    SetColormap(String),
    SetTheme(String),
    Quit,
}

#[derive(Debug, Default)]
pub struct CommandPalette {
    pub visible: bool,
    input: String,
    cursor: usize,
}

impl CommandPalette {
    pub fn open(&mut self) {
        self.visible = true;
        self.input.clear();
        self.cursor = 0;
    }

    /// Open with pre-filled text (e.g. "jump " for `/` shortcut).
    pub fn open_with(&mut self, prefix: &str) {
        self.visible = true;
        self.input = prefix.to_string();
        self.cursor = self.input.len();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.input.clear();
        self.cursor = 0;
    }

    /// Handle a key event while the palette is open.
    /// Returns Some(Command) if the user submitted a valid command, or None.
    /// Returns Err(()) if the palette should close without executing.
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<Option<Command>, ()> {
        match key.code {
            KeyCode::Esc => {
                self.close();
                return Err(());
            }
            KeyCode::Enter => {
                let cmd = parse_command(&self.input);
                self.close();
                return Ok(cmd);
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.input.remove(self.cursor);
            }
            KeyCode::Left if self.cursor > 0 => {
                self.cursor -= 1;
            }
            KeyCode::Right if self.cursor < self.input.len() => {
                self.cursor += 1;
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += 1;
            }
            _ => {}
        }
        Ok(None)
    }

    /// Draw the command palette at the bottom of the screen.
    pub fn draw(&self, frame: &mut Frame, area: Rect, _theme: &ThemeColors) {
        if !self.visible {
            return;
        }
        // Draw at the very bottom row of the given area.
        let y = area.y + area.height.saturating_sub(1);
        let palette_area = Rect::new(area.x, y, area.width, 1);

        frame.render_widget(Clear, palette_area);

        let display = format!(":{}", self.input);
        let cursor_pos = self.cursor + 1; // +1 for the ':'

        // Build styled text with cursor indicator
        let text = if cursor_pos < display.len() {
            format!("{}│{}", &display[..cursor_pos], &display[cursor_pos..])
        } else {
            format!("{display}│")
        };

        let para = Paragraph::new(text).style(
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(para, palette_area);
    }
}

fn parse_command(input: &str) -> Option<Command> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim());

    match cmd.as_str() {
        "q" | "quit" | "exit" => Some(Command::Quit),
        "jump" | "goto" | "t" => arg
            .and_then(|a| a.parse::<f64>().ok())
            .map(Command::JumpToTime),
        "export" | "e" => Some(Command::Export(arg.unwrap_or("csv").to_string())),
        "cmap" | "colormap" => arg.map(|a| Command::SetColormap(a.to_string())),
        "theme" => arg.map(|a| Command::SetTheme(a.to_string())),
        _ => None,
    }
}
