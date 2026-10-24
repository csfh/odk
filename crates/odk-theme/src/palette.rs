//! The source layer of ADR 0040: a theme's `colors.toml`, by Omarchy's names and with the values
//! the theme wrote (`docs/spec/odk-source-palette.md`). Nothing here gives a colour a meaning.

use std::fs;
use std::path::Path;

use toml::{Table, Value};

use crate::{Color, Error};

/// Whether the theme is dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

impl Mode {
    /// The value `colors.toml` writes: `"dark"` or `"light"`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// One colour key of `colors.toml`, for diagnostics and for going through every key. Code that
/// draws reads the fields of [`Palette`], never a key by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum PaletteKey {
    Accent,
    Selection,
    SelectionBackground,
    SelectionForeground,
    Muted,
    Background,
    DarkBackground,
    DarkerBackground,
    LighterBackground,
    Foreground,
    DarkForeground,
    LightForeground,
    BrightForeground,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Magenta,
    Brown,
    BrightRed,
    BrightYellow,
    BrightGreen,
    BrightCyan,
    BrightBlue,
    BrightMagenta,
}

impl PaletteKey {
    /// Every key, in the order Omarchy's stock themes write them.
    pub const ALL: [Self; 27] = [
        Self::Accent,
        Self::Selection,
        Self::SelectionBackground,
        Self::SelectionForeground,
        Self::Muted,
        Self::Background,
        Self::DarkBackground,
        Self::DarkerBackground,
        Self::LighterBackground,
        Self::Foreground,
        Self::DarkForeground,
        Self::LightForeground,
        Self::BrightForeground,
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Cyan,
        Self::Blue,
        Self::Magenta,
        Self::Brown,
        Self::BrightRed,
        Self::BrightYellow,
        Self::BrightGreen,
        Self::BrightCyan,
        Self::BrightBlue,
        Self::BrightMagenta,
    ];

    /// The key as `colors.toml` writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Accent => "accent",
            Self::Selection => "selection",
            Self::SelectionBackground => "selection_background",
            Self::SelectionForeground => "selection_foreground",
            Self::Muted => "muted",
            Self::Background => "background",
            Self::DarkBackground => "dark_background",
            Self::DarkerBackground => "darker_background",
            Self::LighterBackground => "lighter_background",
            Self::Foreground => "foreground",
            Self::DarkForeground => "dark_foreground",
            Self::LightForeground => "light_foreground",
            Self::BrightForeground => "bright_foreground",
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
            Self::Magenta => "magenta",
            Self::Brown => "brown",
            Self::BrightRed => "bright_red",
            Self::BrightYellow => "bright_yellow",
            Self::BrightGreen => "bright_green",
            Self::BrightCyan => "bright_cyan",
            Self::BrightBlue => "bright_blue",
            Self::BrightMagenta => "bright_magenta",
        }
    }

    /// The key a name stands for: its long name, or the short name of [`PaletteKey::alias`].
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|key| key.name() == name || key.alias() == Some(name))
    }

    /// The short name some themes write instead, such as Aether's `lighter_bg`. Omarchy's own
    /// resolver (`omarchy-theme-color`) reads the same eight, and so does ODK; when a theme
    /// writes both names, the long one wins.
    pub const fn alias(self) -> Option<&'static str> {
        match self {
            Self::Background => Some("bg"),
            Self::DarkBackground => Some("dark_bg"),
            Self::DarkerBackground => Some("darker_bg"),
            Self::LighterBackground => Some("lighter_bg"),
            Self::Foreground => Some("fg"),
            Self::DarkForeground => Some("dark_fg"),
            Self::LightForeground => Some("light_fg"),
            Self::BrightForeground => Some("bright_fg"),
            _ => None,
        }
    }

    /// Whether a theme must define this key. The required keys are the ones ODK cannot draw
    /// without; every stock Omarchy theme defines them.
    pub const fn is_required(self) -> bool {
        matches!(
            self,
            Self::Accent
                | Self::Muted
                | Self::Background
                | Self::DarkBackground
                | Self::LighterBackground
                | Self::Foreground
                | Self::Red
                | Self::Yellow
                | Self::Green
                | Self::Blue
        )
    }
}

