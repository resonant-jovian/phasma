use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect, Size},
    style::Style,
    widgets::{Block, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// < 80 cols: minimal chrome, no footer
    Compact,
    /// 80-159 cols: standard layout
    Standard,
    /// 160+ cols: wide layout (same as standard for now)
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

/// Minimum panel dimensions — panels below these show a placeholder.
pub const MIN_HEATMAP_WIDTH: u16 = 10;
pub const MIN_HEATMAP_HEIGHT: u16 = 5;
pub const MIN_CHART_WIDTH: u16 = 20;
pub const MIN_CHART_HEIGHT: u16 = 6;
pub const MIN_TABLE_WIDTH: u16 = 20;
pub const MIN_TABLE_HEIGHT: u16 = 3;

/// Returns true if the area is below chart minimum size.
pub fn panel_too_small(area: Rect) -> bool {
    area.width < MIN_CHART_WIDTH || area.height < MIN_CHART_HEIGHT
}

/// Returns true if the area is below heatmap minimum size.
pub fn heatmap_too_small(area: Rect) -> bool {
    area.width < MIN_HEATMAP_WIDTH || area.height < MIN_HEATMAP_HEIGHT
}

/// Render a "(too small)" placeholder inside a bordered block.
pub fn draw_too_small(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    dim_color: ratatui::style::Color,
) {
    let block = Block::bordered()
        .title(title)
        .border_style(Style::default().fg(dim_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width >= 12 && inner.height >= 1 {
        frame.render_widget(
            Paragraph::new("(too small)").style(Style::default().fg(dim_color)),
            inner,
        );
    }
}

pub struct ResponsiveLayout;

impl ResponsiveLayout {
    pub fn compute(size: Size) -> ScreenLayout {
        let mode = match size.width {
            0..=79 => LayoutMode::Compact,
            80..=159 => LayoutMode::Standard,
            _ => LayoutMode::Wide,
        };

        let full = Rect::new(0, 0, size.width, size.height);

        match mode {
            LayoutMode::Compact => {
                // Compact: status bar, tab bar, content — no footer
                let [status_area, rest] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(full);

                let [tab_bar_area, content_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(rest);

                ScreenLayout {
                    mode,
                    status_area,
                    tab_bar_area,
                    content_area,
                    footer_area: Rect::new(0, 0, 0, 0), // hidden
                }
            }
            LayoutMode::Standard | LayoutMode::Wide => {
                let [status_area, rest] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(full);

                let [tab_bar_area, content_and_footer] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(rest);

                let [content_area, footer_area] =
                    Layout::vertical([Constraint::Min(0), Constraint::Length(2)])
                        .areas(content_and_footer);

                ScreenLayout {
                    mode,
                    status_area,
                    tab_bar_area,
                    content_area,
                    footer_area,
                }
            }
        }
    }
}
