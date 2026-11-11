//! The few colours the dialog needs, read from Omarchy's current theme with fallbacks. This
//! reads `colors.toml` itself: the dialog must not depend on ODK (ADR 0016).

use rune::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub border: Color,
}

/// `0xRRGGBB` as an opaque colour.
fn colour(rgb: u32) -> Color {
    let channel = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.0;
    Color {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: 1.0,
    }
}

fn hex(table: &toml::Table, key: &str) -> Option<u32> {
    let text = table.get(key)?.as_str()?.strip_prefix('#')?;
    if text.len() != 6 {
        return None;
    }
    u32::from_str_radix(text, 16).ok()
}

impl Theme {
    pub fn load() -> Self {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state"))
            });
        let table = state
            .map(|s| s.join("omarchy/current/theme/colors.toml"))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<toml::Table>(&t).ok())
            .unwrap_or_default();
        Self::from_table(&table)
    }

    fn from_table(table: &toml::Table) -> Self {
        let background = hex(table, "background").unwrap_or(0x1a1b26);
        let foreground = hex(table, "foreground").unwrap_or(0xc0caf5);
        let accent = hex(table, "accent").unwrap_or(0x7aa2f7);
        // Text on the accent: whichever of the two main colours is further from it.
        let on_accent = if distance(accent, background) >= distance(accent, foreground) {
            background
        } else {
            foreground
        };
        Self {
            background: colour(background),
            foreground: colour(foreground),
            muted: colour(hex(table, "muted").unwrap_or(0x8a8fa8)),
            accent: colour(accent),
            on_accent: colour(on_accent),
            border: Color {
                a: 0.4,
                ..colour(foreground)
            },
        }
    }
}

fn distance(a: u32, b: u32) -> i32 {
    let channel = |c: u32, shift: u32| ((c >> shift) & 0xff) as i32;
    [16, 8, 0]
        .iter()
        .map(|s| (channel(a, *s) - channel(b, *s)).abs())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_theme_falls_back_to_the_defaults() {
        let theme = Theme::from_table(&toml::Table::new());
        assert_eq!(theme.background.r, colour(0x1a1b26).r);
        assert_eq!(theme.accent.b, colour(0x7aa2f7).b);
    }

    #[test]
    fn a_colour_that_is_not_six_hex_digits_is_ignored() {
        let table: toml::Table = toml::from_str("background = \"#fff\"\naccent = \"red\"").unwrap();
        let theme = Theme::from_table(&table);
        assert_eq!(theme.background.r, colour(0x1a1b26).r);
        assert_eq!(theme.accent.r, colour(0x7aa2f7).r);
    }

    #[test]
    fn text_on_the_accent_is_the_main_colour_further_from_it() {
        let table: toml::Table = toml::from_str(
            "background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#101010\"",
        )
        .unwrap();
        let theme = Theme::from_table(&table);
        assert_eq!(theme.on_accent.r, 1.0, "light text on a dark accent");
        let table: toml::Table = toml::from_str(
            "background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#f0f0f0\"",
        )
        .unwrap();
        assert_eq!(Theme::from_table(&table).on_accent.r, 0.0);
    }
}
