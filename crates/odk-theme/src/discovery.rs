//! Where a theme's files are (`docs/spec/odk-source-palette.md`, "Discovery"). Paths come from
//! the environment only in the constructors that say so; everything else takes them as given,
//! so tests never depend on the machine's own Omarchy.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;

/// `$XDG_STATE_HOME/omarchy/current/theme`, with `$XDG_STATE_HOME` defaulting to
/// `~/.local/state`. Omarchy replaces this directory on every theme change, so watch its
/// parent (see [`crate::ThemeWatcher`]).
pub fn current_theme_dir() -> Result<PathBuf, Error> {
    Ok(current_dir()?.join("theme"))
}

/// `$XDG_STATE_HOME/omarchy/current`, the parent of the current theme. `omarchy-theme-set`
/// writes `~/.local/state`; honouring `$XDG_STATE_HOME` lets oma point an application at the
/// read-only copy its sandbox exposes (`docs/checks.md`).
pub(crate) fn current_dir() -> Result<PathBuf, Error> {
    let state = match std::env::var_os("XDG_STATE_HOME").map(PathBuf::from) {
        Some(p) if p.is_absolute() => p,
        _ => home()?.join(".local/state"),
    };
    Ok(state.join("omarchy/current"))
}

fn home() -> Result<PathBuf, Error> {
    Ok(PathBuf::from(
        std::env::var_os("HOME").ok_or(Error::NoStateDir)?,
    ))
}

/// `~/.config/omarchy/themes` and `$OMARCHY_PATH/themes`, with `$OMARCHY_PATH` defaulting to
/// `/usr/share/omarchy`.
fn theme_dirs() -> Result<(PathBuf, PathBuf), Error> {
    let omarchy = std::env::var_os("OMARCHY_PATH")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"));
    Ok((
        home()?.join(".config/omarchy/themes"),
        omarchy.join("themes"),
    ))
}

/// Which theme an application follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeSource {
    /// The current theme: `<current>/theme`, named by `<current>/theme.name`. Omarchy replaces
    /// the `theme` directory on every change, so `current` is what is watched.
    Current { current: PathBuf },
    /// A theme by name, found the way `omarchy-theme-set` finds it: each file from the user's
    /// copy in `user_themes` when it has one, else from the stock theme in `stock_themes`.
    Named {
        name: String,
        user_themes: PathBuf,
        stock_themes: PathBuf,
    },
    /// A theme directory, read as it is.
    Dir(PathBuf),
}

impl ThemeSource {
    /// The theme Omarchy has selected, from the environment (see [`current_theme_dir`]).
    pub fn current() -> Result<Self, Error> {
        Ok(Self::Current {
            current: current_dir()?,
        })
    }

    /// A stock or user-installed theme by name, from the places Omarchy keeps them:
    /// `~/.config/omarchy/themes` and `$OMARCHY_PATH/themes`, with `$OMARCHY_PATH` defaulting
    /// to `/usr/share/omarchy`. The name is normalised as Omarchy does it (see [`theme_name`]).
    pub fn named(name: &str) -> Result<Self, Error> {
        let (user_themes, stock_themes) = theme_dirs()?;
        Ok(Self::Named {
            name: theme_name(name)?,
            user_themes,
            stock_themes,
        })
    }

    /// The names of the themes [`Self::named`] can find, from the places Omarchy keeps them,
    /// sorted: see [`Self::installed_in`]. Empty when the environment names no home.
    pub fn installed() -> Vec<String> {
        theme_dirs().map_or_else(
            |_| Vec::new(),
            |(user, stock)| Self::installed_in(&user, &stock),
        )
    }

