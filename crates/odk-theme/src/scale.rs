//! The text size Omarchy's user has chosen, as a scale (ADR 0043,
//! `docs/spec/odk-source-palette.md`, "Text size"). Of everything in `shell.toml` only
//! `[font] base-size` is read (and, in the user's own file only, `[motion] reduced`, ADR 0046),
//! and it is the user's own setting rather than a theme's: the
//! machine-level `~/.config/omarchy/shell.toml` layers over the theme's.

use toml::Table;

/// The base size at which every ODK length is as its spec gives it: the Omarchy shell's default.
const DEFAULT_PX: f32 = 12.0;
/// The range `omarchy display text size` accepts.
const MIN_PX: f32 = 9.0;
const MAX_PX: f32 = 20.0;

/// How much larger than ODK's own the user wants text and the controls around it: Omarchy's
/// `[font] base-size` over 12 px, kept to the 9 to 20 px that `omarchy display text size`
/// accepts so no setting can break a layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextScale {
    base_size: f32,
}

impl Default for TextScale {
    fn default() -> Self {
        Self {
            base_size: DEFAULT_PX,
        }
    }
}

impl TextScale {
    /// The scale for a base size in px, kept to 9 to 20; anything that is not a number gives the
    /// default.
    pub fn from_base_size(px: f32) -> Self {
        if px.is_finite() {
            Self {
                base_size: px.clamp(MIN_PX, MAX_PX),
            }
        } else {
            Self::default()
        }
    }

    /// The scale for the text of a theme's `shell.toml` and of the user's own, if either has a
    /// `[font] base-size`: the user's wins. Text that does not parse, or has no such number,
    /// says nothing; a theme with a bad `shell.toml` is refused elsewhere, and the user's file
    /// is never the reason a theme fails to load.
    pub fn from_shell(theme: Option<&str>, user: Option<&str>) -> Self {
        [user, theme]
            .into_iter()
            .flatten()
            .find_map(base_size)
            .map_or_else(Self::default, Self::from_base_size)
    }

    /// The `[font] base-size` in px, as kept.
    pub fn base_size(self) -> f32 {
        self.base_size
    }

    /// How many times ODK's own length this scale makes it: 1 at 12 px.
    pub fn factor(self) -> f32 {
        self.base_size / DEFAULT_PX
    }

    pub fn is_default(self) -> bool {
        self == Self::default()
    }

    /// `px` scaled, to a whole logical pixel, so that a height or a line stays on the pixel grid
    /// and a border drawn against it stays sharp.
    pub fn px(self, px: f32) -> f32 {
        (px * self.factor()).round()
    }
}

/// Whether the user's own `shell.toml` asks for reduced motion: `[motion] reduced = true`
/// (ADR 0046). It counts only while the compositor says nothing (ADR 0048); this reads the file. Only the user's file counts, never a theme's, since a theme is not the user's
/// accessibility setting; a file that is missing, does not parse, or holds anything but a
/// boolean there says nothing.
pub fn user_reduces_motion(user: Option<&str>) -> bool {
    user.and_then(|text| toml::from_str::<Table>(text).ok())
        .and_then(|table| table.get("motion")?.as_table()?.get("reduced")?.as_bool())
        .unwrap_or(false)
}

/// The `[font] base-size` of `text`, when it is a number.
fn base_size(text: &str) -> Option<f32> {
    let table: Table = toml::from_str(text).ok()?;
    let value = table.get("font")?.as_table()?.get("base-size")?;
    value
        .as_float()
        .map(|f| f as f32)
        .or_else(|| value.as_integer().map(|i| i as f32))
        .filter(|px| px.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME: &str = "[font]\nbase-size = 13\n";
    const USER: &str = "[bar]\ntext = \"#ffffff\"\n\n[font]\nbase-size = 14\n";

    #[test]
    fn no_setting_is_the_default() {
        assert!(TextScale::from_shell(None, None).is_default());
        assert!(TextScale::from_shell(Some("[bar]\ntext = \"#fff\""), Some("")).is_default());
        assert_eq!(TextScale::default().factor(), 1.0);
    }

    /// Only `[motion] reduced = true` in the user's own file asks for reduced motion (ADR 0046).
    #[test]
    fn only_the_users_own_boolean_asks_for_reduced_motion() {
        assert!(user_reduces_motion(Some("[motion]\nreduced = true\n")));
        for says_nothing in [
            None,
            Some(""),
            Some("[motion]\nreduced = false\n"),
            Some("[motion]\nreduced = \"yes\"\n"),
            Some("[motion]\nreduced = 1\n"),
            Some("reduced = true\n"),
            Some("[font]\nreduced = true\n"),
            Some("[motion"),
        ] {
            assert!(!user_reduces_motion(says_nothing), "{says_nothing:?}");
        }
    }

    #[test]
    fn the_users_size_wins_over_the_themes() {
        assert_eq!(TextScale::from_shell(Some(THEME), None).base_size(), 13.0);
        assert_eq!(
            TextScale::from_shell(Some(THEME), Some(USER)).base_size(),
            14.0
        );
        // A user file with no size leaves the theme's.
        let other = "[bar]\ntext = \"#fff\"\n";
        assert_eq!(
            TextScale::from_shell(Some(THEME), Some(other)).base_size(),
            13.0
        );
    }

    #[test]
    fn the_size_is_kept_to_what_omarchy_accepts() {
        for (set, kept) in [
            ("1", 9.0),
            ("8", 9.0),
            ("9", 9.0),
            ("20", 20.0),
            ("200", 20.0),
        ] {
            let text = format!("[font]\nbase-size = {set}\n");
            assert_eq!(TextScale::from_shell(Some(&text), None).base_size(), kept);
        }
        assert_eq!(
            TextScale::from_shell(Some("[font]\nbase-size = 13.5"), None).base_size(),
            13.5
        );
    }

    #[test]
    fn text_that_is_not_a_size_is_ignored() {
        for text in [
            "not toml at all [",
            "[font]\nbase-size = \"big\"",
            "[font]\nbase-size = nan",
            "font = 3",
            "[font]\nbody = 20",
        ] {
            assert!(
                TextScale::from_shell(Some(text), None).is_default(),
                "{text:?}"
            );
        }
        // A user file that cannot be read does not hide a theme's size.
        assert_eq!(
            TextScale::from_shell(Some(THEME), Some("[font")).base_size(),
            13.0
        );
    }

    #[test]
    fn lengths_scale_to_whole_pixels() {
        let s = TextScale::from_base_size(14.0);
        assert_eq!(s.px(12.0), 14.0);
        assert_eq!(s.px(34.0), 40.0);
        assert_eq!(s.px(0.0), 0.0);
        assert_eq!(TextScale::default().px(34.0), 34.0);
    }
}
