use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::themes::ThemeColors;

/// F9 Poisson Detail — stub for Poisson solver diagnostics.
///
/// Will display FFT spectrum, convergence info, boundary treatment details.
pub struct PoissonDetailTab;

impl Default for PoissonDetailTab {
    fn default() -> Self {
        Self
    }
}

impl PoissonDetailTab {
    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Poisson Solver Detail",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  This tab will display Poisson solver diagnostics when",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "  multiple solver backends are available.",
                Style::default().fg(theme.dim),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Planned features:",
                Style::default().fg(theme.fg),
            )]),
            Line::from(vec![Span::styled(
                "    • FFT power spectrum P(k) of potential field",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Multigrid convergence history (residual vs V-cycle)",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Boundary condition treatment details",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Green's function kernel visualization",
                Style::default().fg(theme.dim),
            )]),
            Line::from(vec![Span::styled(
                "    • Solver timing breakdown (FFT vs padding vs copy)",
                Style::default().fg(theme.dim),
            )]),
        ];

        let block = Block::bordered()
            .title(" Poisson Detail (stub) ")
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(text), inner);
    }
}
