use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use notify_debouncer_mini::notify::{self, Event, RecursiveMode, Watcher};

use crate::discovery::current_dir;
use crate::{Error, ThemeSource};

/// Calls a function whenever Omarchy's current theme may have changed, until dropped.
///
/// Omarchy changes theme by deleting `current/theme` and moving a new directory into place
/// (`docs/checks.md`), so a watch on `theme` itself would go stale. This watches its parent,
/// `current`, and everything below it, so a file edited inside the theme in place is seen too;
/// a directory moved in is watched as it arrives. Symlinks are not followed, so a link a theme
/// ships to a large directory is never walked. The burst of events one change makes is
/// coalesced: the callback runs once the events have stopped for [`QUIET`]. Only events that
/// change something count. `notify` also reports a file being opened and read, and a reload
/// reads the theme's files, so counting those would make every reload start the next.
///
/// The callback only says that something changed; the caller reloads with
/// [`crate::ThemeManager::reload`]. One theme change can call it more than once, and a call
/// can arrive before the new directory is in place, so a reload may fail with a missing
/// file. Keep the previous theme when it does: another call follows.
pub struct ThemeWatcher {
    // Dropping the watcher drops the sender inside it, which ends the thread that waits.
    _watcher: notify::RecommendedWatcher,
}

/// How long events must stop for before the callback runs.
const QUIET: Duration = Duration::from_millis(150);

impl ThemeWatcher {
    /// Watches `$XDG_STATE_HOME/omarchy/current`. `on_change` runs on a background thread.
    pub fn start_current(on_change: impl Fn() + Send + 'static) -> Result<Self, Error> {
        Self::start(current_dir()?, on_change)
    }

    /// Watches the given `current` directory, the parent of `theme`.
    pub fn start(current: PathBuf, on_change: impl Fn() + Send + 'static) -> Result<Self, Error> {
        Self::watch(vec![current], Vec::new(), on_change)
    }