    /// The names of the themes in `user_themes` and `stock_themes`, sorted and without
    /// repeats: each directory (a symlink to one counts, a broken one does not) whose
    /// `colors.toml` is in the user's copy or the stock one, as [`Self::resolve`] would find
    /// it. A name that [`theme_name`] would refuse is left out. Nothing is read but the
    /// directories' listings.
    pub fn installed_in(user_themes: &Path, stock_themes: &Path) -> Vec<String> {
        let mut names = Vec::new();
        for dir in [user_themes, stock_themes] {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if theme_name(&name).is_ok_and(|n| n == name)
                    && [user_themes, stock_themes]
                        .iter()
                        .any(|d| d.join(&name).join("colors.toml").is_file())
                {
                    names.push(name);
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// Finds the theme's files. Only a named theme can be missing here; the files of the
    /// current theme or of a directory may still be missing when they are read.
    pub fn resolve(&self) -> Result<ThemeFiles, Error> {
        match self {
            Self::Current { current } => Ok(ThemeFiles::in_dir(
                &current.join("theme"),
                current_name(current),
            )),
            Self::Named {
                name,
                user_themes,
                stock_themes,
            } => {
                let name = theme_name(name)?;
                let user = user_themes.join(&name);
                let stock = stock_themes.join(&name);
                // `is_dir` follows symlinks, so a user theme that is a broken link counts as
                // absent and the stock theme is used, as `omarchy-theme-set` would.
                let (has_user, has_stock) = (user.is_dir(), stock.is_dir());
                if !has_user && !has_stock {
                    return Err(Error::NoTheme {
                        name,
                        searched: vec![user, stock],
                    });
                }
                let file = |file: &str| match (user.join(file), stock.join(file)) {
                    (mine, _) if has_user && mine.is_file() => mine,
                    (_, stock) if has_stock => stock,
                    (mine, _) => mine,
                };
                Ok(ThemeFiles {
                    colors: file("colors.toml"),
                    shell: file("shell.toml"),
                    name: Some(name),
                })
            }
            Self::Dir(dir) => Ok(ThemeFiles::in_dir(
                dir,
                dir.file_name().map(|n| n.to_string_lossy().into_owned()),
            )),
        }
    }

    /// The directories to watch for a change to this theme: for the current theme its parent
    /// `current`, which outlives every switch; for a named theme each of its copies that
    /// exists; for a directory the directory.
    pub fn watch_paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Current { current } => vec![current.clone()],
            Self::Named {
                name,
                user_themes,
                stock_themes,
            } => match theme_name(name) {
                Ok(name) => [user_themes.join(&name), stock_themes.join(&name)]
                    .into_iter()
                    .filter(|p| p.is_dir())
                    .collect(),
                Err(_) => Vec::new(),
            },
            Self::Dir(dir) => vec![dir.clone()],
        }
    }
}

/// Where one theme's files are, as found. The files themselves may not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeFiles {
    /// The theme's name when it is known: Omarchy's `theme.name` for the current theme, the
    /// name asked for, or a directory's own name.
    pub name: Option<String>,
    pub colors: PathBuf,
    pub shell: PathBuf,
}

impl ThemeFiles {
    pub(crate) fn in_dir(dir: &Path, name: Option<String>) -> Self {
        Self {
            name,
            colors: dir.join("colors.toml"),
            shell: dir.join("shell.toml"),
        }
    }
}

/// `name` as Omarchy names theme directories: lower case, with spaces as dashes, so
/// `"Tokyo Night"` is `tokyo-night`. A name that is empty, starts with `.` or holds `/` is
/// refused, as `omarchy-theme-set` refuses it, so no name can reach outside a themes
/// directory.
pub fn theme_name(name: &str) -> Result<String, Error> {
    let normal = name.to_ascii_lowercase().replace(' ', "-");
    if normal.is_empty() || normal.starts_with('.') || normal.contains(['/', '\0']) {
        return Err(Error::BadThemeName(name.to_owned()));
    }
    Ok(normal)
}

