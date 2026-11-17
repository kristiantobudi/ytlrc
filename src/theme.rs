use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub title: Color,
    pub accent: Color,
    pub text: Color,
    pub past: Color,
    pub hint: Color,
    pub border: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub gauge_track: Color,
}

impl Theme {
    /// Returns this theme with every color replaced by the terminal's
    /// default, for `NO_COLOR` environments. Structure (bold/underline/active
    /// vs past) is still conveyed through text modifiers, just not color.
    pub fn no_color(self) -> Self {
        Self {
            name: self.name,
            title: Color::Reset,
            accent: Color::Reset,
            text: Color::Reset,
            past: Color::Reset,
            hint: Color::Reset,
            border: Color::Reset,
            ok: Color::Reset,
            warn: Color::Reset,
            err: Color::Reset,
            gauge_track: Color::Reset,
        }
    }
}

pub const THEMES: [Theme; 5] = [
    Theme {
        name: "Neon",
        title: Color::Rgb(255, 255, 255),
        accent: Color::Rgb(0, 255, 209),
        text: Color::Rgb(205, 214, 214),
        past: Color::Rgb(88, 100, 100),
        hint: Color::Rgb(118, 136, 136),
        border: Color::Rgb(0, 108, 96),
        ok: Color::Rgb(0, 255, 140),
        warn: Color::Rgb(255, 214, 0),
        err: Color::Rgb(255, 92, 92),
        gauge_track: Color::Rgb(44, 62, 62),
    },
    Theme {
        name: "Dracula",
        title: Color::Rgb(248, 248, 242),
        accent: Color::Rgb(189, 147, 249),
        text: Color::Rgb(214, 214, 230),
        past: Color::Rgb(98, 98, 122),
        hint: Color::Rgb(130, 130, 158),
        border: Color::Rgb(68, 71, 90),
        ok: Color::Rgb(80, 250, 123),
        warn: Color::Rgb(241, 250, 140),
        err: Color::Rgb(255, 85, 85),
        gauge_track: Color::Rgb(52, 54, 70),
    },
    Theme {
        name: "Nord",
        title: Color::Rgb(236, 239, 244),
        accent: Color::Rgb(136, 192, 208),
        text: Color::Rgb(216, 222, 233),
        past: Color::Rgb(76, 86, 106),
        hint: Color::Rgb(112, 126, 148),
        border: Color::Rgb(67, 76, 94),
        ok: Color::Rgb(163, 190, 140),
        warn: Color::Rgb(235, 203, 139),
        err: Color::Rgb(191, 97, 106),
        gauge_track: Color::Rgb(59, 66, 82),
    },
    Theme {
        name: "Gruvbox",
        title: Color::Rgb(251, 241, 199),
        accent: Color::Rgb(250, 189, 47),
        text: Color::Rgb(235, 219, 178),
        past: Color::Rgb(124, 111, 100),
        hint: Color::Rgb(146, 131, 116),
        border: Color::Rgb(80, 73, 69),
        ok: Color::Rgb(184, 187, 38),
        warn: Color::Rgb(254, 128, 25),
        err: Color::Rgb(251, 73, 52),
        gauge_track: Color::Rgb(60, 56, 54),
    },
    Theme {
        name: "Mono",
        title: Color::Rgb(255, 255, 255),
        accent: Color::Rgb(255, 255, 255),
        text: Color::Rgb(198, 198, 198),
        past: Color::Rgb(96, 96, 96),
        hint: Color::Rgb(128, 128, 128),
        border: Color::Rgb(70, 70, 70),
        ok: Color::Rgb(255, 255, 255),
        warn: Color::Rgb(205, 205, 205),
        err: Color::Rgb(255, 255, 255),
        gauge_track: Color::Rgb(48, 48, 48),
    },
];

/// `NO_COLOR` (<https://no-color.org>) disables ANSI color output. Ratatui
/// still needs a `Color` value per cell, so "no color" maps every themed
/// color to the terminal's own default (`Color::Reset`) instead of an RGB
/// value — no escape codes are emitted for color, only for the bold/
/// underline modifiers used to keep the active line distinguishable.
pub fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok_and(|value| !value.is_empty())
}

/// `REDUCE_MOTION` (informal convention mirroring the CSS media feature)
/// disables the per-word karaoke bold+underline pulse, falling back to a
/// single flat highlight for the active line.
pub fn reduce_motion() -> bool {
    std::env::var("REDUCE_MOTION").is_ok_and(|value| !value.is_empty())
}

/// Accept a theme name (case-insensitive) or a number 1–5.
pub fn by_name(name: &str) -> Option<usize> {
    let wanted = name.trim().to_ascii_lowercase();
    THEMES
        .iter()
        .position(|theme| theme.name.to_ascii_lowercase() == wanted)
        .or_else(|| {
            wanted
                .parse::<usize>()
                .ok()
                .and_then(|number| number.checked_sub(1))
                .filter(|index| *index < THEMES.len())
        })
}

pub fn names() -> String {
    THEMES
        .iter()
        .enumerate()
        .map(|(index, theme)| format!("{}. {}", index + 1, theme.name))
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_theme_name_and_number() {
        assert_eq!(by_name("dracula"), Some(1));
        assert_eq!(by_name("  NORD "), Some(2));
        assert_eq!(by_name("5"), Some(4));
        assert_eq!(by_name("0"), None);
        assert_eq!(by_name("missing"), None);
    }

    #[test]
    fn themes_have_distinct_accents() {
        for (index, theme) in THEMES.iter().enumerate() {
            for other in &THEMES[index + 1..] {
                assert_ne!(
                    theme.accent, other.accent,
                    "themes {} and {} share the same accent",
                    theme.name, other.name
                );
            }
        }
    }

    #[test]
    fn no_color_variant_resets_every_color_but_keeps_the_name() {
        let theme = THEMES[0].no_color();
        assert_eq!(theme.name, THEMES[0].name);
        assert_eq!(theme.accent, Color::Reset);
        assert_eq!(theme.text, Color::Reset);
        assert_eq!(theme.gauge_track, Color::Reset);
    }
}
