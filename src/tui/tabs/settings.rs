use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::{
    colormaps::Colormap,
    themes::{Theme, ThemeColors},
    tui::action::Action,
};

const THEMES: &[Theme] = &[Theme::Dark, Theme::Light, Theme::Solarized, Theme::Gruvbox];
const COLORMAPS: &[Colormap] = &[
    Colormap::Viridis,
    Colormap::Inferno,
    Colormap::Plasma,
    Colormap::Magma,
    Colormap::Grayscale,
    Colormap::Cubehelix,
    Colormap::Coolwarm,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    Theme,
    Colormap,
    CellAspect,
    MinTermCols,
    MinTermRows,
}

const SECTIONS: &[SettingsSection] = &[
    SettingsSection::Theme,
    SettingsSection::Colormap,
    SettingsSection::CellAspect,
    SettingsSection::MinTermCols,
    SettingsSection::MinTermRows,
];

pub struct SettingsTab {
    focused: usize,
    /// We track copies so the user can cycle and see names.
    /// The real state is pushed back to App via actions.
    theme_idx: usize,
    colormap_idx: usize,
    cell_aspect: f64,
    min_cols: u16,
    min_rows: u16,
}

impl Default for SettingsTab {
    fn default() -> Self {
        Self {
            focused: 0,
            theme_idx: 0,
            colormap_idx: 0,
            cell_aspect: 0.5,
            min_cols: 80,
            min_rows: 24,
        }
    }
}

impl SettingsTab {
    /// Sync local state from app-level values so the tab always shows the real current settings.
    pub fn sync(&mut self, theme: Theme, colormap: Colormap) {
        self.theme_idx = THEMES.iter().position(|t| *t == theme).unwrap_or(0);
        self.colormap_idx = COLORMAPS.iter().position(|c| *c == colormap).unwrap_or(0);
    }

    pub fn current_theme(&self) -> Theme {
        THEMES[self.theme_idx]
    }

    pub fn current_colormap(&self) -> Colormap {
        COLORMAPS[self.colormap_idx]
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        let section = SECTIONS[self.focused];
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.focused = (self.focused + 1) % SECTIONS.len();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.focused = (self.focused + SECTIONS.len() - 1) % SECTIONS.len();
                None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                match section {
                    SettingsSection::Theme => {
                        self.theme_idx = (self.theme_idx + THEMES.len() - 1) % THEMES.len();
                        Some(Action::ThemeCycle) // we'll handle direction in app
                    }
                    SettingsSection::Colormap => {
                        self.colormap_idx =
                            (self.colormap_idx + COLORMAPS.len() - 1) % COLORMAPS.len();
                        Some(Action::VizCycleColormap)
                    }
                    SettingsSection::CellAspect => {
                        self.cell_aspect = (self.cell_aspect - 0.05).max(0.2);
                        None
                    }
                    SettingsSection::MinTermCols => {
                        self.min_cols = self.min_cols.saturating_sub(10).max(40);
                        None
                    }
                    SettingsSection::MinTermRows => {
                        self.min_rows = self.min_rows.saturating_sub(2).max(12);
                        None
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => match section {
                SettingsSection::Theme => {
                    self.theme_idx = (self.theme_idx + 1) % THEMES.len();
                    Some(Action::ThemeCycle)
                }
                SettingsSection::Colormap => {
                    self.colormap_idx = (self.colormap_idx + 1) % COLORMAPS.len();
                    Some(Action::VizCycleColormap)
                }
                SettingsSection::CellAspect => {
                    self.cell_aspect = (self.cell_aspect + 0.05).min(1.0);
                    None
                }
                SettingsSection::MinTermCols => {
                    self.min_cols = (self.min_cols + 10).min(300);
                    None
                }
                SettingsSection::MinTermRows => {
                    self.min_rows = (self.min_rows + 2).min(100);
                    None
                }
            },
            _ => None,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let [settings_area, preview_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);

        self.draw_settings_list(frame, settings_area, theme);
        self.draw_preview(frame, preview_area, theme);
    }

    fn draw_settings_list(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Settings ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            "Use ◄/► or h/l to change, ▲/▼ or j/k to navigate",
            Style::default().fg(theme.dim),
        )));
        lines.push(Line::from(""));

