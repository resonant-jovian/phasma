use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::themes::ThemeColors;

#[derive(Debug, Clone)]
pub enum FormField {
    TextInput {
        label: String,
        value: String,
        unit: String,
        error: Option<String>,
        focused: bool,
        editing: bool,
    },
    Dropdown {
        label: String,
        options: Vec<String>,
        selected: usize,
        focused: bool,
    },
    Toggle {
        label: String,
        value: bool,
        focused: bool,
    },
}

impl FormField {
    pub fn text(label: impl Into<String>, value: impl Into<String>) -> Self {
        FormField::TextInput {
            label: label.into(),
            value: value.into(),
            unit: String::new(),
            error: None,
            focused: false,
            editing: false,
        }
    }

    pub fn text_with_unit(
        label: impl Into<String>,
        value: impl Into<String>,
        unit: impl Into<String>,
    ) -> Self {
        FormField::TextInput {
            label: label.into(),
            value: value.into(),
            unit: unit.into(),
            error: None,
            focused: false,
            editing: false,
        }
    }

    pub fn dropdown(label: impl Into<String>, options: Vec<String>, selected: usize) -> Self {
        FormField::Dropdown {
            label: label.into(),
            options,
            selected,
            focused: false,
        }
    }

    pub fn toggle(label: impl Into<String>, value: bool) -> Self {
        FormField::Toggle {
            label: label.into(),
            value,
            focused: false,
        }
    }

    pub fn set_focused(&mut self, v: bool) {
        match self {
            FormField::TextInput {
                focused, editing, ..
            } => {
                *focused = v;
                if !v {
                    *editing = false;
                }
            }
            FormField::Dropdown { focused, .. } => *focused = v,
            FormField::Toggle { focused, .. } => *focused = v,
        }
    }

    pub fn set_error(&mut self, err: Option<String>) {
        if let FormField::TextInput { error, .. } = self {
            *error = err;
        }
    }

    pub fn is_editing(&self) -> bool {
        matches!(self, FormField::TextInput { editing: true, .. })
    }

    pub fn value_str(&self) -> String {
        match self {
            FormField::TextInput { value, .. } => value.clone(),
            FormField::Dropdown {
                options, selected, ..
            } => options.get(*selected).cloned().unwrap_or_default(),
            FormField::Toggle { value, .. } => if *value { "true" } else { "false" }.to_string(),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            FormField::TextInput { label, .. } => label,
            FormField::Dropdown { label, .. } => label,
            FormField::Toggle { label, .. } => label,
        }
    }

    /// Handle a key event.  Returns true if the value changed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self {
            FormField::TextInput {
                value,
                focused,
                editing,
                ..
            } => {
                if !*focused {
                    return false;
                }
                match key.code {
                    KeyCode::Enter => {
                        *editing = !*editing;
                        false // toggling edit mode doesn't change value
                    }
                    KeyCode::Esc if *editing => {
                        *editing = false;
                        false
                    }
                    _ if *editing => match key.code {
                        KeyCode::Char(c) => {
                            value.push(c);
                            true
                        }
                        KeyCode::Backspace => {
                            value.pop();
                            true
                        }
                        _ => false,
                    },
                    _ => false,
                }
            }
            FormField::Dropdown {
                options,
                selected,
                focused,
                ..
            } => {
                if !*focused {
                    return false;
                }
                match key.code {
                    KeyCode::Left | KeyCode::Char('h') => {
                        if !options.is_empty() {
                            *selected = selected.saturating_sub(1);
                        }
                        true
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if !options.is_empty() {
                            *selected = (*selected + 1).min(options.len() - 1);
                        }
                        true
                    }
                    _ => false,
                }
            }
            FormField::Toggle { value, focused, .. } => {
                if !*focused {
                    return false;
                }
                match key.code {
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        *value = !*value;
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    pub fn draw(&self, area: Rect, buf: &mut Buffer, theme: &ThemeColors) {
        let (focused, is_editing, has_error) = match self {
            FormField::TextInput {
                focused,
                editing,
                error,
                ..
            } => (*focused, *editing, error.is_some()),
            FormField::Dropdown { focused, .. } => (*focused, false, false),
            FormField::Toggle { focused, .. } => (*focused, false, false),
        };

        let line = match self {
            FormField::TextInput {
                label,
                value,
                unit,
                error,
                ..
            } => {
                let label_color = if has_error { theme.error } else { theme.dim };
                let value_color = if is_editing { Color::Yellow } else { theme.fg };
                let cursor = if is_editing { "▎" } else { "" };
                let mut spans = vec![
                    Span::styled(format!("{:<16}", label), Style::default().fg(label_color)),
                    Span::styled(value.clone(), Style::default().fg(value_color)),
                    Span::styled(cursor, Style::default().fg(Color::Yellow)),
                ];
                if !unit.is_empty() {
                    spans.push(Span::styled(
                        format!(" {unit}"),
                        Style::default().fg(theme.dim),
                    ));
                }
                if is_editing {
                    spans.push(Span::styled(
                        "  [editing]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::DIM),
                    ));
                } else if focused {
                    spans.push(Span::styled(
                        "  [Enter to edit]",
                        Style::default().fg(theme.dim),
                    ));
                }
                if let Some(err) = error {
                    spans.push(Span::styled(
                        format!("  ✗ {err}"),
                        Style::default().fg(theme.error),
                    ));
                }
                Line::from(spans)
            }
            FormField::Dropdown {
                label,
                options,
                selected,
                ..
            } => {
                let val = options.get(*selected).map(|s| s.as_str()).unwrap_or("—");
                Line::from(vec![
                    Span::styled(format!("{:<16}", label), Style::default().fg(theme.dim)),
                    Span::styled(
                        format!("◄ {val} ►"),
                        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                    ),
                ])
            }
            FormField::Toggle { label, value, .. } => {
                let (sym, col) = if *value {
                    ("● ON ", theme.ok)
                } else {
                    ("○ OFF", theme.dim)
                };
                Line::from(vec![
                    Span::styled(format!("{:<16}", label), Style::default().fg(theme.dim)),
                    Span::styled(sym, Style::default().fg(col).add_modifier(Modifier::BOLD)),
                ])
            }
        };

        let style = if is_editing {
            Style::default().bg(Color::Rgb(40, 40, 60))
        } else if focused {
            Style::default().bg(theme.highlight)
        } else {
            Style::default()
        };

        let para = Paragraph::new(line).style(style);
        para.render(area, buf);
    }
}
