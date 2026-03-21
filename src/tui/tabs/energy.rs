use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use ratatui_plt::prelude::{
    Annotation, Axis as PltAxis, Bounds, LegendPosition, LinePlot, PsdPlot, RefLineDash,
    ReferenceLine, Scale, Series, Spectrogram, StackedArea,
};
use ratatui_plt::fft::stft;

use std::borrow::Cow;

use crate::{
    data::DataProvider,
    themes::ThemeColors,
    tui::action::Action,
    tui::plt_bridge::phasma_theme_to_plt,
};

type SeriesData<'a> = (&'a str, &'a [(f64, f64)], Color);

/// Which traces are visible in the energy chart
#[derive(Default)]
struct TraceVisibility {
    total_energy: bool,
    kinetic_energy: bool,
    potential_energy: bool,
}

/// Time window for scrolling/zooming the x-axis
#[derive(Default)]
struct TimeWindow {
    /// Right edge override (None = auto/latest)
    t_end: Option<f64>,
    /// Window width in sim time units (None = fit all)
    width: Option<f64>,
}

impl TimeWindow {
    /// Apply window to data bounds, returning (x_min, x_max)
    fn apply(&self, data_x_min: f64, data_x_max: f64) -> (f64, f64) {
        let x_max = self.t_end.unwrap_or(data_x_max);
        match self.width {
            Some(w) => ((x_max - w).max(data_x_min), x_max),
            None => (data_x_min, x_max),
        }
    }

    /// Scroll left by 10% of current window width
    fn scroll_left(&mut self, data_x_min: f64, data_x_max: f64) {
        let w = self.width.unwrap_or(data_x_max - data_x_min);
        let step = w * 0.1;
        let current_end = self.t_end.unwrap_or(data_x_max);
        let new_end = (current_end - step).max(data_x_min + w * 0.1);
        self.t_end = Some(new_end);
    }

    /// Scroll right by 10% of current window width
    fn scroll_right(&mut self, data_x_min: f64, data_x_max: f64) {
        let w = self.width.unwrap_or(data_x_max - data_x_min);
        let step = w * 0.1;
        let current_end = self.t_end.unwrap_or(data_x_max);
        let new_end = (current_end + step).min(data_x_max);
        self.t_end = Some(new_end);
    }

    /// Expand window (zoom out time axis)
    fn expand(&mut self, data_x_min: f64, data_x_max: f64) {
        let full = data_x_max - data_x_min;
        let current = self.width.unwrap_or(full);
        self.width = Some((current * 1.5).min(full));
        if self.width.unwrap_or(0.0) >= full * 0.99 {
            self.width = None; // snap to fit-all
            self.t_end = None;
        }
    }

    /// Contract window (zoom in time axis)
    fn contract(&mut self, data_x_min: f64, data_x_max: f64) {
        let full = data_x_max - data_x_min;
        let current = self.width.unwrap_or(full);
        let new_w = (current / 1.5).max(full * 0.01);
        self.width = Some(new_w);
    }

    /// Fit all data
    fn fit_all(&mut self) {
        self.t_end = None;
        self.width = None;
    }
}

/// Cached time-series data rebuilt only when new diagnostics arrive.
#[derive(Default)]
struct CachedSeries {
    total_energy: Vec<(f64, f64)>,
    kinetic_energy: Vec<(f64, f64)>,
    potential_energy: Vec<(f64, f64)>,
    energy_drift: Vec<(f64, f64)>,
    mass_drift: Vec<(f64, f64)>,
    c2_drift: Vec<(f64, f64)>,
    abs_energy_drift: Vec<(f64, f64)>,
    abs_mass_drift: Vec<(f64, f64)>,
    abs_c2_drift: Vec<(f64, f64)>,
    virial: Vec<(f64, f64)>,
    entropy: Vec<(f64, f64)>,
    momentum_x: Vec<(f64, f64)>,
    momentum_y: Vec<(f64, f64)>,
    momentum_z: Vec<(f64, f64)>,
    cached_at_len: usize,
}

