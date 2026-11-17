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

/// Terima nama theme (tidak peka huruf besar/kecil) atau nomor 1–5.
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
        assert_eq!(by_name("tidak-ada"), None);
    }

    #[test]
    fn themes_have_distinct_accents() {
        for (index, theme) in THEMES.iter().enumerate() {
            for other in &THEMES[index + 1..] {
                assert_ne!(
                    theme.accent, other.accent,
                    "tema {} dan {} punya aksen sama",
                    theme.name, other.name
                );
            }
        }
    }
}
