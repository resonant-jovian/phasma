use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::themes::ThemeColors;

#[derive(Default)]
pub struct HelpOverlay {
    pub visible: bool,
    scroll: u16,
}

impl HelpOverlay {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.scroll = 0;
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        if !self.visible {
            return;
        }

        // Center overlay
        let w = area.width.min(62);
        let h = area.height.min(68);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let overlay = Rect::new(x, y, w, h);

        frame.render_widget(Clear, overlay);

        let key = |s: &'static str| {
            Span::styled(
                s,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        };
        let desc = |s: &'static str| Span::styled(s, Style::default().fg(theme.fg));

        let section = |s: &'static str| {
            Line::from(vec![Span::styled(
                s,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])
        };

        let lines = vec![
            Line::from(vec![Span::styled(
                " Keybindings ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            section("Global"),
            Line::from(vec![key("  F1-F10    "), desc("Switch tabs")]),
            Line::from(vec![key("  Tab       "), desc("Next tab")]),
            Line::from(vec![key("  Shift+Tab "), desc("Previous tab")]),
            Line::from(vec![
                key("  q         "),
                desc("Quit (confirms if sim running)"),
            ]),
            Line::from(vec![key("  Space     "), desc("Pause/resume simulation")]),
            Line::from(vec![key("  ?         "), desc("Toggle this help")]),
            Line::from(vec![
                key("  e         "),
                desc("Export menu (1-9 quick select)"),
            ]),
            Line::from(vec![key("  Shift+t   "), desc("Cycle theme")]),
            Line::from(vec![key("  Shift+c   "), desc("Cycle colormap (global)")]),
            Line::from(vec![
                key("  ◄/►       "),
                desc("Scrub backward/forward in time"),
            ]),
            Line::from(vec![key("  Backspace "), desc("Jump to live (exit scrub)")]),
            Line::from(vec![key("  :         "), desc("Command palette")]),
            Line::from(vec![
                key("  a         "),
                desc("Add bookmark at current time"),
            ]),
            Line::from(vec![key("  Ctrl+B    "), desc("Navigate to next bookmark")]),
            Line::from(""),
            section("Setup (F1)"),
            Line::from(vec![key("  j/k ▲/▼   "), desc("Navigate config list")]),
            Line::from(vec![key("  Enter     "), desc("Load selected config")]),
            Line::from(vec![key("  r         "), desc("Start simulation")]),
            Line::from(""),
            section("Run Control (F2)"),
            Line::from(vec![key("  p/Space   "), desc("Pause/resume")]),
            Line::from(vec![key("  s         "), desc("Stop simulation")]),
            Line::from(vec![key("  r         "), desc("Restart simulation")]),
            Line::from(vec![
                key("  1/2/3     "),
                desc("Log filter: All / Warn+ / Error"),
            ]),
            Line::from(""),
            section("Density (F3)"),
            Line::from(vec![key("  1/2/3     "), desc("Projection axis")]),
            Line::from(vec![key("  +/- scroll"), desc("Zoom in/out")]),
            Line::from(vec![key("  r/0       "), desc("Reset zoom")]),
            Line::from(vec![key("  l         "), desc("Toggle log scale")]),
            Line::from(vec![key("  Shift+c   "), desc("Cycle colormap (global)")]),
            Line::from(vec![key("  i         "), desc("Toggle info bar")]),
            Line::from(vec![key("  n         "), desc("Toggle contour overlay")]),
            Line::from(""),
            section("Phase Space (F4)"),
            Line::from(vec![key("  1/2/3     "), desc("Spatial dim (x₁/x₂/x₃)")]),
            Line::from(vec![key("  4/5/6     "), desc("Velocity dim (v₁/v₂/v₃)")]),
            Line::from(vec![key("  +/- scroll"), desc("Zoom in/out")]),
            Line::from(vec![key("  r/0       "), desc("Reset zoom")]),
            Line::from(vec![key("  l         "), desc("Toggle log scale")]),
            Line::from(vec![key("  Shift+c   "), desc("Cycle colormap (global)")]),
            Line::from(vec![key("  i         "), desc("Toggle info bar")]),
            Line::from(vec![key("  ,/.       "), desc("Adjust slice position")]),
            Line::from(vec![
                key("  p         "),
                desc("Toggle physical aspect ratio"),
            ]),
            Line::from(""),
            section("Energy (F5)"),
            Line::from(vec![
                key("  t/k/w     "),
                desc("Toggle traces: total/kinetic/potential"),
            ]),
            Line::from(vec![key("  d         "), desc("Toggle drift view")]),
            Line::from(vec![
                key("  1/2/3/4   "),
                desc("Panel: Energy/Mass/Virial/Entropy"),
            ]),
            Line::from(vec![key("  h/l       "), desc("Scroll time left/right")]),
            Line::from(vec![
                key("  Shift+h/l "),
                desc("Expand/contract time window"),
            ]),
            Line::from(vec![
                key("  f         "),
                desc("Fit all (reset time window)"),
            ]),
            Line::from(vec![key("  g         "), desc("Toggle grid lines")]),
            Line::from(""),
            section("Rank (F6) / Perf (F8) / Poisson (F9)"),
            Line::from(vec![desc("  Display-only — no tab-specific keys")]),
            Line::from(""),
            section("Profiles (F7)"),
            Line::from(vec![
                key("  1/2/3/4/5 "),
                desc("ρ(r) / M(r) / Φ(r) / σ(r) / β(r)"),
            ]),
            Line::from(vec![key("  l         "), desc("Toggle log scale")]),
            Line::from(vec![key("  a         "), desc("Toggle analytic overlay")]),
            Line::from(vec![
                key("  s         "),
                desc("Toggle stacked/single layout"),
            ]),
            Line::from(""),
            section("Settings (F10)"),
            Line::from(vec![key("  j/k ▲/▼   "), desc("Navigate settings")]),
            Line::from(vec![key("  h/l ◄/►   "), desc("Change value")]),
            Line::from(""),
            Line::from(vec![Span::styled(
                " ? or Esc to close  ▲/▼ to scroll",
                Style::default().fg(theme.dim),
            )]),
        ];

        let block = Block::bordered()
            .title(" Help ")
            .border_style(Style::default().fg(theme.accent))
            .style(Style::default().bg(theme.bg));
        let inner = block.inner(overlay);
        frame.render_widget(block, overlay);
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), inner);
    }
}
