//! The six named user directories, looked up in `user-dirs.dirs` at every launch (ADR 0012).
//!
//! There is no fallback to a conventional location. A name with no entry, or one that is
//! missing or unsafe, is unavailable and the application still launches without it.

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::fmt::UserDir;

/// Why a named directory cannot be given to an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// `user-dirs.dirs` is missing or has no entry for it.
    NoEntry,
    /// The entry points at something that does not exist.
    Missing,
    /// After following symlinks it is the home directory, outside it, under `Applications`,
    /// or under a hidden name, any of which could reach code that runs outside the sandbox.
    Unsafe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved(HashMap<UserDir, Result<PathBuf, Unavailable>>);

impl Resolved {
    pub fn get(&self, dir: UserDir) -> &Result<PathBuf, Unavailable> {
        &self.0[&dir]
    }
}

fn key(dir: UserDir) -> &'static str {
    match dir {
        UserDir::Documents => "XDG_DOCUMENTS_DIR",
        UserDir::Downloads => "XDG_DOWNLOAD_DIR",
        UserDir::Pictures => "XDG_PICTURES_DIR",
        UserDir::Videos => "XDG_VIDEOS_DIR",
        UserDir::Music => "XDG_MUSIC_DIR",
        UserDir::Desktop => "XDG_DESKTOP_DIR",
    }
}

/// Resolves all six directories for the user whose home is `home`, from `user_dirs_file`.
pub fn resolve(home: &Path, user_dirs_file: &Path) -> Resolved {
    let entries = fs::read_to_string(user_dirs_file)
        .map(|t| parse(&t, home))
        .unwrap_or_default();
    let real_home = fs::canonicalize(home).ok();
    let all = [
        UserDir::Documents,
        UserDir::Downloads,
        UserDir::Pictures,
        UserDir::Videos,
        UserDir::Music,
        UserDir::Desktop,
    ];
    Resolved(
        all.into_iter()
            .map(|dir| {
                let result = match (entries.get(key(dir)), &real_home) {
                    (Some(path), Some(home)) => check(path, home),
                    (Some(_), None) => Err(Unavailable::Unsafe),
                    (None, _) => Err(Unavailable::NoEntry),
                };
                (dir, result)
            })
            .collect(),
    )
}

/// Reads `XDG_*_DIR="…"` lines. Only `$HOME` is expanded, as `xdg-user-dirs` writes them.
fn parse(text: &str, home: &Path) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        let path = match value.strip_prefix("$HOME") {
            Some(rest) => PathBuf::from(format!("{}{rest}", home.display())),
            None => PathBuf::from(value),
        };
        if path.is_absolute() {
            out.insert(name.trim().to_owned(), path);
        }
    }
    out
}

