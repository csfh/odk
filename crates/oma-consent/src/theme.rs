//! The few colours the dialog needs, read from Omarchy's current theme with fallbacks. This
//! reads `colors.toml` itself: the dialog must not depend on ODK (ADR 0016).

use gpui::{Rgba, rgb};

#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Rgba,
    pub foreground: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub on_accent: Rgba,
    /// `0xRRGGBBAA` for borders.
    pub border: u32,
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
        let background = hex(&table, "background").unwrap_or(0x1a1b26);
        let foreground = hex(&table, "foreground").unwrap_or(0xc0caf5);
        let accent = hex(&table, "accent").unwrap_or(0x7aa2f7);
        Self {
            background: rgb(background),
            foreground: rgb(foreground),
            muted: rgb(hex(&table, "muted").unwrap_or(0x8a8fa8)),
            accent: rgb(accent),
            // Text on the accent: whichever of the two main colours is further from it.
            on_accent: rgb(
                if distance(accent, background) >= distance(accent, foreground) {
                    background
                } else {
                    foreground
                },
            ),
            border: (foreground << 8) | 0x66,
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
