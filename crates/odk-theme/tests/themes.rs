//! The source palette on real theme files: Omarchy's stock themes as fixtures, and two custom
//! themes (`tests/fixtures/README.md`).

use std::fs;
use std::path::{Path, PathBuf};

use odk_theme::{Color, ColorError, Error, Mode, Palette, PaletteKey};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("colors.toml")
}

fn load(name: &str) -> Palette {
    Palette::load(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The value of every `key = "#..."` line, read without TOML or ODK, to check the palette
/// against.
fn written(name: &str) -> Vec<(String, String)> {
    fs::read_to_string(fixture(name))
        .unwrap()
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
            value
                .starts_with('#')
                .then(|| (key.trim().to_owned(), value.to_owned()))
        })
        .collect()
}

const STOCK: [&str; 5] = [
    "tokyo-night",
    "nord",
    "catppuccin",
    "catppuccin-latte",
    "vantablack",
];

#[test]
fn every_stock_fixture_defines_every_key_and_keeps_its_values() {
    for name in STOCK {
        let palette = load(name);
        assert_eq!(palette.colours().count(), 25, "{name}");
        // Omarchy's stock themes write no selection_background or selection_foreground.
        assert_eq!(
            palette.missing().collect::<Vec<_>>(),
            [
                PaletteKey::SelectionBackground,
                PaletteKey::SelectionForeground
            ],
            "{name}"
        );
        let lines = written(name);
        assert_eq!(lines.len(), 25, "{name}");
        for (key, value) in lines {
            let key = PaletteKey::ALL
                .into_iter()
                .find(|k| k.name() == key)
                .unwrap_or_else(|| panic!("{name}: unknown key {key}"));
            let colour = palette.get(key).unwrap();
            assert_eq!(
                colour.to_hex(),
                value.to_ascii_lowercase(),
                "{name} {key:?}"
            );
        }
    }
}

#[test]
fn tokyo_night_is_read_exactly() {
    let p = load("tokyo-night");
    assert_eq!(p.mode, Mode::Dark);
    let hex = |c: Color| c.to_hex();
    assert_eq!(hex(p.accent), "#7aa2f7");
    assert_eq!(hex(p.background), "#1a1b26");
    assert_eq!(p.darker_background.map(hex).as_deref(), Some("#0e0e14"));
    assert_eq!(hex(p.foreground), "#a9b1d6");
    assert_eq!(hex(p.red), "#f7768e");
    assert_eq!(p.brown.map(hex).as_deref(), Some("#75493d"));
    assert_eq!(p.bright_cyan.map(hex).as_deref(), Some("#0db9d7"));
    assert_eq!(p.red.to_rgba8(), [0xf7, 0x76, 0x8e, 0xff]);
}

/// Tokyo Night's bright colours differ from its regular ones; Catppuccin Latte's are the same.
/// Neither is changed to look more like the other.
#[test]
fn regular_and_bright_colours_stay_as_the_theme_wrote_them() {
    let pairs = |p: &Palette| {
        [
            (p.red, p.bright_red),
            (p.yellow, p.bright_yellow),
            (p.green, p.bright_green),
            (p.blue, p.bright_blue),
        ]
        .map(|(regular, bright)| (regular, bright.unwrap()))
    };
    for (regular, bright) in pairs(&load("tokyo-night")) {
        assert_ne!(regular, bright);
    }
    for (regular, bright) in pairs(&load("catppuccin-latte")) {
        assert_eq!(regular, bright);
    }
    let latte = load("catppuccin-latte");
    assert_eq!(latte.cyan, latte.bright_cyan);
    assert_eq!(latte.magenta, latte.bright_magenta);
}

/// Bright is a name, not a promise: in Tokyo Night `bright_yellow` is orange, darker than
/// `yellow` by luminance.
#[test]
fn a_bright_colour_need_not_be_brighter() {
    let p = load("tokyo-night");
    assert!(p.bright_yellow.unwrap().luminance() < p.yellow.luminance());
}

#[test]
fn vantablack_stays_monochrome() {
    let p = load("vantablack");
    for (key, c) in p.colours() {
        assert!(c.r == c.g && c.g == c.b, "{key:?} is {}", c.to_hex());
    }
}

#[test]
fn light_themes_are_light() {
    assert_eq!(load("catppuccin-latte").mode, Mode::Light);
    for name in ["tokyo-night", "nord", "catppuccin", "vantablack"] {
        assert_eq!(load(name).mode, Mode::Dark, "{name}");
    }
}

#[test]
fn catppuccin_is_mocha() {
    let p = load("catppuccin");
    assert_eq!(p.background.to_hex(), "#1e1e2e");
    assert_eq!(p.foreground.to_hex(), "#cdd6f4");
}

#[test]
fn an_incomplete_custom_theme_loads_with_its_gaps_explicit() {
    let p = load("custom-incomplete");
    assert_eq!(p.mode, Mode::Light);
    assert_eq!(
        p.dark_background.to_hex(),
        "#e9ecef",
        "read under the short name"
    );
    assert_eq!(p.lighter_background.to_hex(), "#ffffff");
    let missing: Vec<_> = p.missing().collect();
    for key in [
        PaletteKey::BrightRed,
        PaletteKey::Orange,
        PaletteKey::Cyan,
        PaletteKey::Brown,
        PaletteKey::DarkerBackground,
    ] {
        assert!(missing.contains(&key), "{key:?}");
    }
    assert_eq!(p.colours().count(), 10);
}

#[test]
fn a_malformed_custom_theme_names_the_key_and_the_problem() {
    let err = Palette::load(&fixture("custom-malformed")).unwrap_err();
    assert!(
        matches!(
            &err,
            Error::BadColor {
                key: "red",
                value,
                source: ColorError::Length(5),
            } if value == "#f7768"
        ),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "colors.toml `red` is \"#f7768\", not a colour: a colour has 6 or 8 hex digits after `#`, not 5"
    );
}

/// The `colors.toml` of every theme in `root`, or none when there is no such directory.
fn themes_in(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path().join("colors.toml"))
        .filter(|path| path.is_file())
        .collect()
}

/// Every stock theme Omarchy ships parses as a palette: only keys ODK does not know hold
/// anything but a colour. Skipped where Omarchy is not installed.
#[test]
fn every_stock_theme_parses() {
    let themes = themes_in(Path::new("/usr/share/omarchy/themes"));
    for path in &themes {
        Palette::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    eprintln!("{} stock themes parsed", themes.len());
}

/// The user's own themes are reported, not asserted: one may be half-written, which ADR 0041
/// treats as a theme that does not load, and the tests must not depend on them.
#[test]
fn the_user_s_own_themes_are_reported() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let themes = themes_in(&PathBuf::from(home).join(".config/omarchy/themes"));
    let failed: Vec<_> = themes
        .iter()
        .filter_map(|path| Some((path, Palette::load(path).err()?)))
        .collect();
    for (path, error) in &failed {
        eprintln!("does not load: {}: {error}", path.display());
    }
    eprintln!(
        "{} of the user's {} themes parsed",
        themes.len() - failed.len(),
        themes.len()
    );
}
