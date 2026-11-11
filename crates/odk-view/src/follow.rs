//! Following the user's theme while the window is open (`docs/spec/odk-source-palette.md`,
//! "Reload"; ADR 0041, ADR 0062).
//!
//! A background thread owns the [`ThemeManager`]. It reloads when the theme's files change or when
//! `odkd` says something it reports changed (the palette, the text size, the request for reduced
//! motion), and hands the window a snapshot only when it is a different one. What the window does
//! with it is the caller's: [`crate::run`] gives it to Rune's `AppHandle::update`.

use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use odk_theme::{ThemeManager, ThemeSnapshot, ThemeWatcher};

/// Keeps a window's theme up to date for as long as it lives. Dropping it stops the thread.
pub struct Follower {
    requests: Sender<()>,
    _watcher: Option<ThemeWatcher>,
}

impl Follower {
    /// Asks for a reload now, for a change the files do not announce.
    pub fn reload(&self) {
        let _ = self.requests.send(());
    }
}

/// Follows `manager` from now on, calling `apply` with each snapshot that differs from the one
/// before, on a background thread. The manager's own source is watched, and so is `odkd`'s host.
pub fn follow(manager: ThemeManager, apply: impl Fn(ThemeSnapshot) + Send + 'static) -> Follower {
    let (requests, queue) = channel::<()>();
    // If the watcher cannot start, the theme is followed only on request.
    let watcher = manager.source().and_then(|source| {
        let sender = requests.clone();
        ThemeWatcher::follow(source, move || {
            let _ = sender.send(());
        })
        .ok()
    });
    if let Some(host) = manager.host() {
        let sender = requests.clone();
        host.on_change(move || {
            let _ = sender.send(());
        });
    }
    std::thread::spawn(move || {
        let mut manager = manager;
        while queue.recv().is_ok() {
            // Whatever queued up meanwhile is answered with one reload.
            while queue.try_recv().is_ok() {}
            let before = manager.snapshot();
            manager.reload();
            let after = manager.snapshot();
            if !Arc::ptr_eq(&before, &after) {
                apply((*after).clone());
            }
        }
    });
    Follower {
        requests,
        _watcher: watcher,
    }
}