/// Cached spectrogram data (recomputed periodically).
struct CachedSpectrogram {
    grid: ratatui_plt::prelude::GridData,
    at_len: usize,
}

impl Default for CachedSpectrogram {
    fn default() -> Self {
        Self {
            grid: ratatui_plt::prelude::GridData::new(vec![0.0], vec![0.0], vec![vec![0.0]]),
            at_len: 0,
        }
    }
}

pub struct EnergyTab {
    traces: TraceVisibility,
    show_drift: bool,      // show fractional drift or absolute values
    selected_panel: usize, // 0=energy, 1=mass, 2=virial, 3=entropy, 4=psd, 5=momentum, 6=spectrogram
    show_grid: bool,
    stacked_mode: bool,
    time_window: TimeWindow,
    cached: CachedSeries,
    cached_spectrogram: CachedSpectrogram,
    /// Time and label of exit event for annotation
    exit_event_time: Option<f64>,
    exit_reason_label: String,
}

impl Default for EnergyTab {
    fn default() -> Self {
        Self {
            traces: TraceVisibility {
                total_energy: true,
                kinetic_energy: true,
                potential_energy: true,
            },
            show_drift: false,
            selected_panel: 0,
            show_grid: false,
            stacked_mode: false,
            time_window: TimeWindow::default(),
            cached: CachedSeries::default(),
            cached_spectrogram: CachedSpectrogram::default(),
            exit_event_time: None,
            exit_reason_label: String::new(),
        }
    }
}

