use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Colormap {
    #[default]
    Viridis,
    Inferno,
    Plasma,
    Magma,
    Grayscale,
    Cubehelix,
    Coolwarm,
}

impl Colormap {
    pub fn name(&self) -> &'static str {
        match self {
            Colormap::Viridis => "viridis",
            Colormap::Inferno => "inferno",
            Colormap::Plasma => "plasma",
            Colormap::Magma => "magma",
            Colormap::Grayscale => "grayscale",
            Colormap::Cubehelix => "cubehelix",
            Colormap::Coolwarm => "coolwarm",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "inferno" => Colormap::Inferno,
            "plasma" => Colormap::Plasma,
            "magma" => Colormap::Magma,
            "grayscale" => Colormap::Grayscale,
            "cubehelix" => Colormap::Cubehelix,
            "coolwarm" => Colormap::Coolwarm,
            _ => Colormap::Viridis,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Colormap::Viridis => Colormap::Inferno,
            Colormap::Inferno => Colormap::Plasma,
            Colormap::Plasma => Colormap::Magma,
            Colormap::Magma => Colormap::Grayscale,
            Colormap::Grayscale => Colormap::Cubehelix,
            Colormap::Cubehelix => Colormap::Coolwarm,
            Colormap::Coolwarm => Colormap::Viridis,
        }
    }
}

/// Map normalised t ∈ [0,1] to an RGB color using the given colormap.
pub fn lookup(cmap: Colormap, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = match cmap {
        Colormap::Viridis => viridis(t),
        Colormap::Inferno => inferno(t),
        Colormap::Plasma => plasma(t),
        Colormap::Magma => magma(t),
        Colormap::Grayscale => {
            let v = (t * 255.0) as u8;
            return Color::Rgb(v, v, v);
        }
        Colormap::Cubehelix => cubehelix(t),
        Colormap::Coolwarm => coolwarm(t),
    };
    Color::Rgb(r, g, b)
}

// ── Key-stop tables ──────────────────────────────────────────────────────────
// Each table has 9 entries at t = 0, 0.125, 0.25, ..., 1.0

// Viridis — perceptually uniform, dark-purple → blue → teal → green → yellow
const VIRIDIS_STOPS: [(f64, u8, u8, u8); 9] = [
    (0.000, 68, 1, 84),
    (0.125, 72, 40, 120),
    (0.250, 62, 83, 136),
    (0.375, 49, 113, 142),
    (0.500, 38, 130, 142),
    (0.625, 53, 183, 121),
    (0.750, 110, 206, 88),
    (0.875, 181, 222, 43),
    (1.000, 253, 231, 37),
];

// Inferno — black → dark-purple → red → orange → pale-yellow
const INFERNO_STOPS: [(f64, u8, u8, u8); 9] = [
    (0.000, 0, 0, 4),
    (0.125, 20, 11, 53),
    (0.250, 58, 9, 100),
    (0.375, 96, 19, 110),
    (0.500, 138, 43, 96),
    (0.625, 181, 74, 74),
    (0.750, 224, 117, 44),
    (0.875, 252, 166, 21),
    (1.000, 252, 255, 164),
];

// Plasma — dark-blue → purple → pink → orange → yellow
const PLASMA_STOPS: [(f64, u8, u8, u8); 9] = [
    (0.000, 13, 8, 135),
    (0.125, 84, 2, 163),
    (0.250, 139, 10, 165),
    (0.375, 185, 50, 137),
    (0.500, 219, 92, 104),
    (0.625, 244, 136, 73),
    (0.750, 254, 188, 43),
    (0.875, 252, 229, 35),
    (1.000, 240, 249, 33),
];

// Magma — black → dark-purple → dark-red → orange → pale-cream
const MAGMA_STOPS: [(f64, u8, u8, u8); 9] = [
    (0.000, 0, 0, 4),
    (0.125, 16, 11, 52),
    (0.250, 51, 17, 93),
    (0.375, 90, 22, 109),
    (0.500, 129, 37, 129),
    (0.625, 176, 74, 99),
    (0.750, 218, 121, 73),
    (0.875, 251, 176, 52),
    (1.000, 252, 253, 191),
];