/// The name in `<current>/theme.name`, which `omarchy-theme-set` writes just after it swaps the
/// theme in. `None` when the file is missing or holds no usable name.
fn current_name(current: &Path) -> Option<String> {
    let text = fs::read_to_string(current.join("theme.name")).ok()?;
    let name = text.trim();
    (!name.contains('\n') && theme_name(name).is_ok()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn theme_names_are_normalised_and_cannot_escape() {
        assert_eq!(theme_name("Tokyo Night").unwrap(), "tokyo-night");
        assert_eq!(theme_name("catppuccin-latte").unwrap(), "catppuccin-latte");
        for bad in ["", ".", "..", ".hidden", "../etc", "a/b", "/abs", "a\0b"] {
            assert!(
                matches!(theme_name(bad), Err(Error::BadThemeName(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_current_theme_is_named_by_theme_name() {
        let current = tempfile::tempdir().unwrap();
        let source = ThemeSource::Current {
            current: current.path().to_owned(),
        };
        let files = source.resolve().unwrap();
        assert_eq!(files.colors, current.path().join("theme/colors.toml"));
        assert_eq!(files.shell, current.path().join("theme/shell.toml"));
        assert_eq!(files.name, None, "no theme.name yet");

        fs::write(current.path().join("theme.name"), "tokyo-night\n").unwrap();
        assert_eq!(
            source.resolve().unwrap().name.as_deref(),
            Some("tokyo-night")
        );
        fs::write(current.path().join("theme.name"), "../../etc\n").unwrap();
        assert_eq!(source.resolve().unwrap().name, None, "not a theme name");
        assert_eq!(source.watch_paths(), vec![current.path().to_owned()]);
    }

    /// Each file comes from the user's copy when it has one, else from the stock theme, the way
    /// `omarchy-theme-set` lays the user's theme over the stock one.
    #[test]
    fn a_named_theme_lays_the_user_s_files_over_the_stock_ones() {
        let root = tempfile::tempdir().unwrap();
        let (user, stock) = (root.path().join("user"), root.path().join("stock"));
        fs::create_dir_all(user.join("nord")).unwrap();
        fs::create_dir_all(stock.join("nord")).unwrap();
        fs::write(stock.join("nord/colors.toml"), "").unwrap();
        fs::write(stock.join("nord/shell.toml"), "").unwrap();
        fs::write(user.join("nord/shell.toml"), "").unwrap();
        let source = ThemeSource::Named {
            name: "Nord".into(),
            user_themes: user.clone(),
            stock_themes: stock.clone(),
        };
        let files = source.resolve().unwrap();
        assert_eq!(files.name.as_deref(), Some("nord"));
        assert_eq!(files.colors, stock.join("nord/colors.toml"));
        assert_eq!(files.shell, user.join("nord/shell.toml"));
        assert_eq!(
            source.watch_paths(),
            vec![user.join("nord"), stock.join("nord")]
        );

        fs::write(user.join("nord/colors.toml"), "").unwrap();
        assert_eq!(
            source.resolve().unwrap().colors,
            user.join("nord/colors.toml")
        );
    }

    #[test]
    fn a_user_theme_alone_is_found_and_names_its_own_missing_files() {
        let root = tempfile::tempdir().unwrap();
        let (user, stock) = (root.path().join("user"), root.path().join("stock"));
        fs::create_dir_all(user.join("mine")).unwrap();
        let files = ThemeSource::Named {
            name: "mine".into(),
            user_themes: user.clone(),
            stock_themes: stock,
        }
        .resolve()
        .unwrap();
        assert_eq!(files.colors, user.join("mine/colors.toml"));
    }

    #[test]
    fn a_missing_theme_says_where_it_looked() {
        let root = tempfile::tempdir().unwrap();
        let (user, stock) = (root.path().join("user"), root.path().join("stock"));
        let err = ThemeSource::Named {
            name: "nowhere".into(),
            user_themes: user.clone(),
            stock_themes: stock.clone(),
        }
        .resolve()
        .unwrap_err();
        match err {
            Error::NoTheme { name, searched } => {
                assert_eq!(name, "nowhere");
                assert_eq!(searched, vec![user.join("nowhere"), stock.join("nowhere")]);
            }
            other => panic!("{other:?}"),
        }
    }

    /// Many user themes are symlinks to a working copy. One whose target is gone counts as
    /// absent, so the stock theme of that name is used, or the theme is missing.
    #[test]
    fn a_broken_symlink_in_the_user_themes_is_not_a_theme() {
        let root = tempfile::tempdir().unwrap();
        let (user, stock) = (root.path().join("user"), root.path().join("stock"));
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(stock.join("nord")).unwrap();
        symlink(root.path().join("gone"), user.join("nord")).unwrap();
        symlink(root.path().join("gone"), user.join("ghost")).unwrap();
        let named = |name: &str| ThemeSource::Named {
            name: name.into(),
            user_themes: user.clone(),
            stock_themes: stock.clone(),
        };
        assert_eq!(
            named("nord").resolve().unwrap().colors,
            stock.join("nord/colors.toml")
        );
        assert_eq!(named("nord").watch_paths(), vec![stock.join("nord")]);
        assert!(matches!(
            named("ghost").resolve(),
            Err(Error::NoTheme { .. })
        ));
    }

    #[test]
    fn a_directory_is_named_after_itself() {
        let files = ThemeSource::Dir(PathBuf::from("/themes/vantablack"))
            .resolve()
            .unwrap();
        assert_eq!(files.name.as_deref(), Some("vantablack"));
        assert_eq!(files.colors, Path::new("/themes/vantablack/colors.toml"));
    }

    /// The installed themes are the directories that hold a `colors.toml` in the user's copy
    /// or the stock one, once each, sorted; a user theme that only overlays some files counts
    /// when the stock theme of its name has the colours, and a broken link or a directory
    /// without colours does not.
    #[test]
    fn installed_themes_are_the_directories_with_colours() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let (user, stock) = (root.path().join("user"), root.path().join("stock"));
        let touch = |dir: &Path, theme: &str, file: &str| {
            fs::create_dir_all(dir.join(theme)).unwrap();
            fs::write(dir.join(theme).join(file), "").unwrap();
        };
        touch(&stock, "nord", "colors.toml");
        touch(&stock, "tokyo-night", "colors.toml");
        touch(&user, "nord", "colors.toml");
        touch(&user, "aether", "colors.toml");
        touch(&user, "overlay", "shell.toml");
        touch(&stock, "overlay", "colors.toml");
        touch(&user, "no-colours", "shell.toml");
        touch(&user, ".hidden", "colors.toml");
        symlink(root.path().join("gone"), user.join("ghost")).unwrap();
        assert_eq!(
            ThemeSource::installed_in(&user, &stock),
            ["aether", "nord", "overlay", "tokyo-night"]
        );
        assert!(
            ThemeSource::installed_in(&root.path().join("a"), &root.path().join("b")).is_empty()
        );
    }
}