/// A theme's `colors.toml` as the theme wrote it: every colour exactly as parsed, and `None` for
/// each optional key the theme leaves out. Nothing is derived or filled in here: choosing what
/// stands in for a missing colour, or which colour plays which role, is a later layer's job.
///
/// Keys ODK does not know are ignored, so a theme written for a newer Omarchy still loads.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub mode: Mode,

    pub accent: Color,
    pub selection: Option<Color>,
    pub selection_background: Option<Color>,
    pub selection_foreground: Option<Color>,
    pub muted: Color,

    pub background: Color,
    pub dark_background: Color,
    pub darker_background: Option<Color>,
    pub lighter_background: Color,

    pub foreground: Color,
    pub dark_foreground: Option<Color>,
    pub light_foreground: Option<Color>,
    pub bright_foreground: Option<Color>,

    pub red: Color,
    pub orange: Option<Color>,
    pub yellow: Color,
    pub green: Color,
    pub cyan: Option<Color>,
    pub blue: Color,
    pub magenta: Option<Color>,
    pub brown: Option<Color>,

    pub bright_red: Option<Color>,
    pub bright_yellow: Option<Color>,
    pub bright_green: Option<Color>,
    pub bright_cyan: Option<Color>,
    pub bright_blue: Option<Color>,
    pub bright_magenta: Option<Color>,
}

impl Palette {
    /// Reads and parses a `colors.toml`. A file that is missing or cannot be read is
    /// [`Error::Io`], naming the path.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Parses the text of a `colors.toml`. Every key ODK knows must hold a `#RRGGBB` or
    /// `#RRGGBBAA` string, whether it is required or not; the first that does not is an error
    /// naming it.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let table: Table = toml::from_str(text).map_err(|source| Error::Toml {
            file: "colors.toml",
            source,
        })?;
        let mut colours = [None; PaletteKey::ALL.len()];
        for (slot, key) in colours.iter_mut().zip(PaletteKey::ALL) {
            *slot = read(&table, key)?;
        }
        let get = |key: PaletteKey| colours[key as usize];
        let required = |key: PaletteKey| get(key).ok_or(Error::MissingKey(key.name()));
        use PaletteKey as K;
        Ok(Self {
            mode: mode(&table)?,
            accent: required(K::Accent)?,
            selection: get(K::Selection),
            selection_background: get(K::SelectionBackground),
            selection_foreground: get(K::SelectionForeground),
            muted: required(K::Muted)?,
            background: required(K::Background)?,
            dark_background: required(K::DarkBackground)?,
            darker_background: get(K::DarkerBackground),
            lighter_background: required(K::LighterBackground)?,
            foreground: required(K::Foreground)?,
            dark_foreground: get(K::DarkForeground),
            light_foreground: get(K::LightForeground),
            bright_foreground: get(K::BrightForeground),
            red: required(K::Red)?,
            orange: get(K::Orange),
            yellow: required(K::Yellow)?,
            green: required(K::Green)?,
            cyan: get(K::Cyan),
            blue: required(K::Blue)?,
            magenta: get(K::Magenta),
            brown: get(K::Brown),
            bright_red: get(K::BrightRed),
            bright_yellow: get(K::BrightYellow),
            bright_green: get(K::BrightGreen),
            bright_cyan: get(K::BrightCyan),
            bright_blue: get(K::BrightBlue),
            bright_magenta: get(K::BrightMagenta),
        })
    }

    /// The colour under `key`, or `None` when the theme leaves that optional key out.
    pub fn get(&self, key: PaletteKey) -> Option<Color> {
        use PaletteKey as K;
        match key {
            K::Accent => Some(self.accent),
            K::Selection => self.selection,
            K::SelectionBackground => self.selection_background,
            K::SelectionForeground => self.selection_foreground,
            K::Muted => Some(self.muted),
            K::Background => Some(self.background),
            K::DarkBackground => Some(self.dark_background),
            K::DarkerBackground => self.darker_background,
            K::LighterBackground => Some(self.lighter_background),
            K::Foreground => Some(self.foreground),
            K::DarkForeground => self.dark_foreground,
            K::LightForeground => self.light_foreground,
            K::BrightForeground => self.bright_foreground,
            K::Red => Some(self.red),
            K::Orange => self.orange,
            K::Yellow => Some(self.yellow),
            K::Green => Some(self.green),
            K::Cyan => self.cyan,
            K::Blue => Some(self.blue),
            K::Magenta => self.magenta,
            K::Brown => self.brown,
            K::BrightRed => self.bright_red,
            K::BrightYellow => self.bright_yellow,
            K::BrightGreen => self.bright_green,
            K::BrightCyan => self.bright_cyan,
            K::BrightBlue => self.bright_blue,
            K::BrightMagenta => self.bright_magenta,
        }
    }

    /// The colours the theme defines, with their keys, in [`PaletteKey::ALL`]'s order.
    pub fn colours(&self) -> impl Iterator<Item = (PaletteKey, Color)> + '_ {
        PaletteKey::ALL
            .into_iter()
            .filter_map(|key| Some((key, self.get(key)?)))
    }

    /// The optional keys the theme leaves out.
    pub fn missing(&self) -> impl Iterator<Item = PaletteKey> + '_ {
        PaletteKey::ALL
            .into_iter()
            .filter(|key| self.get(*key).is_none())
    }
}

