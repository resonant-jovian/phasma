use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
};

use crate::{data::DataProvider, themes::ThemeColors, tui::action::Action};

type SeriesData<'a> = (&'a str, Vec<(f64, f64)>, Color);

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

pub struct EnergyTab {
    traces: TraceVisibility,
    show_drift: bool,      // show fractional drift or absolute values
    selected_panel: usize, // 0=energy, 1=mass, 2=virial, 3=entropy
    show_grid: bool,
    time_window: TimeWindow,
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
        if data_provider.diagnostics().is_empty() {
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

        // 2×2 grid layout
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        let [top_left, top_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top);

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom);

        self.draw_energy_chart(frame, top_left, theme, data_provider);
        self.draw_mass_chart(frame, top_right, theme, data_provider);
        self.draw_virial_chart(frame, bottom_left, theme, data_provider);
        self.draw_entropy_chart(frame, bottom_right, theme, data_provider);
    }

    fn draw_energy_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let diag = data_provider.diagnostics();

        if self.show_drift {
            let drift_data = diag.energy_drift_series();
            // Add threshold line from exit config
            let threshold = data_provider
                .config()
                .map(|c| c.exit.energy_drift_tolerance)
                .unwrap_or(0.5);
            draw_single_series_with_threshold(
                frame,
                area,
                " ΔE/E₀ ",
                &drift_data,
                theme.chart[3],
                theme,
                Some(threshold),
                &self.time_window,
                self.show_grid,
            );
        } else {
            let mut datasets = Vec::new();
            let e_data = diag.total_energy.iter_chart_data();
            let k_data = diag.kinetic_energy.iter_chart_data();
            let w_data = diag.potential_energy.iter_chart_data();

            if self.traces.total_energy && !e_data.is_empty() {
                datasets.push(("E_tot", e_data, theme.chart[0]));
            }
            if self.traces.kinetic_energy && !k_data.is_empty() {
                datasets.push(("T", k_data, theme.chart[1]));
            }
            if self.traces.potential_energy && !w_data.is_empty() {
                datasets.push(("W", w_data, theme.chart[2]));
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

    fn draw_mass_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let drift = data_provider.diagnostics().mass_drift_series();
        let threshold = data_provider
            .config()
            .map(|c| c.exit.mass_drift_tolerance)
            .unwrap_or(0.1);
        draw_single_series_with_threshold(
            frame,
            area,
            " ΔM/M₀ ",
            &drift,
            theme.chart[4],
            theme,
            Some(threshold),
            &self.time_window,
            self.show_grid,
        );
    }

    fn draw_virial_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let data = data_provider.diagnostics().virial_ratio.iter_chart_data();
        // Reference line at virial=1.0 (virial equilibrium) shown as threshold
        draw_single_series_with_threshold(
            frame,
            area,
            " 2T/|W| (virial) ",
            &data,
            theme.chart[5],
            theme,
            Some(1.0),
            &self.time_window,
            self.show_grid,
        );
    }

    fn draw_entropy_chart(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &ThemeColors,
        data_provider: &dyn DataProvider,
    ) {
        let data = data_provider.diagnostics().entropy.iter_chart_data();
        draw_single_series_with_threshold(
            frame,
            area,
            " S(t) ",
            &data,
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

    // Filter data to window
    let windowed: Vec<(f64, f64)> = data
        .iter()
        .copied()
        .filter(|(x, _)| *x >= x_min && *x <= x_max)
        .collect();
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
        for &(x, y) in data {
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

    let windowed_series: Vec<Vec<(f64, f64)>> = series
        .iter()
        .map(|(_, data, _)| {
            let filtered: Vec<(f64, f64)> = data
                .iter()
                .copied()
                .filter(|(x, _)| *x >= x_min && *x <= x_max)
                .collect();
            densify(&filtered, target_points)
        })
        .collect();

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
    // Add 5% padding
    let ypad = (y_max - y_min) * 0.05;
    (x_min, x_max, y_min - ypad, y_max + ypad)
}

/// Linearly interpolate sparse data so there are at least `target` points.
/// This ensures braille line charts look solid even when the source data
/// is heavily downsampled. Returns the input unchanged if already dense enough.
fn densify(data: &[(f64, f64)], target: usize) -> Vec<(f64, f64)> {
    if data.len() >= target || data.len() < 2 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(target);
    let n_segments = data.len() - 1;
    let points_per_seg = (target / n_segments).max(2);
    for i in 0..n_segments {
        let (x0, y0) = data[i];
        let (x1, y1) = data[i + 1];
        let steps = if i < n_segments - 1 {
            points_per_seg
        } else {
            // Last segment: emit enough to reach target
            target.saturating_sub(out.len()).max(2)
        };
        for j in 0..steps {
            let frac = j as f64 / steps as f64;
            out.push((x0 + frac * (x1 - x0), y0 + frac * (y1 - y0)));
        }
    }
    // Always include the final point
    if let Some(&last) = data.last() {
        out.push(last);
    }
    out
}
