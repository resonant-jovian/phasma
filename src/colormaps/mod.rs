/// Phasma colormap selection enum — used for config, cycling, and serialization.
/// Actual color mapping is handled by ratatui-plt via `plt_bridge::phasma_cmap_to_plt()`.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn default_is_viridis() {
        assert_eq!(Colormap::default(), Colormap::Viridis);
    }
}
