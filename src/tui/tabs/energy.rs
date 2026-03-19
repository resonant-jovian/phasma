use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use std::borrow::Cow;

use crate::{
    data::DataProvider,
    themes::ThemeColors,
    tui::action::Action,
    tui::chart_utils::{data_bounds, densify},
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
    cached_at_len: usize,
}

pub struct EnergyTab {
    traces: TraceVisibility,
    show_drift: bool,      // show fractional drift or absolute values
    selected_panel: usize, // 0=energy, 1=mass, 2=virial, 3=entropy
    show_grid: bool,
    time_window: TimeWindow,
    cached: CachedSeries,
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
            time_window: TimeWindow::default(),
            cached: CachedSeries::default(),
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

    pub fn update(&mut self, _action: &Action) -> Option<Action> {
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
                cached_at_len: current_len,
            };
        }

        // Compact mode: single panel (selected by panel key 1-4)
        if area.width < 76 {
            match self.selected_panel {
                0 => self.draw_energy_chart(frame, area, theme, data_provider),
                1 => self.draw_mass_chart(frame, area, theme),
                2 => self.draw_virial_chart(frame, area, theme),
                _ => self.draw_entropy_chart(frame, area, theme),
            }
            return;
        }

        // 2×2 grid layout
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
        self.draw_entropy_chart(frame, bottom_right, theme);
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
            );
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
        );
    }
}

/// Generate evenly-spaced label strings between `lo` and `hi`.
/// `n` is the total number of labels (including endpoints).
fn grid_labels(lo: f64, hi: f64, n: usize, scientific: bool) -> Vec<String> {
    if n <= 1 {
        let val = (lo + hi) * 0.5;
        return vec![if scientific {
            format!("{val:.2e}")
        } else {
            format!("{val:.2}")
        }];
    }
    (0..n)
        .map(|i| {
            let frac = i as f64 / (n - 1) as f64;
            let val = lo + frac * (hi - lo);
            if scientific {
                format!("{val:.2e}")
            } else {
                format!("{val:.2}")
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
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
    // If threshold is more than 5x the data range away from data center, suppress
    // it from Y-bounds so it doesn't squash the actual data to a thin line.
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

    let chart_width = area.width.saturating_sub(2) as usize;
    let dense = densify(&windowed, chart_width * 2);

    let mut datasets = vec![
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(color))
            .data(&dense),
    ];

    // Add dashed threshold line
    let threshold_data: Vec<(f64, f64)>;
    if let Some(thr) = threshold {
        threshold_data = vec![(x_min, thr), (x_max, thr)];
        datasets.push(
            Dataset::default()
                .name(format!("thr={thr:.2e}"))
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(theme.warn))
                .data(&threshold_data),
        );
    }

    let x_labels = if show_grid {
        grid_labels(x_min, x_max, 5, false)
    } else {
        vec![format!("{x_min:.2}"), format!("{x_max:.2}")]
    };
    let y_labels = if show_grid {
        grid_labels(y_min, y_max, 5, true)
    } else {
        vec![format!("{y_min:.2e}"), format!("{y_max:.2e}")]
    };

    let chart = Chart::new(datasets)
        .block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
        )
        .x_axis(
            Axis::default()
                .bounds([x_min, x_max])
                .labels(x_labels)
                .style(Style::default().fg(theme.dim)),
        )
        .y_axis(
            Axis::default()
                .bounds([y_min, y_max])
                .labels(y_labels)
                .style(Style::default().fg(theme.dim)),
        );

    frame.render_widget(chart, area);
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

    let chart_width = area.width.saturating_sub(2) as usize;
    let target_points = chart_width * 2;
    let is_fit_all = time_window.t_end.is_none() && time_window.width.is_none();

    // In fit-all mode, densify directly from source data (avoids filter allocation).
    // In windowed mode, filter first then densify.
    let filter_storage: Vec<Vec<(f64, f64)>> = if is_fit_all {
        Vec::new()
    } else {
        series
            .iter()
            .map(|(_, data, _)| {
                data.iter()
                    .copied()
                    .filter(|(x, _)| *x >= x_min && *x <= x_max)
                    .collect()
            })
            .collect()
    };

    let windowed_series: Vec<Cow<'_, [(f64, f64)]>> = if is_fit_all {
        series
            .iter()
            .map(|(_, data, _)| densify(data, target_points))
            .collect()
    } else {
        filter_storage
            .iter()
            .map(|filtered| densify(filtered, target_points))
            .collect()
    };

    let datasets: Vec<Dataset> = series
        .iter()
        .zip(windowed_series.iter())
        .map(|((name, _, color), dense)| {
            Dataset::default()
                .name(*name)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(*color))
                .data(dense)
        })
        .collect();

    let x_labels = if show_grid {
        grid_labels(x_min, x_max, 5, false)
    } else {
        vec![format!("{x_min:.2}"), format!("{x_max:.2}")]
    };
    let y_labels = if show_grid {
        grid_labels(y_min, y_max, 5, true)
    } else {
        vec![format!("{y_min:.2e}"), format!("{y_max:.2e}")]
    };

    let chart = Chart::new(datasets)
        .block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(theme.border)),
        )
        .x_axis(
            Axis::default()
                .bounds([x_min, x_max])
                .labels(x_labels)
                .style(Style::default().fg(theme.dim)),
        )
        .y_axis(
            Axis::default()
                .bounds([y_min, y_max])
                .labels(y_labels)
                .style(Style::default().fg(theme.dim)),
        );

    frame.render_widget(chart, area);
}
