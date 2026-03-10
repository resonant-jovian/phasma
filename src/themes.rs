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
                bg: Color::Black,
                fg: Color::White,
                accent: Color::Cyan,
                border: Color::Yellow,
                highlight: Color::Rgb(50, 50, 50),
                error: Color::Red,
                warn: Color::Yellow,
                ok: Color::Green,
                dim: Color::DarkGray,
                chart: [
                    Color::Cyan,
                    Color::Green,
                    Color::Magenta,
                    Color::Red,
                    Color::Yellow,
                    Color::LightBlue,
                    Color::LightGreen,
                ],
            },
            Theme::Light => ThemeColors {
                bg: Color::Rgb(255, 255, 255),
                fg: Color::Rgb(10, 10, 10),
                accent: Color::Rgb(0, 50, 160),
                border: Color::Rgb(50, 50, 50),
                highlight: Color::Rgb(210, 225, 245),
                error: Color::Rgb(170, 0, 0),
                warn: Color::Rgb(140, 70, 0),
                ok: Color::Rgb(0, 100, 0),
                dim: Color::Rgb(90, 90, 90),
                chart: [
                    Color::Rgb(0, 80, 160),  // dark blue
                    Color::Rgb(0, 120, 0),   // dark green
                    Color::Rgb(140, 0, 140), // dark magenta
                    Color::Rgb(200, 0, 0),   // dark red
                    Color::Rgb(170, 100, 0), // dark amber
                    Color::Rgb(0, 100, 140), // dark teal
                    Color::Rgb(60, 130, 0),  // dark lime
                ],
            },
            Theme::Solarized => ThemeColors {
                bg: Color::Rgb(0, 43, 54),         // base03
                fg: Color::Rgb(131, 148, 150),     // base0
                accent: Color::Rgb(38, 139, 210),  // blue
                border: Color::Rgb(101, 123, 131), // base01
                highlight: Color::Rgb(7, 54, 66),  // base02
                error: Color::Rgb(220, 50, 47),    // red
                warn: Color::Rgb(203, 75, 22),     // orange
                ok: Color::Rgb(133, 153, 0),       // green
                dim: Color::Rgb(88, 110, 117),     // base01
                chart: [
                    Color::Rgb(38, 139, 210), // blue
                    Color::Rgb(133, 153, 0),  // green
                    Color::Rgb(211, 54, 130), // magenta
                    Color::Rgb(220, 50, 47),  // red
                    Color::Rgb(181, 137, 0),  // yellow
                    Color::Rgb(42, 161, 152), // cyan
                    Color::Rgb(203, 75, 22),  // orange
                ],
            },
            Theme::Gruvbox => ThemeColors {
                bg: Color::Rgb(40, 40, 40),
                fg: Color::Rgb(235, 219, 178),
                accent: Color::Rgb(131, 165, 152),
                border: Color::Rgb(168, 153, 132),
                highlight: Color::Rgb(60, 56, 54),
                error: Color::Rgb(204, 36, 29),
                warn: Color::Rgb(215, 153, 33),
                ok: Color::Rgb(152, 151, 26),
                dim: Color::Rgb(146, 131, 116),
                chart: [
                    Color::Rgb(131, 165, 152), // aqua
                    Color::Rgb(184, 187, 38),  // green
                    Color::Rgb(211, 134, 155), // purple
                    Color::Rgb(251, 73, 52),   // red
                    Color::Rgb(250, 189, 47),  // yellow
                    Color::Rgb(69, 133, 136),  // teal
                    Color::Rgb(254, 128, 25),  // orange
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub border: Color,
    pub highlight: Color,
    pub error: Color,
    pub warn: Color,
    pub ok: Color,
    pub dim: Color,
    /// Chart trace palette — 7 distinguishable colors tuned for the background.
    pub chart: [Color; 7],
}