fn check(path: &Path, real_home: &Path) -> Result<PathBuf, Unavailable> {
    let real = fs::canonicalize(path).map_err(|_| Unavailable::Missing)?;
    let below = real
        .strip_prefix(real_home)
        .map_err(|_| Unavailable::Unsafe)?;
    match below.components().next() {
        Some(Component::Normal(first))
            if first != "Applications" && !first.to_string_lossy().starts_with('.') =>
        {
            Ok(real)
        }
        _ => Err(Unavailable::Unsafe),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    use super::*;

    fn setup(lines: &str) -> (TempDir, Resolved) {
        let home = TempDir::new().unwrap();
        for d in [
            "Documents",
            "Downloads",
            "Music",
            "Applications",
            ".config",
            "Pics",
        ] {
            fs::create_dir(home.path().join(d)).unwrap();
        }
        fs::write(home.path().join("user-dirs.dirs"), lines).unwrap();
        let resolved = resolve(home.path(), &home.path().join("user-dirs.dirs"));
        (home, resolved)
    }

    #[test]
    fn resolves_entries_inside_home() {
        let (home, r) = setup(
            "# comment\nXDG_DOCUMENTS_DIR=\"$HOME/Documents\"\nXDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\nXDG_PICTURES_DIR=\"$HOME/Pics\"\n",
        );
        let real = fs::canonicalize(home.path()).unwrap();
        assert_eq!(r.get(UserDir::Documents), &Ok(real.join("Documents")));
        assert_eq!(r.get(UserDir::Downloads), &Ok(real.join("Downloads")));
        assert_eq!(
            r.get(UserDir::Pictures),
            &Ok(real.join("Pics")),
            "names are not assumed"
        );
        assert_eq!(
            r.get(UserDir::Music),
            &Err(Unavailable::NoEntry),
            "no entry, no fallback to ~/Music"
        );
    }

    #[test]
    fn a_missing_file_makes_everything_unavailable() {
        let home = TempDir::new().unwrap();
        let r = resolve(home.path(), &home.path().join("nope"));
        assert_eq!(r.get(UserDir::Documents), &Err(Unavailable::NoEntry));
    }

    #[test]
    fn unsafe_entries_are_unavailable() {
        let (home, r) = setup(
            "XDG_DESKTOP_DIR=\"$HOME/\"\nXDG_DOCUMENTS_DIR=\"$HOME/Applications\"\nXDG_DOWNLOAD_DIR=\"$HOME/.config\"\nXDG_PICTURES_DIR=\"/\"\nXDG_VIDEOS_DIR=\"$HOME/Nowhere\"\nXDG_MUSIC_DIR=\"/usr\"\n",
        );
        let _keep = home;
        assert_eq!(
            r.get(UserDir::Desktop),
            &Err(Unavailable::Unsafe),
            "home itself"
        );
        assert_eq!(
            r.get(UserDir::Documents),
            &Err(Unavailable::Unsafe),
            "~/Applications"
        );
        assert_eq!(
            r.get(UserDir::Downloads),
            &Err(Unavailable::Unsafe),
            "a hidden name"
        );
        assert_eq!(
            r.get(UserDir::Pictures),
            &Err(Unavailable::Unsafe),
            "an ancestor of home"
        );
        assert_eq!(r.get(UserDir::Videos), &Err(Unavailable::Missing));
        assert_eq!(
            r.get(UserDir::Music),
            &Err(Unavailable::Unsafe),
            "outside home"
        );
    }

    #[test]
    fn symlinks_are_followed_before_the_check() {
        let home = TempDir::new().unwrap();
        fs::create_dir(home.path().join(".ssh")).unwrap();
        fs::create_dir(home.path().join("Applications")).unwrap();
        symlink(home.path().join(".ssh"), home.path().join("Docs")).unwrap();
        symlink(home.path().join("Applications"), home.path().join("Pics")).unwrap();
        symlink("/usr", home.path().join("Music")).unwrap();
        fs::write(
            home.path().join("user-dirs.dirs"),
            "XDG_DOCUMENTS_DIR=\"$HOME/Docs\"\nXDG_PICTURES_DIR=\"$HOME/Pics\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\n",
        )
        .unwrap();
        let r = resolve(home.path(), &home.path().join("user-dirs.dirs"));
        assert_eq!(r.get(UserDir::Documents), &Err(Unavailable::Unsafe));
        assert_eq!(r.get(UserDir::Pictures), &Err(Unavailable::Unsafe));
        assert_eq!(r.get(UserDir::Music), &Err(Unavailable::Unsafe));
    }

    #[test]
    fn a_home_that_is_itself_a_symlink_still_works() {
        let real = TempDir::new().unwrap();
        fs::create_dir(real.path().join("Documents")).unwrap();
        let link_dir = TempDir::new().unwrap();
        let home_link = link_dir.path().join("home");
        symlink(real.path(), &home_link).unwrap();
        fs::write(
            real.path().join("user-dirs.dirs"),
            "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\n",
        )
        .unwrap();
        let r = resolve(&home_link, &real.path().join("user-dirs.dirs"));
        assert_eq!(
            r.get(UserDir::Documents),
            &Ok(fs::canonicalize(real.path()).unwrap().join("Documents"))
        );
    }
}
