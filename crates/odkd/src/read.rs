//! What `odkd` reads from the host: the same things, by the same rules, that `odk-theme` read in
//! each application before ADR 0062.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use odk_theme::{HostState, Hyprland, TextScale, ThemeSource};

/// Where each value comes from.
pub struct Sources {
    /// The theme Omarchy has selected, when the environment says where its state is.
    pub theme: Option<ThemeSource>,
    /// The user's own `~/.config/omarchy/shell.toml`: `[font] base-size` only (ADR 0043).
    pub user_shell: Option<PathBuf>,
    /// The user's `~/.config/fontconfig/fonts.conf`, where `omarchy font set` names the font.
    pub user_fonts: Option<PathBuf>,
    /// The compositor whose `animations:enabled` is the user's request for reduced motion
    /// (ADR 0048).
    pub compositor: Option<Hyprland>,
}

impl Sources {
    pub fn from_environment() -> Self {
        let config = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
            .map(|home| home.join(".config"));
        Self {
            theme: ThemeSource::current().ok(),
            user_shell: config.as_ref().map(|dir| dir.join("omarchy/shell.toml")),
            user_fonts: config.map(|dir| dir.join("fontconfig/fonts.conf")),
            compositor: Hyprland::from_environment(),
        }
    }

    /// The state now. A value that cannot be read is absent.
    pub fn read(&self) -> HostState {
        let files = self.theme.as_ref().and_then(|theme| theme.resolve().ok());
        let text = |path: Option<&PathBuf>| path.and_then(|p| fs::read_to_string(p).ok());
        HostState {
            reduced_motion: self
                .compositor
                .as_ref()
                .and_then(Hyprland::animations_enabled)
                .map(|animated| !animated),
            base_size: Some(
                TextScale::from_shell(
                    text(files.as_ref().map(|f| &f.shell)).as_deref(),
                    text(self.user_shell.as_ref()).as_deref(),
                )
                .base_size(),
            ),
            font_family: monospace_family(),
            colors: files.and_then(|f| colors(&fs::read_to_string(f.colors).ok()?)),
        }
    }
}

/// The entries of a `colors.toml` that are text, as the file has them. A file that is not TOML
/// says nothing; whether the entries make a palette is for each application to say.
fn colors(text: &str) -> Option<BTreeMap<String, String>> {
    let table: toml::Table = text.parse().ok()?;
    let entries: BTreeMap<_, _> = table
        .into_iter()
        .filter_map(|(name, value)| Some((name, value.as_str()?.to_owned())))
        .collect();
    (!entries.is_empty()).then_some(entries)
}

/// The family fontconfig gives for `monospace`, which is what an application resolves too.
fn monospace_family() -> Option<String> {
    let out = Command::new("fc-match")
        .args(["-f", "%{family[0]}", "monospace"])
        .output()
        .ok()
        .filter(|out| out.status.success())?;
    let family = String::from_utf8(out.stdout).ok()?;
    let family = family.trim();
    (!family.is_empty()).then(|| family.to_owned())
}

/// What `new` says, and where it cannot say, what was said before (a theme switch replaces the
/// theme's directory, and a reading that lands in the gap must not turn the palette off).
pub fn keep(previous: &HostState, mut new: HostState) -> HostState {
    new.reduced_motion = new.reduced_motion.or(previous.reduced_motion);
    new.font_family = new.font_family.or_else(|| previous.font_family.clone());
    new.colors = new.colors.or_else(|| previous.colors.clone());
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entries_are_the_texts_of_the_file_and_nothing_else() {
        let entries = colors("mode = \"dark\"\nbackground = \"#0D0C0B\"\nsize = 3\n").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries["mode"], "dark");
        assert!(colors("[").is_none());
        assert!(colors("").is_none());
    }

    #[test]
    fn a_gap_in_the_reading_keeps_what_was_read_before() {
        let before = HostState {
            reduced_motion: Some(true),
            font_family: Some("Mono".into()),
            colors: colors("mode = \"dark\""),
            base_size: Some(14.0),
        };
        let gap = HostState {
            base_size: Some(12.0),
            ..HostState::default()
        };
        let kept = keep(&before, gap);
        assert_eq!(kept.reduced_motion, Some(true));
        assert_eq!(kept.font_family.as_deref(), Some("Mono"));
        assert!(kept.colors.is_some());
        assert_eq!(kept.base_size, Some(12.0), "the size is read every time");
        let moved = keep(
            &before,
            HostState {
                reduced_motion: Some(false),
                ..HostState::default()
            },
        );
        assert_eq!(moved.reduced_motion, Some(false));
    }
}
