use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::collections::VecDeque;
use std::time::Instant;

use crate::sim::SimState;
use crate::themes::ThemeColors;

pub struct StatusBar {
    config_name: String,
    sim_running: bool,
    sim_paused: bool,
    sim_done: bool,
    last_t: f64,
    t_final: f64,
    // Rolling throughput (steps/sec)
    step_times: VecDeque<Instant>,
    last_step: u64,
    max_density: f64,
    sim_start_time: Option<Instant>,
    rss_mb: f64,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            config_name: "—".to_string(),
            sim_running: false,
            sim_paused: false,
            sim_done: false,
            last_t: 0.0,
            t_final: 0.0,
            step_times: VecDeque::with_capacity(12),
            last_step: 0,
            max_density: 0.0,
            sim_start_time: None,
            rss_mb: 0.0,
        }
    }
}

impl StatusBar {
    pub fn set_config_name(&mut self, name: impl Into<String>) {
        self.config_name = name.into();
    }

    pub fn on_sim_start(&mut self) {
        self.sim_running = true;
        self.sim_paused = false;
        self.sim_done = false;
        self.step_times.clear();
        self.sim_start_time = Some(Instant::now());
    }

    pub fn on_sim_pause(&mut self) { self.sim_paused = true; }
    pub fn on_sim_resume(&mut self) { self.sim_paused = false; }

    pub fn on_sim_stop(&mut self) {
        self.sim_running = false;
        self.sim_done = true;
    }

    pub fn on_state_update(&mut self, s: &SimState) {
        self.last_t = s.t;
        self.t_final = s.t_final;
        self.max_density = s.max_density;

        if s.step != self.last_step {
            self.last_step = s.step;
            if self.step_times.len() >= 10 {
                self.step_times.pop_front();
            }
            self.step_times.push_back(Instant::now());
        }

        // Update RSS every ~10 steps to avoid syscall overhead
        if s.step % 10 == 0 {
            self.rss_mb = read_rss_mb();
        }

        if s.exit_reason.is_some() {
            self.sim_running = false;
            self.sim_done = true;
        }
    }

    fn steps_per_sec(&self) -> f64 {
        if self.step_times.len() < 2 {
            return 0.0;
        }
        let dt = self.step_times.back().unwrap().duration_since(*self.step_times.front().unwrap()).as_secs_f64();
        if dt <= 0.0 { return 0.0; }
        (self.step_times.len() - 1) as f64 / dt
    }

    /// Estimate time remaining based on elapsed wall time and simulation progress.
    pub fn eta_seconds(&self) -> Option<f64> {
        let start = self.sim_start_time?;
        if self.t_final <= 0.0 || self.last_t <= 0.0 { return None; }
        let progress = self.last_t / self.t_final;
        if progress <= 0.01 || progress >= 1.0 { return None; }
        let elapsed = start.elapsed().as_secs_f64();
        Some(elapsed * (1.0 - progress) / progress)
    }

    pub fn is_sim_active(&self) -> bool {
        self.sim_running && !self.sim_paused && !self.sim_done
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let state_icon = if !self.sim_running && !self.sim_done {
            Span::styled(" ⏹ Idle", Style::default().fg(theme.dim))
        } else if self.sim_paused {
            Span::styled(" ⏸ Paused", Style::default().fg(theme.warn))
        } else if self.sim_done {
            Span::styled(" ■ Done", Style::default().fg(theme.ok))
        } else {
            Span::styled(" ▶ Running", Style::default().fg(theme.ok).add_modifier(Modifier::BOLD))
        };

        let version = env!("CARGO_PKG_VERSION");

        let sep = Span::styled(" │ ", Style::default().fg(theme.dim));

        let mut spans = vec![
            Span::styled(
                format!(" PHASMA v{version}"),
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            sep.clone(),
            Span::styled(self.config_name.clone(), Style::default().fg(theme.fg)),
            sep.clone(),
            state_icon,
        ];

        if self.sim_running || self.sim_done {
            spans.push(sep.clone());
            spans.push(Span::styled(
                format!(" t={:.2}/{:.1}", self.last_t, self.t_final),
                Style::default().fg(Color::Cyan),
            ));

            let hz = self.steps_per_sec();
            if hz > 0.0 {
                spans.push(sep.clone());
                spans.push(Span::styled(
                    format!(" {hz:.1} steps/s"),
                    Style::default().fg(theme.dim),
                ));
            }

            if self.max_density > 0.0 {
                spans.push(sep.clone());
                spans.push(Span::styled(
                    format!(" ρ_max={:.2e}", self.max_density),
                    Style::default().fg(theme.dim),
                ));
            }

            if let Some(eta) = self.eta_seconds() {
                let eta_str = format_duration(eta);
                spans.push(sep.clone());
                spans.push(Span::styled(
                    format!(" ETA {eta_str}"),
                    Style::default().fg(Color::Green),
                ));
            }
        }

        if self.rss_mb > 0.0 {
            spans.push(Span::styled(" │ ", Style::default().fg(theme.dim)));
            spans.push(Span::styled(
                format!(" {:.0} MB", self.rss_mb),
                Style::default().fg(theme.dim),
            ));
        }

        let bar = Paragraph::new(Line::from(spans))
            .style(Style::default().bg(theme.highlight));
        frame.render_widget(bar, area);
    }
}

fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.0}s")
    } else if secs < 3600.0 {
        format!("{}m{:02}s", secs as u64 / 60, secs as u64 % 60)
    } else {
        format!("{}h{:02}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
    }
}

fn read_rss_mb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("VmRSS:") {
                    let val = val.trim().trim_end_matches(" kB").trim();
                    if let Ok(kb) = val.parse::<f64>() {
                        return kb / 1024.0;
                    }
                }
            }
        }
    }
    0.0
}