        for (i, section) in SECTIONS.iter().enumerate() {
            let focused = i == self.focused;
            let marker = if focused { "►" } else { " " };
            let label_style = if focused {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };
            let value_style = if focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };

            let (label, value) = match section {
                SettingsSection::Theme => {
                    let name = THEMES[self.theme_idx].name();
                    let all: Vec<&str> = THEMES.iter().map(|t| t.name()).collect();
                    ("Theme", format!("◄ {} ►  ({})", name, all.join(", ")))
                }
                SettingsSection::Colormap => {
                    let name = COLORMAPS[self.colormap_idx].name();
                    let all: Vec<&str> = COLORMAPS.iter().map(|c| c.name()).collect();
                    ("Colormap", format!("◄ {} ►  ({})", name, all.join(", ")))
                }
                SettingsSection::CellAspect => {
                    ("Cell aspect ratio", format!("◄ {:.2} ►", self.cell_aspect))
                }
                SettingsSection::MinTermCols => {
                    ("Min terminal cols", format!("◄ {} ►", self.min_cols))
                }
                SettingsSection::MinTermRows => {
                    ("Min terminal rows", format!("◄ {} ►", self.min_rows))
                }
            };

            let bg = if focused {
                theme.highlight
            } else {
                Color::Reset
            };
            lines.push(
                Line::from(vec![
                    Span::styled(format!("{marker} "), label_style),
                    Span::styled(format!("{:<22}", label), label_style),
                    Span::styled(value, value_style),
                ])
                .style(Style::default().bg(bg)),
            );
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Settings are saved automatically on quit.",
            Style::default().fg(theme.dim),
        )));

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_preview(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let block = Block::bordered()
            .title(" Preview ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let cur_theme = THEMES[self.theme_idx];
        let tc = cur_theme.colors();
        let cur_cmap = COLORMAPS[self.colormap_idx];

        let mut lines: Vec<Line> = Vec::new();

        // Theme preview
        lines.push(Line::from(Span::styled(
            format!("Theme: {}", cur_theme.name()),
            Style::default().fg(tc.accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        lines.push(Line::from(vec![
            Span::styled("  fg:        ", Style::default().fg(tc.dim)),
            Span::styled("Sample text", Style::default().fg(tc.fg)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  accent:    ", Style::default().fg(tc.dim)),
            Span::styled("Accent text", Style::default().fg(tc.accent)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  border:    ", Style::default().fg(tc.dim)),
            Span::styled("Border text", Style::default().fg(tc.border)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  dim:       ", Style::default().fg(tc.dim)),
            Span::styled("Dim text", Style::default().fg(tc.dim)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  ok:        ", Style::default().fg(tc.dim)),
            Span::styled("✓ OK", Style::default().fg(tc.ok)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  warn:      ", Style::default().fg(tc.dim)),
            Span::styled("⚠ Warning", Style::default().fg(tc.warn)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  error:     ", Style::default().fg(tc.dim)),
            Span::styled("✗ Error", Style::default().fg(tc.error)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  highlight: ", Style::default().fg(tc.dim)),
            Span::styled(" Highlight ", Style::default().fg(tc.fg).bg(tc.highlight)),
        ]));

        lines.push(Line::from(""));

        // Colormap preview — render a gradient bar
        lines.push(Line::from(Span::styled(
            format!("Colormap: {}", cur_cmap.name()),
            Style::default().fg(tc.accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        let bar_width = inner.width.saturating_sub(4) as usize;
        if bar_width > 0 {
            let mut upper_spans: Vec<Span> = Vec::with_capacity(bar_width);
            for i in 0..bar_width {
                let t = i as f64 / (bar_width - 1).max(1) as f64;
                let color = crate::colormaps::lookup(cur_cmap, t);
                upper_spans.push(Span::styled("█", Style::default().fg(color)));
            }
            lines.push(Line::from(vec![Span::raw("  ")]).patch_style(Style::default()));
            // Build the gradient line manually
            let mut gradient_spans = vec![Span::raw("  ")];
            gradient_spans.extend(upper_spans);
            lines.push(Line::from(gradient_spans));

            lines.push(Line::from(vec![
                Span::styled("  0.0", Style::default().fg(tc.dim)),
                Span::styled(" ".repeat(bar_width.saturating_sub(7)), Style::default()),
                Span::styled("1.0", Style::default().fg(tc.dim)),
            ]));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }
}