/// The colour under `key`, else under its alias; `None` when the theme has neither.
fn read(table: &Table, key: PaletteKey) -> Result<Option<Color>, Error> {
    let found = match (table.get(key.name()), key.alias()) {
        (Some(value), _) => Some((key.name(), value)),
        (None, Some(alias)) => table.get(alias).map(|value| (alias, value)),
        (None, None) => None,
    };
    let Some((name, value)) = found else {
        return Ok(None);
    };
    let text = string(name, value)?;
    Color::parse_hex(text)
        .map(Some)
        .map_err(|source| Error::BadColor {
            key: name,
            value: text.to_owned(),
            source,
        })
}

fn mode(table: &Table) -> Result<Mode, Error> {
    let value = table.get("mode").ok_or(Error::MissingKey("mode"))?;
    match string("mode", value)? {
        "dark" => Ok(Mode::Dark),
        "light" => Ok(Mode::Light),
        other => Err(Error::BadMode(other.to_owned())),
    }
}

fn string<'a>(key: &'static str, value: &'a Value) -> Result<&'a str, Error> {
    value.as_str().ok_or(Error::WrongType {
        key,
        found: value.type_str(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorError;

    /// The required keys only.
    const MINIMAL: &str = r##"
mode = "dark"
accent = "#7aa2f7"
muted = "#414868"
background = "#1a1b26"
dark_background = "#13141c"
lighter_background = "#24283b"
foreground = "#a9b1d6"
red = "#f7768e"
yellow = "#e0af68"
green = "#9ece6a"
blue = "#7aa2f7"
"##;

    fn hex(s: &str) -> Color {
        Color::parse_hex(s).unwrap()
    }

    fn without(text: &str, key: &str) -> String {
        text.lines()
            .filter(|l| !l.starts_with(&format!("{key} ")))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn keys_have_omarchy_s_names_and_the_right_positions() {
        for (i, key) in PaletteKey::ALL.into_iter().enumerate() {
            assert_eq!(key as usize, i, "{key:?}");
        }
        let names: std::collections::HashSet<_> = PaletteKey::ALL.map(PaletteKey::name).into();
        assert_eq!(names.len(), PaletteKey::ALL.len(), "names are unique");
        assert_eq!(PaletteKey::BrightMagenta.name(), "bright_magenta");
        assert_eq!(PaletteKey::LighterBackground.alias(), Some("lighter_bg"));
        assert_eq!(PaletteKey::Red.alias(), None);
        for key in PaletteKey::ALL {
            assert_eq!(PaletteKey::from_name(key.name()), Some(key));
        }
        assert_eq!(
            PaletteKey::from_name("lighter_bg"),
            Some(PaletteKey::LighterBackground)
        );
        assert_eq!(PaletteKey::from_name("purple"), None);
        assert_eq!(PaletteKey::from_name("mode"), None);
    }

    #[test]
    fn a_minimal_theme_loads_with_every_optional_key_missing() {
        let p = Palette::parse(MINIMAL).unwrap();
        assert_eq!(p.mode, Mode::Dark);
        assert_eq!(p.background, hex("#1a1b26"));
        assert_eq!(p.colours().count(), 10);
        let missing: Vec<_> = p.missing().collect();
        assert_eq!(missing.len(), 17);
        assert!(missing.iter().all(|k| !k.is_required()));
        assert_eq!(p.bright_red, None);
        assert_eq!(p.selection_foreground, None);
    }

    #[test]
    fn every_required_key_is_required() {
        for key in PaletteKey::ALL.into_iter().filter(|k| k.is_required()) {
            assert!(
                matches!(Palette::parse(&without(MINIMAL, key.name())), Err(Error::MissingKey(k)) if k == key.name()),
                "{key:?}"
            );
        }
        assert!(matches!(
            Palette::parse(&without(MINIMAL, "mode")),
            Err(Error::MissingKey("mode"))
        ));
    }

    #[test]
    fn a_missing_optional_key_is_none_and_a_present_one_is_kept() {
        let with = format!("{MINIMAL}bright_red = \"#ff7a93\"\n");
        let p = Palette::parse(&with).unwrap();
        assert_eq!(p.bright_red, Some(hex("#ff7a93")));
        assert_eq!(p.get(PaletteKey::BrightRed), Some(hex("#ff7a93")));
        assert_eq!(p.bright_green, None);
        assert!(!p.missing().any(|k| k == PaletteKey::BrightRed));
    }

    #[test]
    fn unknown_keys_are_ignored_whatever_they_hold() {
        let extra = format!(
            "{MINIMAL}hyprland_active_border = \"rgba(26a269ee) rgba(2ec27eee) 45deg\"\nfuture = 3\n[section]\nx = 1\n"
        );
        assert_eq!(
            Palette::parse(&extra).unwrap(),
            Palette::parse(MINIMAL).unwrap()
        );
    }

    #[test]
    fn a_malformed_colour_is_an_error_naming_its_key_even_when_optional() {
        let bad = MINIMAL.replace("\"#f7768e\"", "\"#f7768\"");
        assert!(matches!(
            Palette::parse(&bad),
            Err(Error::BadColor { key: "red", ref value, source: ColorError::Length(5) }) if value == "#f7768"
        ));
        let bad_optional = format!("{MINIMAL}bright_cyan = \"cyan\"\n");
        let err = Palette::parse(&bad_optional).unwrap_err();
        assert!(matches!(
            err,
            Error::BadColor {
                key: "bright_cyan",
                source: ColorError::NoHash,
                ..
            }
        ));
        assert_eq!(
            err.to_string(),
            "colors.toml `bright_cyan` is \"cyan\", not a colour: a colour is written #RRGGBB or #RRGGBBAA, starting with `#`"
        );
    }

    #[test]
    fn a_value_of_the_wrong_type_is_its_own_error() {
        let number = MINIMAL.replace("red = \"#f7768e\"", "red = 0xf7768e");
        assert!(matches!(
            Palette::parse(&number),
            Err(Error::WrongType {
                key: "red",
                found: "integer"
            })
        ));
        let array = MINIMAL.replace("mode = \"dark\"", "mode = [\"dark\"]");
        assert!(matches!(
            Palette::parse(&array),
            Err(Error::WrongType {
                key: "mode",
                found: "array"
            })
        ));
        let table = format!("{MINIMAL}[bright_red]\nvalue = \"#ff0000\"\n");
        assert!(matches!(
            Palette::parse(&table),
            Err(Error::WrongType {
                key: "bright_red",
                found: "table"
            })
        ));
    }

    #[test]
    fn an_unknown_mode_is_refused() {
        let sepia = MINIMAL.replace("\"dark\"", "\"sepia\"");
        assert!(matches!(Palette::parse(&sepia), Err(Error::BadMode(m)) if m == "sepia"));
        let shouting = MINIMAL.replace("\"dark\"", "\"Dark\"");
        assert!(matches!(Palette::parse(&shouting), Err(Error::BadMode(_))));
        let light = MINIMAL.replace("\"dark\"", "\"light\"");
        assert_eq!(Palette::parse(&light).unwrap().mode, Mode::Light);
    }

    #[test]
    fn invalid_toml_names_the_file() {
        assert!(matches!(
            Palette::parse("mode = \"dark"),
            Err(Error::Toml {
                file: "colors.toml",
                ..
            })
        ));
    }

    #[test]
    fn short_names_are_read_and_the_long_name_wins() {
        let short = MINIMAL
            .replace("dark_background =", "dark_bg =")
            .replace("lighter_background =", "lighter_bg =")
            .replace("background =", "bg =")
            .replace("foreground =", "fg =")
            + "darker_bg = \"#0e0e14\"\nbright_fg = \"#c0caf5\"\n";
        let p = Palette::parse(&short).unwrap();
        let long = Palette::parse(MINIMAL).unwrap();
        assert_eq!(p.background, long.background);
        assert_eq!(p.lighter_background, long.lighter_background);
        assert_eq!(p.foreground, long.foreground);
        assert_eq!(p.darker_background, Some(hex("#0e0e14")));
        assert_eq!(p.bright_foreground, Some(hex("#c0caf5")));

        let both = format!("{MINIMAL}lighter_bg = \"#ff00ff\"\n");
        assert_eq!(
            Palette::parse(&both).unwrap().lighter_background,
            long.lighter_background
        );
        // The alias is not read when the long name is there, so it is not checked either.
        let both_bad = format!("{MINIMAL}lighter_bg = \"oops\"\n");
        assert!(Palette::parse(&both_bad).is_ok());
        // An error in a value written under the alias names the alias, where it must be fixed.
        let bad_short = short.replace("bg = \"#1a1b26\"", "bg = \"#1a1b2\"");
        assert!(matches!(
            Palette::parse(&bad_short),
            Err(Error::BadColor { key: "bg", .. })
        ));
    }

    #[test]
    fn eight_digit_colours_keep_their_alpha() {
        let alpha = format!("{MINIMAL}selection = \"#292e4280\"\n");
        let p = Palette::parse(&alpha).unwrap();
        assert_eq!(p.selection.unwrap().to_hex(), "#292e4280");
    }

    #[test]
    fn load_names_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("colors.toml");
        match Palette::load(&path) {
            Err(Error::Io { path: p, source }) => {
                assert_eq!(p, path);
                assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("{other:?}"),
        }
        fs::write(&path, MINIMAL).unwrap();
        assert_eq!(
            Palette::load(&path).unwrap(),
            Palette::parse(MINIMAL).unwrap()
        );
    }
}
