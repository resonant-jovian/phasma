use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Solarized,
    Gruvbox,
}

impl Theme {
    pub fn name(&self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::Solarized => "solarized",
            Theme::Gruvbox => "gruvbox",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Theme::Light,
            "solarized" => Theme::Solarized,
            "gruvbox" => Theme::Gruvbox,
            _ => Theme::Dark,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Solarized,
            Theme::Solarized => Theme::Gruvbox,
            Theme::Gruvbox => Theme::Dark,
        }
    }

    pub fn colors(&self) -> ThemeColors {
        match self {
            Theme::Dark => ThemeColors {
                bg:        Color::Black,
                fg:        Color::White,
                accent:    Color::Cyan,
                border:    Color::Yellow,
                highlight: Color::Rgb(50, 50, 50),
                error:     Color::Red,
                warn:      Color::Yellow,
                ok:        Color::Green,
                dim:       Color::DarkGray,
            },
            Theme::Light => ThemeColors {
                bg:        Color::Rgb(255, 255, 255),
                fg:        Color::Rgb(20, 20, 20),
                accent:    Color::Rgb(0, 60, 180),
                border:    Color::Rgb(80, 80, 80),
                highlight: Color::Rgb(200, 215, 240),
                error:     Color::Rgb(180, 0, 0),
                warn:      Color::Rgb(160, 90, 0),
                ok:        Color::Rgb(0, 120, 0),
                dim:       Color::Rgb(70, 70, 70),
            },
            Theme::Solarized => ThemeColors {
                bg:        Color::Rgb(0, 43, 54),    // base03
                fg:        Color::Rgb(131,148,150),  // base0
                accent:    Color::Rgb(38,139,210),   // blue
                border:    Color::Rgb(101,123,131),  // base01
                highlight: Color::Rgb(7, 54, 66),    // base02
                error:     Color::Rgb(220, 50, 47),  // red
                warn:      Color::Rgb(203,75,22),    // orange
                ok:        Color::Rgb(133,153,0),    // green
                dim:       Color::Rgb(88,110,117),   // base01
            },
            Theme::Gruvbox => ThemeColors {
                bg:        Color::Rgb(40, 40, 40),
                fg:        Color::Rgb(235,219,178),
                accent:    Color::Rgb(131,165,152),
                border:    Color::Rgb(168,153,132),
                highlight: Color::Rgb(60, 56, 54),
                error:     Color::Rgb(204, 36, 29),
                warn:      Color::Rgb(215,153,33),
                ok:        Color::Rgb(152,151,26),
                dim:       Color::Rgb(146,131,116),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub bg:        Color,
    pub fg:        Color,
    pub accent:    Color,
    pub border:    Color,
    pub highlight: Color,
    pub error:     Color,
    pub warn:      Color,
    pub ok:        Color,
    pub dim:       Color,
}
