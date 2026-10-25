//! Where `oma` keeps things (ADR 0009). Every path derives from two roots that tests replace,
//! so nothing here ever needs the real home directory.

use std::path::{Path, PathBuf};

use crate::fmt::Identifier;

#[derive(Debug, Clone)]
pub struct Layout {
    home: PathBuf,
    /// `~/.local/share/oma`
    state: PathBuf,
}

impl Layout {
    /// A layout rooted at `home`, keeping state in `<data_home>/oma`.
    pub fn new(home: impl Into<PathBuf>, data_home: impl AsRef<Path>) -> Self {
        Self {
            home: home.into(),
            state: data_home.as_ref().join("oma"),
        }
    }

    /// From `$HOME` and `$XDG_DATA_HOME` (default `~/.local/share`). `None` without a usable `$HOME`.
    pub fn from_env() -> Option<Self> {
        let home = PathBuf::from(std::env::var_os("HOME").filter(|h| !h.is_empty())?);
        if !home.is_absolute() {
            return None;
        }
        let data_home = match std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            Some(p) if p.is_absolute() => p,
            _ => home.join(".local/share"),
        };
        Some(Self::new(home, data_home))
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// `~/Applications`, where installed Bundles live.
    pub fn applications(&self) -> PathBuf {
        self.home.join("Applications")
    }

    /// The application's private data directory.
    pub fn data_dir(&self, id: &Identifier) -> PathBuf {
        self.state.join("data").join(id.as_str())
    }

    /// The application's consent record (ADR 0023).
    pub fn grants_file(&self, id: &Identifier) -> PathBuf {
        self.state.join("grants").join(format!("{id}.toml"))
    }

    /// Where `oma dev` assembles a Bundle before it installs it (ADR 0051).
    pub fn dev_dir(&self) -> PathBuf {
        self.state.join("dev")
    }

    /// The application's own inbox (ADR 0015).
    pub fn inbox_dir(&self, id: &Identifier) -> PathBuf {
        self.state.join("inbox").join(id.as_str())
    }

    /// `$XDG_STATE_HOME/omarchy/current`, which holds Omarchy's current theme.
    pub fn theme_dir(&self) -> PathBuf {
        let state = match std::env::var_os("XDG_STATE_HOME").map(PathBuf::from) {
            Some(p) if p.is_absolute() => p,
            _ => self.home.join(".local/state"),
        };
        state.join("omarchy/current")
    }

    /// The user's text size, which `omarchy display text size` writes (ADR 0043). Omarchy keeps
    /// it under `$HOME/.config`, not `$XDG_CONFIG_HOME`.
    pub fn user_omarchy_shell(&self) -> PathBuf {
        self.home.join(".config/omarchy/shell.toml")
    }

    /// The user's fontconfig file, where `omarchy font set` names the monospace font.
    pub fn user_fonts_conf(&self) -> PathBuf {
        self.home.join(".config/fontconfig/fonts.conf")
    }

    /// The copy of those two settings that sandboxes are given (ADR 0044, [`crate::omarchy`]):
    /// `omarchy/` and `fontconfig/` beneath it.
    pub fn omarchy_mirror(&self) -> PathBuf {
        self.state.join("omarchy")
    }

    /// Where `user-dirs.dirs` is read from: `$XDG_CONFIG_HOME` or `~/.config`.
    pub fn user_dirs_file(&self) -> PathBuf {
        let config = match std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
            Some(p) if p.is_absolute() => p,
            _ => self.home.join(".config"),
        };
        config.join("user-dirs.dirs")
    }
}
