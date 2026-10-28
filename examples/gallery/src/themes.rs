//! The themes the gallery can show: the ones installed, which of them is previewed in place of
//! the one Omarchy has selected, and the snapshots loaded for the pages that compare them.
//!
//! A preview changes what is drawn, never Omarchy's selection or any theme file
//! (`docs/spec/odk-source-palette.md`, "Previewing a theme"). Loading reads files, so it happens
//! when a theme is chosen, never while drawing; what was loaded is kept.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use odk::gpui::{App, Context};
use odk::theme::{ThemeDiagnostics, ThemeManager, ThemeSnapshot, ThemeSource};

/// A theme loaded for the gallery, with what its manager said about it.
#[derive(Clone)]
pub struct Loaded {
    pub name: String,
    pub snapshot: Arc<ThemeSnapshot>,
    pub diagnostics: ThemeDiagnostics,
}

/// The installed themes and the previewed one.
pub struct Themes {
    /// Names of themes found in Omarchy's theme directories, and any given by `GALLERY_THEME`.
    names: Vec<String>,
    /// Themes given as directories, by the name they are listed under.
    directories: HashMap<String, PathBuf>,
    loaded: HashMap<String, Arc<Loaded>>,
    /// The theme being previewed; `None` follows the system.
    choice: Option<String>,
    /// Why the last attempt to load a theme failed.
    error: Option<String>,
    /// How many times a theme's files have been read, for the tests that check drawing reads
    /// none.
    reads: usize,
}

impl Themes {
    /// The installed themes, and `extra`, a theme named or a directory (`GALLERY_THEME`).
    pub fn new(extra: Option<&str>) -> Self {
        let mut themes = Self {
            names: ThemeSource::installed(),
            directories: HashMap::new(),
            loaded: HashMap::new(),
            choice: None,
            error: None,
            reads: 0,
        };
        if let Some(extra) = extra {
            themes.add(extra);
        }
        themes
    }

    /// Lists a theme given by name or by directory, so it can be chosen.
    pub fn add(&mut self, theme: &str) -> String {
        let path = PathBuf::from(theme);
        let name = if path.join("colors.toml").is_file() {
            let name = path
                .file_name()
                .map_or_else(|| theme.to_owned(), |n| n.to_string_lossy().into_owned());
            self.directories.insert(name.clone(), path);
            name
        } else {
            theme.to_owned()
        };
        if !self.names.contains(&name) {
            self.names.push(name.clone());
            self.names.sort();
        }
        name
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// How many times a theme's files have been read.
    pub fn reads(&self) -> usize {
        self.reads
    }

    /// The theme being previewed, or `None` when the gallery draws the system's.
    pub fn choice(&self) -> Option<&str> {
        self.choice.as_deref()
    }

    /// Why the theme last asked for could not be shown.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn source(&self, name: &str) -> Result<ThemeSource, String> {
        match self.directories.get(name) {
            Some(dir) => Ok(ThemeSource::Dir(dir.clone())),
            None => ThemeSource::named(name).map_err(|e| e.to_string()),
        }
    }

    /// Reads `name` from disk, resolved with the colour options the application follows and
    /// with the user's own text size and request for reduced motion, as the followed theme is.
    fn read(&mut self, name: &str, cx: &App) -> Result<Loaded, String> {
        self.reads += 1;
        let mut manager = ThemeManager::following_host(self.source(name)?);
        manager
            .set_options(odk::ui::colour_options(cx))
            .map_err(|e| e.to_string())?;
        match manager.reload() {
            odk::theme::Reload::Failed(error) => Err(format!("{name}: {error}")),
            _ => Ok(Loaded {
                name: name.to_owned(),
                snapshot: manager.snapshot(),
                diagnostics: manager.diagnostics(),
            }),
        }
    }

    /// `name`, if it has been loaded.
    pub fn loaded(&self, name: &str) -> Option<Arc<Loaded>> {
        self.loaded.get(name).cloned()
    }

    /// `name`, loaded the first time it is asked for and kept.
    pub fn load(&mut self, name: &str, cx: &App) -> Result<Arc<Loaded>, String> {
        if let Some(loaded) = self.loaded.get(name) {
            return Ok(loaded.clone());
        }
        let loaded = Arc::new(self.read(name, cx)?);
        self.loaded.insert(name.to_owned(), loaded.clone());
        Ok(loaded)
    }

    /// `name`, read again from disk, for when a person chooses it.
    pub fn reload(&mut self, name: &str, cx: &App) -> Result<Arc<Loaded>, String> {
        self.loaded.remove(name);
        self.load(name, cx)
    }

    /// Draws `name` in every window in place of the system's theme. On failure the theme drawn
    /// stays and [`Self::error`] says why.
    pub fn preview<T: 'static>(&mut self, name: &str, cx: &mut Context<T>) -> bool {
        match self.reload(name, cx) {
            Ok(loaded) => {
                odk::ui::preview_theme(cx, loaded.snapshot.clone());
                self.choice = Some(name.to_owned());
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    /// Draws the system's theme again.
    pub fn follow(&mut self, cx: &mut App) {
        odk::ui::end_theme_preview(cx);
        self.choice = None;
        self.error = None;
    }
}

/// A theme's name as a person would write it: `tokyo-night` is "Tokyo Night".
pub fn pretty(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |c| {
                c.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The name of the theme the system has selected, as Omarchy wrote it.
pub fn system_name(cx: &App) -> String {
    odk::ui::followed_theme(cx)
        .name
        .as_deref()
        .map_or_else(|| "unnamed".to_owned(), pretty)
}
