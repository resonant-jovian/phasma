use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::themes::ThemeColors;

pub struct HelpOverlay {
    pub visible: bool,
    scroll: u16,
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self { visible: false, scroll: 0 }
    }
}

impl HelpOverlay {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.scroll = 0;
    }

    pub fn scroll_down(&mut self) { self.scroll = self.scroll.saturating_add(1); }
    pub fn scroll_up(&mut self)   { self.scroll = self.scroll.saturating_sub(1); }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        if !self.visible { return; }

        // Center overlay
        let w = area.width.min(62);
        let h = area.height.min(48);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let overlay = Rect::new(x, y, w, h);

        frame.render_widget(Clear, overlay);

        let key = |s: &'static str| Span::styled(s, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        let desc = |s: &'static str| Span::styled(s, Style::default().fg(theme.fg));

        let lines = vec![
            Line::from(vec![Span::styled(" Keybindings ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from(vec![Span::styled("Global", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
            Line::from(vec![key("  F1-F10    "), desc("Switch tabs")]),
            Line::from(vec![key("  Tab       "), desc("Next tab")]),
            Line::from(vec![key("  Shift-Tab "), desc("Previous tab")]),
            Line::from(vec![key("  q         "), desc("Quit (confirms if sim running)")]),
            Line::from(vec![key("  Space     "), desc("Pause/resume simulation")]),
            Line::from(vec![key("  ?         "), desc("Toggle help")]),
            Line::from(vec![key("  e         "), desc("Export menu (1-9 for quick select)")]),
            Line::from(vec![key("  T         "), desc("Cycle theme")]),
            Line::from(vec![key("  C         "), desc("Cycle colormap (global)")]),
            Line::from(""),
            Line::from(vec![Span::styled("Setup (F1)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
            Line::from(vec![key("  j/k       "), desc("Navigate configs")]),
            Line::from(vec![key("  Enter     "), desc("Load selected config")]),
            Line::from(vec![key("  r         "), desc("Start simulation")]),
            Line::from(""),
            Line::from(vec![Span::styled("Run Control (F2)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
            Line::from(vec![key("  p/Space   "), desc("Pause/resume")]),
            Line::from(vec![key("  s         "), desc("Stop")]),
            Line::from(vec![key("  1/2/3     "), desc("Log filter: All/Warn+/Error")]),
            Line::from(""),
            Line::from(vec![Span::styled("Density (F3) / Phase (F4)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
            Line::from(vec![key("  x/y/z     "), desc("Select projection axis (F3)")]),
            Line::from(vec![key("  1-6       "), desc("Select dims (F4 Phase)")]),
            Line::from(vec![key("  +/- scroll"), desc("Zoom in/out")]),
            Line::from(vec![key("  arrows    "), desc("Pan view")]),
            Line::from(vec![key("  r/0       "), desc("Reset zoom & pan")]),
            Line::from(vec![key("  {/}       "), desc("Nudge slice position")]),
            Line::from(vec![key("  l         "), desc("Toggle log scale")]),
            Line::from(vec![key("  c         "), desc("Cycle colormap")]),
            Line::from(vec![key("  i         "), desc("Toggle info overlay")]),
            Line::from(""),
            Line::from(vec![Span::styled("Energy (F5)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
            Line::from(vec![key("  t/k/w/v   "), desc("Toggle traces: E/T/W/virial")]),
            Line::from(vec![key("  d         "), desc("Toggle drift view")]),
            Line::from(vec![key("  h/l ◄/►   "), desc("Scroll time window")]),
            Line::from(vec![key("  H/L       "), desc("Expand/contract time window")]),
            Line::from(vec![key("  f         "), desc("Fit all data")]),
            Line::from(""),
            Line::from(vec![Span::styled("Profiles (F7)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))]),
            Line::from(vec![key("  1-5       "), desc("ρ(r) M(r) Φ(r) σ(r) β(r)")]),
            Line::from(vec![key("  l         "), desc("Toggle log scale")]),
            Line::from(vec![key("  a         "), desc("Toggle analytic overlay")]),
            Line::from(""),
            Line::from(vec![Span::styled("Press ? or Esc to close", Style::default().fg(theme.dim))]),
        ];

        let block = Block::bordered()
            .title(" Help ")
            .border_style(Style::default().fg(theme.accent))
            .style(Style::default().bg(theme.bg));
        let inner = block.inner(overlay);
        frame.render_widget(block, overlay);
        frame.render_widget(
            Paragraph::new(lines).scroll((self.scroll, 0)),
            inner,
        );
    }
}
