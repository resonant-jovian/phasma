/// Timeline scrubber widget (stub — used for playback mode).
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Gauge},
};

pub struct Scrubber {
    pub t: f64,
    pub t_min: f64,
    pub t_max: f64,
    pub playing: bool,
    pub speed: f64,
}

impl Default for Scrubber {
    fn default() -> Self {
        Self {
            t: 0.0,
            t_min: 0.0,
            t_max: 1.0,
            playing: false,
            speed: 1.0,
        }
    }
}

impl Scrubber {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let progress = if self.t_max > self.t_min {
            ((self.t - self.t_min) / (self.t_max - self.t_min)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let play_sym = if self.playing { "▶" } else { "⏸" };
        let label = format!(
            "{play_sym}  t={:.3}  [{:.1}×]  [←/→ step]  [space toggle]",
            self.t, self.speed
        );

        let gauge = Gauge::default()
            .block(Block::bordered().title(" Playback "))
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(progress)
            .label(label);

        frame.render_widget(gauge, area);
    }
}
