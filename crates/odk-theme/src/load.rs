use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::discovery::ThemeFiles;
use crate::{Error, Palette, TextScale, user_reduces_motion};

/// What one reading of a theme's files gives: the palette, the user's text size and whether the
/// user's own `shell.toml` asks for reduced motion.
pub(crate) struct Loaded {
    pub palette: Palette,
    pub text: TextScale,
    pub reduced_motion: bool,
}

/// Reads a theme's two files, and the user's own `shell.toml` when there is one, and builds
/// its palette and its text scale from the same text, so they never disagree. `colors.toml`
/// must exist; a missing `shell.toml` says nothing. A theme's `shell.toml` that is not TOML is
/// an error, as it always was (ADR 0047); the user's file is never one: unreadable, it says
/// nothing.
pub(crate) fn load(files: &ThemeFiles, user_shell: Option<&Path>) -> Result<Loaded, Error> {
    let colors = read(&files.colors)?.ok_or_else(|| Error::Io {
        path: files.colors.clone(),
        source: ErrorKind::NotFound.into(),
    })?;
    let palette = Palette::parse(&colors)?;
    let shell = read(&files.shell)?;
    check_shell(shell.as_deref())?;
    let user = user_shell.and_then(|path| read(path).ok().flatten());
    Ok(Loaded {
        palette,
        text: TextScale::from_shell(shell.as_deref(), user.as_deref()),
        reduced_motion: user_reduces_motion(user.as_deref()),
    })
}

/// A theme's `shell.toml`, when there is one, must be TOML. Of it ODK reads only `[font]
/// base-size`, which [`TextScale`] takes however the rest is written.
pub(crate) fn check_shell(shell: Option<&str>) -> Result<(), Error> {
    match shell {
        Some(text) => toml::from_str::<toml::Table>(text)
            .map(drop)
            .map_err(|source| Error::Toml {
                file: "shell.toml",
                source,
            }),
        None => Ok(()),
    }
}

/// The text of `path`, or `None` when there is no such file.
fn read(path: &Path) -> Result<Option<String>, Error> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLORS: &str = r##"
mode = "dark"
accent = "#D6A63C"
selection = "#D6A63C"
selection_background = "#D6A63C"
selection_foreground = "#0D0C0B"
muted = "#8A8278"
background = "#0D0C0B"
dark_background = "#0A0908"
lighter_background = "#24211E"
foreground = "#E8E0D4"
red = "#C45C3A"
yellow = "#D6A63C"
green = "#6B9A4A"
blue = "#4A86C4"
"##;

    #[test]
    fn the_users_shell_toml_asks_for_reduced_motion_and_a_themes_cannot() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("colors.toml"), COLORS).unwrap();
        fs::write(dir.path().join("shell.toml"), "[motion]\nreduced = true\n").unwrap();
        let files = ThemeFiles::in_dir(dir.path(), None);
        let user = dir.path().join("user.toml");
        let reduced = |user: Option<&Path>| load(&files, user).unwrap().reduced_motion;
        assert!(!reduced(None), "a theme is not the user's setting");
        assert!(!reduced(Some(&user)), "no such file");
        fs::write(&user, "[motion]\nreduced = true\n").unwrap();
        assert!(reduced(Some(&user)));
        fs::write(&user, "[motion").unwrap();
        assert!(!reduced(Some(&user)), "a user file that does not parse");
    }

    #[test]
    fn the_users_own_shell_toml_wins_over_the_themes_and_is_never_an_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("colors.toml"), COLORS).unwrap();
        fs::write(dir.path().join("shell.toml"), "[font]\nbase-size = 13\n").unwrap();
        let files = ThemeFiles::in_dir(dir.path(), None);
        let user = dir.path().join("user.toml");
        let size = |user: Option<&Path>| load(&files, user).unwrap().text.base_size();
        assert_eq!(size(None), 13.0);
        assert_eq!(size(Some(&user)), 13.0, "no such file");
        fs::write(&user, "[font]\nbase-size = 16\n").unwrap();
        assert_eq!(size(Some(&user)), 16.0);
        fs::write(&user, "[font").unwrap();
        assert_eq!(size(Some(&user)), 13.0, "a user file that does not parse");
    }

    #[test]
    fn only_the_base_size_of_a_themes_shell_toml_is_read() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("colors.toml"), COLORS).unwrap();
        // Keys ADR 0035 ignored, and the control colours ADR 0047 stopped reading.
        fs::write(
            dir.path().join("shell.toml"),
            "[spacing]\nscale = 2.0\n[font]\nbody = 30\n[controls]\nnormal-color = \"#112233\"\n",
        )
        .unwrap();
        let loaded = load(&ThemeFiles::in_dir(dir.path(), None), None).unwrap();
        assert!(loaded.text.is_default());
        assert!(!loaded.reduced_motion);
    }

    #[test]
    fn a_missing_shell_toml_says_nothing_and_a_malformed_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let files = ThemeFiles::in_dir(dir.path(), None);
        assert!(matches!(load(&files, None), Err(Error::Io { .. })));
        fs::write(dir.path().join("colors.toml"), COLORS).unwrap();
        let loaded = load(&files, None).unwrap();
        assert_eq!(
            loaded.palette.accent,
            Palette::parse(COLORS).unwrap().accent
        );
        assert!(loaded.text.is_default());
        fs::write(dir.path().join("shell.toml"), "[").unwrap();
        assert!(matches!(
            load(&files, None),
            Err(Error::Toml {
                file: "shell.toml",
                ..
            })
        ));
    }

    /// A required key that is missing is an error naming it, and a colour or a mode that does
    /// not parse is one too.
    #[test]
    fn rejects_a_palette_with_missing_or_bad_values() {
        for key in [
            "background",
            "foreground",
            "accent",
            "muted",
            "red",
            "yellow",
            "green",
            "blue",
            "mode",
            "lighter_background",
            "dark_background",
        ] {
            let without: String = COLORS
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} ")))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                matches!(Palette::parse(&without), Err(Error::MissingKey(k)) if k == key),
                "{key}"
            );
        }
        let bad = COLORS.replace("#D6A63C\"\nselection =", "red\"\nselection =");
        assert!(matches!(
            Palette::parse(&bad),
            Err(Error::BadColor { key: "accent", .. })
        ));
        let bad_mode = COLORS.replace("dark", "sepia");
        assert!(matches!(Palette::parse(&bad_mode), Err(Error::BadMode(_))));
        assert!(matches!(
            Palette::parse("not toml ["),
            Err(Error::Toml {
                file: "colors.toml",
                ..
            })
        ));
    }
}
