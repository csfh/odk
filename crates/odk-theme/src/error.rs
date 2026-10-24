use std::path::PathBuf;

use crate::ColorError;
use crate::component::ComponentError;
use crate::semantic::ResolveError;

/// Why a theme could not be found, read or watched. Every error from a theme's files names the
/// file, and the key when there is one (`docs/spec/odk-source-palette.md`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read. The source's kind tells a missing file
    /// (`ErrorKind::NotFound`) from one the application may not read
    /// (`ErrorKind::PermissionDenied`).
    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid {file}: {source}")]
    Toml {
        file: &'static str,
        source: toml::de::Error,
    },
    /// A required key is missing, under its name and under its short alias.
    #[error("colors.toml has no `{0}`")]
    MissingKey(&'static str),
    /// A key that holds a colour holds text that is not one. `key` is the name as written,
    /// which may be a short alias.
    #[error("colors.toml `{key}` is {value:?}, not a colour: {source}")]
    BadColor {
        key: &'static str,
        value: String,
        source: ColorError,
    },
    #[error("colors.toml `mode` is {0:?}; it must be \"dark\" or \"light\"")]
    BadMode(String),
    /// A key ODK reads holds a TOML value that is not a string, such as `red = 5`.
    #[error("colors.toml `{key}` must be a quoted string, not a TOML {found}")]
    WrongType {
        key: &'static str,
        found: &'static str,
    },
    #[error("cannot watch {}: {source}", path.display())]
    Watch {
        path: PathBuf,
        source: notify_debouncer_mini::notify::Error,
    },
    #[error("neither $XDG_STATE_HOME nor $HOME is set")]
    NoStateDir,
    /// A theme name that is empty, starts with `.` or holds `/`, which Omarchy refuses too.
    #[error("{0:?} is not a theme name")]
    BadThemeName(String),
    #[error("there is no theme {name:?}: looked in {}", list(searched))]
    NoTheme {
        name: String,
        searched: Vec<PathBuf>,
    },
    /// The theme loaded, but its semantic colours cannot be resolved with the options in use
    /// (`docs/spec/odk-semantic-tokens.md`).
    #[error("the theme's semantic colours cannot be resolved: {0}")]
    Resolve(#[from] ResolveError),
    /// The theme's semantic colours resolved, but a component draws a pair of them that does
    /// not reach its contrast (`docs/spec/odk-component-tokens.md`).
    #[error("the theme's component colours fail their checks: {0}")]
    Component(#[from] ComponentError),
}

fn list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" and ")
}
