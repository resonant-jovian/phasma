use ratatui::layout::{Constraint, Layout, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,
    Standard,
    Wide,
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenLayout {
    pub mode: LayoutMode,
    pub status_area: Rect,
    pub tab_bar_area: Rect,
    pub content_area: Rect,
    pub footer_area: Rect,
}

pub struct ResponsiveLayout;

impl ResponsiveLayout {
    pub fn compute(size: Size) -> ScreenLayout {
        let mode = match size.width {
            0..=79  => LayoutMode::Compact,
            80..=159 => LayoutMode::Standard,
            _ => LayoutMode::Wide,
        };

        let full = Rect::new(0, 0, size.width, size.height);

        let [status_area, rest] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
        ]).areas(full);

        let [tab_bar_area, content_and_footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
        ]).areas(rest);

        let [content_area, footer_area] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
        ]).areas(content_and_footer);

        ScreenLayout { mode, status_area, tab_bar_area, content_area, footer_area }
    }
}
