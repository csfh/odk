//! Following the theme while a window is open. Nothing here opens a window.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use odk_theme::{ThemeManager, ThemeSnapshot, ThemeSource};
use odk_view::follow;

const WAIT: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(700);

fn colors(background: &str) -> String {
    format!(
        r##"
mode = "dark"
accent = "#d0d0d0"
muted = "#8a8a8a"
background = "{background}"
dark_background = "#141414"
lighter_background = "#2a2a2a"
foreground = "#d0d0d0"
red = "#d0d0d0"
yellow = "#d0d0d0"
green = "#d0d0d0"
blue = "#d0d0d0"
"##
    )
}

/// A theme directory of its own, removed when the test ends.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str, background: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("odk-view-follow-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a directory");
        fs::write(path.join("colors.toml"), colors(background)).expect("colors.toml");
        Self(path)
    }

    fn set(&self, background: &str) {
        fs::write(self.0.join("colors.toml"), colors(background)).expect("colors.toml");
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn manager(dir: &Dir) -> ThemeManager {
    let mut manager = ThemeManager::new(ThemeSource::Dir(dir.path().to_path_buf()));
    manager.reload();
    manager
}

#[test]
fn a_changed_palette_is_handed_over_and_an_unchanged_one_is_not() {
    let dir = Dir::new("palette", "#1c1c1c");
    let (tx, rx) = channel::<ThemeSnapshot>();
    let follower = follow(manager(&dir), move |snapshot| {
        let _ = tx.send(snapshot);
    });
    let before = manager(&dir).snapshot();

    dir.set("#202a30");
    let after = rx
        .recv_timeout(WAIT)
        .expect("a new snapshot after the files changed");
    assert_ne!(*before, after, "it is a different theme");

    // The same content again is no new theme.
    dir.set("#202a30");
    assert!(
        matches!(rx.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout)),
        "nothing to hand over"
    );
    drop(follower);
}

#[test]
fn a_request_reloads_for_a_change_the_files_do_not_announce() {
    // The theme's directory is not there when the follower starts, so there is nothing to watch:
    // only a request reloads it.
    let path = std::env::temp_dir().join(format!("odk-view-follow-request-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    let (tx, rx) = channel::<ThemeSnapshot>();
    let follower = follow(
        ThemeManager::new(ThemeSource::Dir(path.clone())),
        move |snapshot| {
            let _ = tx.send(snapshot);
        },
    );
    fs::create_dir_all(&path).expect("a directory");
    fs::write(path.join("colors.toml"), colors("#10202a")).expect("colors.toml");
    assert!(
        matches!(rx.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout)),
        "nothing announced it"
    );
    follower.reload();
    rx.recv_timeout(WAIT)
        .expect("the reload that was asked for");
    drop(follower);
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn dropping_the_follower_ends_the_thread() {
    let dir = Dir::new("drop", "#1c1c1c");
    let (tx, rx) = channel::<ThemeSnapshot>();
    let follower = follow(manager(&dir), move |snapshot| {
        let _ = tx.send(snapshot);
    });
    drop(follower);
    // The thread drops the callback, and with it the sender, once it has seen the queue close.
    assert!(
        matches!(rx.recv_timeout(WAIT), Err(RecvTimeoutError::Disconnected)),
        "the thread is gone"
    );
}
