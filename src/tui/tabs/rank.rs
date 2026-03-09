use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::themes::ThemeColors;

/// F6 Rank Monitor — stub for tensor-train rank visualization.
///
/// Will display rank evolution over time when TensorTrain PhaseSpaceRepr is implemented.
pub struct RankTab;

impl Default for RankTab {
    fn default() -> Self {
        Self
    }
}

impl RankTab {
    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Rank Monitor",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  This tab will display tensor-train rank evolution when the",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "  TensorTrain PhaseSpaceRepr is implemented in caustic.",
                Style::default().fg(theme.dim),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Planned metrics:",
                Style::default().fg(theme.fg),
            )]),
            Line::from(vec![Span::styled(
                "    • TT-rank per mode r₁..r₅",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Rank vs time chart",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Compression ratio (TT elements / full grid)",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Truncation error per recompression step",
                Style::default().fg(theme.dim),
            )]),
        ];

        let block = Block::bordered()
            .title(" Rank Monitor (stub) ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(text), inner);
    }
}