    /// Watches whatever `source` is read from ([`ThemeSource::watch_paths`]).
    pub fn follow(
        source: &ThemeSource,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self, Error> {
        Self::watch(source.watch_paths(), Vec::new(), on_change)
    }

    /// [`Self::follow`], and the user's own files as well: `shell.toml` for the text size
    /// (ADR 0043) and `fonts.conf` for the font (see [`crate::ThemeManager::user_files`]).
    /// Omarchy replaces such a file by renaming a new one over it, so each one's directory is
    /// watched, without its subdirectories; those directories hold other files that change
    /// often, and only an event on a named file itself is reported. A directory that is not
    /// there is not watched, and a setting made after the application started is then seen on
    /// the next theme change.
    pub fn follow_with(
        source: &ThemeSource,
        user_files: &[PathBuf],
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self, Error> {
        Self::watch(source.watch_paths(), user_files.to_vec(), on_change)
    }

    fn watch(
        paths: Vec<PathBuf>,
        user_files: Vec<PathBuf>,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self, Error> {
        let watch_error = |path: &PathBuf| {
            let path = path.clone();
            move |source| Error::Watch { path, source }
        };
        let first = paths.first().cloned().unwrap_or_default();
        let (changed, events) = mpsc::channel::<()>();
        let mut watcher = notify::RecommendedWatcher::new(
            {
                let (user_files, paths) = (user_files.clone(), paths.clone());
                move |event: notify::Result<Event>| {
                    let Ok(event) = event else { return };
                    // Anything under a theme path counts; of a user's directory only its file.
                    let counts = |path: &Path| {
                        paths.iter().any(|p| path.starts_with(p))
                            || user_files.iter().any(|f| path == f)
                    };
                    if !event.kind.is_access() && event.paths.iter().any(|p| counts(p)) {
                        let _ = changed.send(());
                    }
                }
            },
            notify::Config::default().with_follow_symlinks(false),
        )
        .map_err(watch_error(&first))?;
        std::thread::spawn(move || {
            while events.recv().is_ok() {
                loop {
                    match events.recv_timeout(QUIET) {
                        Ok(()) => {}
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                on_change();
            }
        });
        for path in &paths {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(watch_error(path))?;
        }
        let mut dirs: Vec<&Path> = user_files.iter().filter_map(|f| f.parent()).collect();
        dirs.sort();
        dirs.dedup();
        for dir in dirs.into_iter().filter(|d| d.is_dir()) {
            // The theme still follows when one cannot be watched.
            let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
        }
        Ok(Self { _watcher: watcher })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc;

    use super::*;

    /// Replaces `theme` the way `omarchy-theme-set` does: remove it, move a new one in.
    #[test]
    fn reports_a_theme_directory_being_replaced() {
        let current = tempfile::tempdir().unwrap();
        fs::create_dir(current.path().join("theme")).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = ThemeWatcher::start(current.path().to_owned(), move || {
            let _ = tx.send(());
        })
        .unwrap();

        fs::create_dir(current.path().join("next-theme")).unwrap();
        fs::remove_dir_all(current.path().join("theme")).unwrap();
        fs::rename(
            current.path().join("next-theme"),
            current.path().join("theme"),
        )
        .unwrap();

        rx.recv_timeout(Duration::from_secs(5))
            .expect("no change reported");
    }

    /// The user's directory also holds files that change for other reasons; only their
    /// `shell.toml`, replaced the way `omarchy display text size` replaces it, is reported.
    #[test]
    fn only_the_users_shell_toml_is_reported_from_its_directory() {
        let current = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let shell = config.path().join("shell.toml");
        fs::write(&shell, "[font]\nbase-size = 12\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let source = ThemeSource::Current {
            current: current.path().to_owned(),
        };
        let _watcher =
            ThemeWatcher::follow_with(&source, std::slice::from_ref(&shell), move || {
                let _ = tx.send(());
            })
            .unwrap();

        fs::write(config.path().join("shell.json"), "{}").unwrap();
        assert!(
            rx.recv_timeout(Duration::from_millis(800)).is_err(),
            "a neighbour's change was reported"
        );

        let next = config.path().join("next.tmp");
        fs::write(&next, "[font]\nbase-size = 16\n").unwrap();
        fs::rename(&next, &shell).unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("a new shell.toml was not reported");
    }

    /// Reading a file is not a change. `notify` reports a file being opened, and a reload opens
    /// the theme's files, so without this every reload would start another, for ever.
    #[test]
    fn reading_the_files_is_not_reported() {
        let current = tempfile::tempdir().unwrap();
        fs::create_dir(current.path().join("theme")).unwrap();
        let colors = current.path().join("theme/colors.toml");
        fs::write(&colors, "mode = \"dark\"\n").unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = ThemeWatcher::start(current.path().to_owned(), move || {
            let _ = tx.send(());
        })
        .unwrap();

        for _ in 0..5 {
            let _ = fs::read_to_string(&colors).unwrap();
        }
        assert!(
            rx.recv_timeout(Duration::from_millis(800)).is_err(),
            "reading a file was reported as a change"
        );
        fs::write(&colors, "mode = \"light\"\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("a write was not reported");
    }

    /// Both of the user's files, `shell.toml` and `fonts.conf`, in directories of their own, are
    /// followed, and a neighbour in either is not.
    #[test]
    fn the_users_text_size_and_font_files_are_both_followed() {
        let current = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let (omarchy, fontconfig) = (
            config.path().join("omarchy"),
            config.path().join("fontconfig"),
        );
        fs::create_dir(&omarchy).unwrap();
        fs::create_dir(&fontconfig).unwrap();
        let files = [omarchy.join("shell.toml"), fontconfig.join("fonts.conf")];
        for f in &files {
            fs::write(f, "old").unwrap();
        }
        let (tx, rx) = mpsc::channel();
        let source = ThemeSource::Current {
            current: current.path().to_owned(),
        };
        let _watcher = ThemeWatcher::follow_with(&source, &files, move || {
            let _ = tx.send(());
        })
        .unwrap();

        fs::write(fontconfig.join("fonts.cache"), "x").unwrap();
        fs::write(omarchy.join("shell.json"), "{}").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(800)).is_err());

        // `omarchy font set` writes in place, `omarchy display text size` renames over.
        fs::write(&files[1], "new").unwrap();
        rx.recv_timeout(Duration::from_secs(5)).expect("fonts.conf");
        fs::write(omarchy.join("next.tmp"), "new").unwrap();
        fs::rename(omarchy.join("next.tmp"), &files[0]).unwrap();
        rx.recv_timeout(Duration::from_secs(5)).expect("shell.toml");
    }

    /// After the theme has been replaced twice, a `colors.toml` edited in place inside it, and
    /// one replaced by writing a new file and renaming it over the old, are both reported.
    #[test]
    fn an_edit_inside_a_replaced_theme_is_reported() {
        let current = tempfile::tempdir().unwrap();
        fs::create_dir(current.path().join("theme")).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = ThemeWatcher::start(current.path().to_owned(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        let settle = |rx: &mpsc::Receiver<()>| {
            std::thread::sleep(Duration::from_millis(400));
            rx.try_iter().for_each(drop);
        };
        for _ in 0..2 {
            let next = current.path().join("next-theme");
            fs::create_dir(&next).unwrap();
            fs::write(next.join("colors.toml"), "old").unwrap();
            fs::remove_dir_all(current.path().join("theme")).unwrap();
            fs::rename(&next, current.path().join("theme")).unwrap();
            rx.recv_timeout(Duration::from_secs(5))
                .expect("replacement reported");
            settle(&rx);
        }
        let colors = current.path().join("theme/colors.toml");

        fs::write(&colors, "edited in place").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("an edit in place is reported");
        settle(&rx);

        let temp = current.path().join("theme/.colors.toml.tmp");
        fs::write(&temp, "written aside").unwrap();
        fs::rename(&temp, &colors).unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("a rename over the file is reported");
    }

    /// A watch on a named theme or a directory sees its `colors.toml` change.
    #[test]
    fn follow_watches_a_theme_directory() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = ThemeWatcher::follow(&ThemeSource::Dir(dir.path().to_owned()), move || {
            let _ = tx.send(());
        })
        .unwrap();
        fs::write(dir.path().join("colors.toml"), "x").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("change reported");
    }

    #[test]
    fn a_second_replacement_is_reported_too() {
        let current = tempfile::tempdir().unwrap();
        fs::create_dir(current.path().join("theme")).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = ThemeWatcher::start(current.path().to_owned(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        for round in 0..2 {
            fs::create_dir(current.path().join("next-theme")).unwrap();
            fs::remove_dir_all(current.path().join("theme")).unwrap();
            fs::rename(
                current.path().join("next-theme"),
                current.path().join("theme"),
            )
            .unwrap();
            rx.recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| panic!("round {round}: no change reported"));
            std::thread::sleep(Duration::from_millis(400));
            rx.try_iter().for_each(drop);
        }
    }
}
