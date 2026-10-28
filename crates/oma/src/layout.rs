//! Where `oma` keeps things (ADR 0009). Every path derives from two roots that tests replace,
//! so nothing here ever needs the real home directory.

use std::path::{Path, PathBuf};

use crate::fmt::Identifier;
use crate::trust::KeyId;

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

    /// Where a signed file's application keeps its data, previous file and inbox: one directory
    /// per (root key id, app id) pair, so the same Identifier under two roots shares nothing
    /// (ADR 0060, section 2).
    pub fn pair_dir(&self, root: &KeyId, app: &Identifier) -> PathBuf {
        self.state
            .join("apps")
            .join(root.to_string())
            .join(app.as_str())
    }

    pub fn pair_data_dir(&self, root: &KeyId, app: &Identifier) -> PathBuf {
        self.pair_dir(root, app).join("data")
    }

    pub fn pair_previous_file(&self, root: &KeyId, app: &Identifier) -> PathBuf {
        self.pair_dir(root, app).join("previous.oma")
    }

    pub fn pair_inbox_dir(&self, root: &KeyId, app: &Identifier) -> PathBuf {
        self.pair_dir(root, app).join("inbox")
    }

    /// The pair's record: the pin, consent, the high-water mark and a denial (ADR 0060,
    /// section 2). A directory Bundle's record is [`Self::grants_file`], beside these, and an
    /// Identifier is never 16 hex digits, so the two never share a name.
    pub fn pair_grants_file(&self, root: &KeyId, app: &Identifier) -> PathBuf {
        self.pairs_dir()
            .join(root.to_string())
            .join(format!("{app}.toml"))
    }

    /// Where every pair's record is, one directory per root.
    pub fn pairs_dir(&self) -> PathBuf {
        self.state.join("grants")
    }

    /// Where `--register` holds a valid file signed by a root that is not the pin (ADR 0060,
    /// section 7).
    pub fn held_file(&self, root: &KeyId, app: &Identifier) -> PathBuf {
        self.state
            .join("held")
            .join(root.to_string())
            .join(format!("{app}.oma"))
    }

    /// Where `odk oma dev` assembles a Bundle before it installs it (ADR 0051).
    pub fn dev_dir(&self) -> PathBuf {
        self.state.join("dev")
    }

    /// The application's own inbox (ADR 0015).
    pub fn inbox_dir(&self, id: &Identifier) -> PathBuf {
        self.state.join("inbox").join(id.as_str())
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