fn interp_stops(stops: &[(f64, u8, u8, u8)], t: f64) -> (u8, u8, u8) {
    // Find the two surrounding stops
    let n = stops.len();
    if t <= stops[0].0 {
        return (stops[0].1, stops[0].2, stops[0].3);
    }
    if t >= stops[n - 1].0 {
        return (stops[n - 1].1, stops[n - 1].2, stops[n - 1].3);
    }
    for i in 0..n - 1 {
        let (t0, r0, g0, b0) = stops[i];
        let (t1, r1, g1, b1) = stops[i + 1];
        if t >= t0 && t <= t1 {
            let s = (t - t0) / (t1 - t0);
            let r = lerp_u8(r0, r1, s);
            let g = lerp_u8(g0, g1, s);
            let b = lerp_u8(b0, b1, s);
            return (r, g, b);
        }
    }
    (255, 255, 255)
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    ((a as f64) + (b as f64 - a as f64) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

// Cubehelix — dark → green-blue → pink → white (Dave Green's cubehelix, start=0.5, rot=-1.5)
const CUBEHELIX_STOPS: [(f64, u8, u8, u8); 9] = [
    (0.000, 0, 0, 0),
    (0.125, 22, 17, 42),
    (0.250, 15, 56, 62),
    (0.375, 28, 98, 47),
    (0.500, 87, 117, 58),
    (0.625, 168, 115, 103),
    (0.750, 196, 130, 182),
    (0.875, 199, 180, 238),
    (1.000, 255, 255, 255),
];

// Coolwarm (diverging) — blue → white → red
const COOLWARM_STOPS: [(f64, u8, u8, u8); 9] = [
    (0.000, 59, 76, 192),
    (0.125, 98, 130, 234),
    (0.250, 141, 176, 254),
    (0.375, 184, 208, 249),
    (0.500, 221, 221, 221),
    (0.625, 245, 196, 173),
    (0.750, 244, 154, 123),
    (0.875, 222, 96, 77),
    (1.000, 180, 4, 38),
];

fn viridis(t: f64) -> (u8, u8, u8) {
    interp_stops(&VIRIDIS_STOPS, t)
}
fn inferno(t: f64) -> (u8, u8, u8) {
    interp_stops(&INFERNO_STOPS, t)
}
fn plasma(t: f64) -> (u8, u8, u8) {
    interp_stops(&PLASMA_STOPS, t)
}
fn magma(t: f64) -> (u8, u8, u8) {
    interp_stops(&MAGMA_STOPS, t)
}
fn cubehelix(t: f64) -> (u8, u8, u8) {
    interp_stops(&CUBEHELIX_STOPS, t)
}
fn coolwarm(t: f64) -> (u8, u8, u8) {
    interp_stops(&COOLWARM_STOPS, t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn from_name_all_variants() {
        assert_eq!(Colormap::from_name("viridis"), Colormap::Viridis);
        assert_eq!(Colormap::from_name("inferno"), Colormap::Inferno);
        assert_eq!(Colormap::from_name("plasma"), Colormap::Plasma);
        assert_eq!(Colormap::from_name("magma"), Colormap::Magma);
        assert_eq!(Colormap::from_name("grayscale"), Colormap::Grayscale);
        assert_eq!(Colormap::from_name("cubehelix"), Colormap::Cubehelix);
        assert_eq!(Colormap::from_name("coolwarm"), Colormap::Coolwarm);
    }

    #[test]
    fn from_name_fallback() {
        assert_eq!(Colormap::from_name("unknown"), Colormap::Viridis);
        assert_eq!(Colormap::from_name(""), Colormap::Viridis);
    }

    #[test]
    fn name_round_trip() {
        let all = [
            Colormap::Viridis,
            Colormap::Inferno,
            Colormap::Plasma,
            Colormap::Magma,
            Colormap::Grayscale,
            Colormap::Cubehelix,
            Colormap::Coolwarm,
        ];
        for c in all {
            assert_eq!(Colormap::from_name(c.name()), c);
        }
    }

    #[test]
    fn next_full_cycle() {
        let start = Colormap::Viridis;
        let end = start.next().next().next().next().next().next().next();
        assert_eq!(start, end);
    }

    #[test]
    fn next_order() {
        assert_eq!(Colormap::Viridis.next(), Colormap::Inferno);
        assert_eq!(Colormap::Inferno.next(), Colormap::Plasma);
        assert_eq!(Colormap::Plasma.next(), Colormap::Magma);
        assert_eq!(Colormap::Magma.next(), Colormap::Grayscale);
        assert_eq!(Colormap::Grayscale.next(), Colormap::Cubehelix);
        assert_eq!(Colormap::Cubehelix.next(), Colormap::Coolwarm);
        assert_eq!(Colormap::Coolwarm.next(), Colormap::Viridis);
    }

    #[test]
    fn lookup_viridis_start() {
        assert_eq!(lookup(Colormap::Viridis, 0.0), Color::Rgb(68, 1, 84));
    }

    #[test]
    fn lookup_viridis_end() {
        assert_eq!(lookup(Colormap::Viridis, 1.0), Color::Rgb(253, 231, 37));
    }

    #[test]
    fn lookup_clamps_negative() {
        assert_eq!(lookup(Colormap::Viridis, -1.0), lookup(Colormap::Viridis, 0.0));
    }

    #[test]
    fn lookup_clamps_above_one() {
        assert_eq!(lookup(Colormap::Viridis, 2.0), lookup(Colormap::Viridis, 1.0));
    }

    #[test]
    fn grayscale_endpoints() {
        assert_eq!(lookup(Colormap::Grayscale, 0.0), Color::Rgb(0, 0, 0));
        assert_eq!(lookup(Colormap::Grayscale, 1.0), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn coolwarm_midpoint() {
        assert_eq!(lookup(Colormap::Coolwarm, 0.5), Color::Rgb(221, 221, 221));
    }

    #[test]
    fn coolwarm_endpoints() {
        assert_eq!(lookup(Colormap::Coolwarm, 0.0), Color::Rgb(59, 76, 192));
        assert_eq!(lookup(Colormap::Coolwarm, 1.0), Color::Rgb(180, 4, 38));
    }

    #[test]
    fn all_midpoint_no_panic() {
        let all = [
            Colormap::Viridis,
            Colormap::Inferno,
            Colormap::Plasma,
            Colormap::Magma,
            Colormap::Grayscale,
            Colormap::Cubehelix,
            Colormap::Coolwarm,
        ];
        for c in all {
            let _ = lookup(c, 0.5);
        }
    }

    #[test]
    fn default_is_viridis() {
        assert_eq!(Colormap::default(), Colormap::Viridis);
    }
}