impl EnergyTab {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('t') => {
                self.traces.total_energy = !self.traces.total_energy;
                None
            }
            KeyCode::Char('k') => {
                self.traces.kinetic_energy = !self.traces.kinetic_energy;
                None
            }
            KeyCode::Char('w') => {
                self.traces.potential_energy = !self.traces.potential_energy;
                None
            }
            KeyCode::Char('d') => {
                self.show_drift = !self.show_drift;
                None
            }
            KeyCode::Char('g') => {
                self.show_grid = !self.show_grid;
                None
            }
            KeyCode::Char('1') => {
                self.selected_panel = 0;
                None
            }
            KeyCode::Char('2') => {
                self.selected_panel = 1;
                None
            }
            KeyCode::Char('3') => {
                self.selected_panel = 2;
                None
            }
            KeyCode::Char('4') => {
                self.selected_panel = 3;
                None
            }
            KeyCode::Char('5') => {
                self.selected_panel = 4;
                None
            }
            KeyCode::Char('6') => {
                self.selected_panel = 5;
                None
            }
            KeyCode::Char('7') => {
                self.selected_panel = 6;
                None
            }
            KeyCode::Char('S') => {
                self.stacked_mode = !self.stacked_mode;
                None
            }
            // Time window controls
            KeyCode::Char('h') => {
                self.time_window.scroll_left(0.0, f64::MAX);
                None
            }
            KeyCode::Char('l') => {
                self.time_window.scroll_right(0.0, f64::MAX);
                None
            }
            KeyCode::Char('H') => {
                self.time_window.expand(0.0, f64::MAX);
                None
            }
            KeyCode::Char('L') => {
                self.time_window.contract(0.0, f64::MAX);
                None
            }
            KeyCode::Char('f') => {
                self.time_window.fit_all();
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, action: &Action) -> Option<Action> {
        if let Action::SimUpdate(state) = action
            && let Some(ref reason) = state.exit_reason
            && self.exit_event_time.is_none()
        {
            self.exit_event_time = Some(state.t);
            self.exit_reason_label = format!("{reason:?}");
        }
        None
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let diag = data_provider.diagnostics();
        if diag.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "No diagnostics data yet — start a simulation on ",
                        Style::default().fg(theme.dim),
                    ),
                    Span::styled(
                        "[F2]",
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                area,
            );
            return;
        }

        // Rebuild cached series only when new data arrives
        let current_len = diag.total_energy.len();
        if current_len != self.cached.cached_at_len {
            let energy_drift = diag.energy_drift_series();
            let mass_drift = diag.mass_drift_series();
            let c2_drift = diag.c2_drift_series();

            let mut abs_energy = energy_drift.clone();
            let mut abs_mass = mass_drift.clone();
            let mut abs_c2 = c2_drift.clone();
            for p in &mut abs_energy {
                p.1 = p.1.abs();
            }
            for p in &mut abs_mass {
                p.1 = p.1.abs();
            }
            for p in &mut abs_c2 {
                p.1 = p.1.abs();
            }

            self.cached = CachedSeries {
                total_energy: diag.total_energy.iter_chart_data(),
                kinetic_energy: diag.kinetic_energy.iter_chart_data(),
                potential_energy: diag.potential_energy.iter_chart_data(),
                energy_drift,
                mass_drift,
                c2_drift,
                abs_energy_drift: abs_energy,
                abs_mass_drift: abs_mass,
                abs_c2_drift: abs_c2,
                virial: diag.virial_ratio.iter_chart_data(),
                entropy: diag.entropy.iter_chart_data(),
                momentum_x: diag.momentum_x.iter_chart_data(),
                momentum_y: diag.momentum_y.iter_chart_data(),
                momentum_z: diag.momentum_z.iter_chart_data(),
                cached_at_len: current_len,
            };
        }

        // Compact mode: single panel (selected by panel key 1-7)
        if area.width < 76 {
            match self.selected_panel {
                0 => self.draw_energy_chart(frame, area, theme, data_provider),
                1 => self.draw_mass_chart(frame, area, theme),
                2 => self.draw_virial_chart(frame, area, theme),
                3 => self.draw_entropy_chart(frame, area, theme),
                4 => self.draw_psd_chart(frame, area, theme),
                5 => self.draw_momentum_chart(frame, area, theme),
                6 => self.draw_spectrogram(frame, area, theme),
                _ => self.draw_psd_chart(frame, area, theme),
            }
            return;
        }

        // 2×2 grid layout — bottom-right swaps based on selected_panel (5=momentum, 6=spectrogram)
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        let [top_left, top_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top);

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom);

        self.draw_energy_chart(frame, top_left, theme, data_provider);
        self.draw_mass_chart(frame, top_right, theme);
        self.draw_virial_chart(frame, bottom_left, theme);

        match self.selected_panel {
            5 => self.draw_momentum_chart(frame, bottom_right, theme),
            6 => self.draw_spectrogram(frame, bottom_right, theme),
            _ => self.draw_entropy_chart(frame, bottom_right, theme),
        }
    }

    fn draw_energy_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        if self.show_drift {
            let threshold = data_provider
                .config()
                .map(|c| c.exit.energy_drift_tolerance)
                .unwrap_or(0.5);
            draw_single_series_with_threshold(
                frame,
                area,
                " ΔE/E₀ ",
                &self.cached.energy_drift,
                theme.chart[3],
                theme,
                Some(threshold),
                &self.time_window,
                self.show_grid,
                self.exit_event_time.as_ref().map(|&t| (t, &self.exit_reason_label as &str)),
            );
        } else if self.stacked_mode
            && !self.cached.kinetic_energy.is_empty()
            && !self.cached.potential_energy.is_empty()
        {
            // Stacked area: T and |W| stack to ~|E_tot|
            let abs_w: Vec<(f64, f64)> = self
                .cached
                .potential_energy
                .iter()
                .map(|&(t, w)| (t, w.abs()))
                .collect();

            let plt_theme = phasma_theme_to_plt(theme);
            let stacked = StackedArea::new()
                .series(
                    Series::new("T")
                        .data(self.cached.kinetic_energy.clone())
                        .color(theme.chart[1]),
                )
                .series(
                    Series::new("|W|")
                        .data(abs_w)
                        .color(theme.chart[2]),
                )
                .x_axis(PltAxis::new().label("t"))
                .y_axis(PltAxis::new())
                .title(" Energy (stacked) ")
                .show_legend(true)
                .legend_position(LegendPosition::TopRight)
                .theme(plt_theme);

            frame.render_widget(&stacked, area);
        } else {
            let mut datasets: Vec<SeriesData> = Vec::new();

            if self.traces.total_energy && !self.cached.total_energy.is_empty() {
                datasets.push(("E_tot", &self.cached.total_energy, theme.chart[0]));
            }
            if self.traces.kinetic_energy && !self.cached.kinetic_energy.is_empty() {
                datasets.push(("T", &self.cached.kinetic_energy, theme.chart[1]));
            }
            if self.traces.potential_energy && !self.cached.potential_energy.is_empty() {
                datasets.push(("W", &self.cached.potential_energy, theme.chart[2]));
            }

            draw_multi_series_windowed(
                frame,
                area,
                " Energy ",
                &datasets,
                theme,
                &self.time_window,
                self.show_grid,
            );
        }
    }

    fn draw_mass_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let series: Vec<SeriesData> = vec![
            ("ΔE/E", &self.cached.abs_energy_drift, theme.chart[0]),
            ("ΔM/M", &self.cached.abs_mass_drift, theme.chart[1]),
            ("ΔC₂/C₂", &self.cached.abs_c2_drift, theme.chart[2]),
        ];
        draw_multi_series_windowed(
            frame,
            area,
            " Conservation Errors ",
            &series,
            theme,
            &self.time_window,
            self.show_grid,
        );
    }

    fn draw_virial_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        draw_single_series_with_threshold(
            frame,
            area,
            " 2T/|W| (virial) ",
            &self.cached.virial,
            theme.chart[5],
            theme,
            Some(1.0),
            &self.time_window,
            self.show_grid,
            None,
        );
    }

    fn draw_entropy_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        draw_single_series_with_threshold(
            frame,
            area,
            " S(t) ",
            &self.cached.entropy,
            theme.chart[6],
            theme,
            None,
            &self.time_window,
            self.show_grid,
            None,
        );
    }

    fn draw_psd_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        if self.cached.total_energy.len() < 8 {
            frame.render_widget(
                Block::bordered()
                    .title(" PSD — Energy ")
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        // Compute PSD of the total energy time series using FFT
        let n = self.cached.total_energy.len();
        let dt = if n >= 2 {
            let t0 = self.cached.total_energy.first().map(|(t, _)| *t).unwrap_or(0.0);
            let tn = self.cached.total_energy.last().map(|(t, _)| *t).unwrap_or(1.0);
            (tn - t0) / (n - 1) as f64
        } else {
            1.0
        };
        let sample_rate = if dt > 0.0 { 1.0 / dt } else { 1.0 };

        // Remove mean and compute FFT
        let mean: f64 =
            self.cached.total_energy.iter().map(|(_, e)| e).sum::<f64>() / n as f64;
        let mut input: Vec<rustfft::num_complex::Complex<f64>> = self
            .cached
            .total_energy
            .iter()
            .map(|(_, e)| rustfft::num_complex::Complex::new(e - mean, 0.0))
            .collect();

        let mut planner = rustfft::FftPlanner::new();
        let fft = planner.plan_fft_forward(n);
        fft.process(&mut input);

        // Compute one-sided PSD: |X(f)|² / (N * fs)
        let n_half = n / 2 + 1;
        let psd_data: Vec<(f64, f64)> = (1..n_half)
            .filter_map(|i| {
                let freq = i as f64 * sample_rate / n as f64;
                let power = (input[i].norm_sqr()) / (n as f64 * sample_rate);
                if freq > 0.0 && power > 0.0 {
                    Some((freq, power))
                } else {
                    None
                }
            })
            .collect();

        if psd_data.len() < 2 {
            frame.render_widget(
                Block::bordered()
                    .title(" PSD — Energy ")
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let plt_theme = phasma_theme_to_plt(theme);
        let psd = PsdPlot::new()
            .series(
                Series::new("E(f)")
                    .data(psd_data)
                    .color(theme.chart[0]),
            )
            .x_axis(PltAxis::new().label("frequency").scale(Scale::Log(10.0)))
            .y_axis(PltAxis::new().label("PSD").scale(Scale::Log(10.0)))
            .title(" PSD — Energy ")
            .theme(plt_theme);

        frame.render_widget(&psd, area);
    }

    fn draw_momentum_chart(&self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        let has_data = !self.cached.momentum_x.is_empty()
            || !self.cached.momentum_y.is_empty()
            || !self.cached.momentum_z.is_empty();

        if !has_data {
            frame.render_widget(
                Block::bordered()
                    .title(" Momentum P(t) ")
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        let series: Vec<SeriesData> = vec![
            ("Px", &self.cached.momentum_x, theme.chart[0]),
            ("Py", &self.cached.momentum_y, theme.chart[1]),
            ("Pz", &self.cached.momentum_z, theme.chart[2]),
        ];
        draw_multi_series_windowed(
            frame,
            area,
            " Momentum P(t) ",
            &series,
            theme,
            &self.time_window,
            self.show_grid,
        );
    }

    fn draw_spectrogram(&mut self, frame: &mut Frame, area: Rect, theme: &ThemeColors) {
        if self.cached.total_energy.len() < 64 {
            frame.render_widget(
                Block::bordered()
                    .title(" Energy Spectrogram ")
                    .border_style(Style::default().fg(theme.border)),
                area,
            );
            return;
        }

        // Recompute spectrogram when enough new data has arrived
        let n = self.cached.total_energy.len();
        if n > self.cached_spectrogram.at_len + 50 || self.cached_spectrogram.at_len == 0 {
            let signal: Vec<f64> = self.cached.total_energy.iter().map(|(_, e)| *e).collect();
            let grid = stft(&signal, 64, 32);
            self.cached_spectrogram = CachedSpectrogram { grid, at_len: n };
        }

        let plt_theme = phasma_theme_to_plt(theme);
        let spec = Spectrogram::new(self.cached_spectrogram.grid.clone())
            .title(" Energy Spectrogram ")
            .x_axis(PltAxis::new().label("time"))
            .y_axis(PltAxis::new().label("freq"))
            .show_colorbar(true)
            .theme(plt_theme);

        frame.render_widget(&spec, area);
    }
}

/// Compute data bounds with 5% y-padding (local, replaces chart_utils::data_bounds).
fn data_bounds(data: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for &(x, y) in data {
        if x < x_min {
            x_min = x;
        }
        if x > x_max {
            x_max = x;
        }
        if y < y_min {
            y_min = y;
        }
        if y > y_max {
            y_max = y;
        }
    }
    if x_min >= x_max {
        x_max = x_min + 1.0;
    }
    if y_min >= y_max {
        y_max = y_min + 1.0;
    }
    let ypad = (y_max - y_min) * 0.05;
    (x_min, x_max, y_min - ypad, y_max + ypad)
}

fn draw_single_series_with_threshold(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    data: &[(f64, f64)],
    color: Color,
    theme: &ThemeColors,
    threshold: Option<f64>,
    time_window: &TimeWindow,
    show_grid: bool,
    exit_event: Option<(f64, &str)>,
) {
    if data.is_empty() {
        frame.render_widget(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
            area,
        );
        return;
    }

    let (raw_x_min, raw_x_max, y_min_raw, y_max_raw) = data_bounds(data);
    let (x_min, x_max) = time_window.apply(raw_x_min, raw_x_max);

    // Filter data to window (skip in fit-all mode)
    let windowed: Cow<'_, [(f64, f64)]> =
        if time_window.t_end.is_none() && time_window.width.is_none() {
            Cow::Borrowed(data)
        } else {
            Cow::Owned(
                data.iter()
                    .copied()
                    .filter(|(x, _)| *x >= x_min && *x <= x_max)
                    .collect(),
            )
        };
    let (_, _, y_min, y_max) = if windowed.is_empty() {
        (x_min, x_max, y_min_raw, y_max_raw)
    } else {
        data_bounds(&windowed)
    };

    // Expand y bounds to include threshold if it's close to the data range.
    let data_range = (y_max - y_min).abs().max(1e-15);
    let data_center = (y_max + y_min) / 2.0;
    let effective_threshold = threshold.filter(|&thr| (thr - data_center).abs() < data_range * 5.0);
    let (y_min, y_max) = if let Some(thr) = effective_threshold {
        (
            y_min.min(thr - data_range * 0.05),
            y_max.max(thr + data_range * 0.05),
        )
    } else {
        (y_min, y_max)
    };

    let plt_theme = phasma_theme_to_plt(theme);

    let mut plot = LinePlot::new()
        .series(Series::new(title.trim()).data(windowed.to_vec()).color(color))
        .x_axis(PltAxis::new().bounds(Bounds::Manual(x_min, x_max)).grid(show_grid))
        .y_axis(PltAxis::new().bounds(Bounds::Manual(y_min, y_max)).grid(show_grid))
        .title(title)
        .show_legend(true)
        .legend_position(LegendPosition::TopRight)
        .theme(plt_theme);

    // Add dashed threshold line
    if let Some(thr) = threshold {
        plot = plot.reference_line(ReferenceLine::hline_dashed(thr, theme.warn));
    }

    // Add exit event annotation
    if let Some((t, label)) = exit_event {
        if t >= x_min && t <= x_max {
            // Find y value near exit time
            let y_val = windowed
                .iter()
                .min_by_key(|(x, _)| ((x - t).abs() * 1e9) as u64)
                .map(|(_, y)| *y)
                .unwrap_or((y_min + y_max) / 2.0);
            plot = plot.annotation(Annotation::new(label, t, y_val).color(theme.warn));
        }
    }

    frame.render_widget(&plot, area);
}

fn draw_multi_series_windowed(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    series: &[SeriesData<'_>],
    theme: &ThemeColors,
    time_window: &TimeWindow,
    show_grid: bool,
) {
    if series.is_empty() {
        frame.render_widget(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
            area,
        );
        return;
    }

    // Compute bounds over all series
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (_, data, _) in series {
        let (a, b, c, d) = data_bounds(data);
        x_min = x_min.min(a);
        x_max = x_max.max(b);
        y_min = y_min.min(c);
        y_max = y_max.max(d);
    }

    let (x_min, x_max) = time_window.apply(x_min, x_max);

    // Recompute y bounds within window
    let mut wy_min = f64::INFINITY;
    let mut wy_max = f64::NEG_INFINITY;
    for (_, data, _) in series {
        for &(x, y) in *data {
            if x >= x_min && x <= x_max {
                if y < wy_min {
                    wy_min = y;
                }
                if y > wy_max {
                    wy_max = y;
                }
            }
        }
    }
    if wy_min < f64::INFINITY {
        let pad = (wy_max - wy_min) * 0.05;
        y_min = wy_min - pad;
        y_max = wy_max + pad;
    }

    let is_fit_all = time_window.t_end.is_none() && time_window.width.is_none();
    let plt_theme = phasma_theme_to_plt(theme);

    let plt_series: Vec<Series> = series
        .iter()
        .map(|(name, data, color)| {
            let filtered: Vec<(f64, f64)> = if is_fit_all {
                data.to_vec()
            } else {
                data.iter()
                    .copied()
                    .filter(|(x, _)| *x >= x_min && *x <= x_max)
                    .collect()
            };
            Series::new(*name).data(filtered).color(*color)
        })
        .collect();

    let plot = LinePlot::new()
        .series_vec(plt_series)
        .x_axis(PltAxis::new().bounds(Bounds::Manual(x_min, x_max)).grid(show_grid))
        .y_axis(PltAxis::new().bounds(Bounds::Manual(y_min, y_max)).grid(show_grid))
        .title(title)
        .show_legend(true)
        .legend_position(LegendPosition::TopRight)
        .theme(plt_theme);

    frame.render_widget(&plot, area);
}
