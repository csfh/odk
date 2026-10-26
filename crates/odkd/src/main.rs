//! `odkd` tells every ODK application what the host says (ADR 0062): the user's request for
//! reduced motion, text size and font, and the theme's palette. It runs as the user, outside
//! every sandbox, and is private: `/usr/lib/odk/odkd`, or `~/.local/lib/odk/odkd` for one user.
//! It is started by `odkd.socket` when an application first connects.
//!
//! It never reads from a client. It writes one line of JSON on connect and one on every change,
//! and it waits on watchers and on the compositor's event socket, never on a clock.

mod hub;
mod read;
mod serve;

use std::process::ExitCode;
use std::sync::{Arc, mpsc};

use odk_theme::{ThemeWatcher, socket_path};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("odkd: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let listener = serve::listener(socket_path())?;
    let sources = read::Sources::from_environment();
    let hub = Arc::new(hub::Hub::default());

    let (changed, wake) = mpsc::channel::<()>();
    let theme_watcher = sources.theme.as_ref().and_then(|theme| {
        let files: Vec<_> = [&sources.user_shell, &sources.user_fonts]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        let tx = changed.clone();
        ThemeWatcher::follow_with(theme, &files, move || {
            let _ = tx.send(());
        })
        .map_err(|e| eprintln!("odkd: not watching the theme: {e}"))
        .ok()
    });
    let tx = changed.clone();
    let compositor_watcher = sources.compositor.as_ref().and_then(|compositor| {
        compositor.watch_reloads(move || {
            let _ = tx.send(());
        })
    });

    let mut state = sources.read();
    hub.publish(state.to_line());
    serve::accept_in_background(listener, hub.clone());

    // The watchers live as long as this loop does.
    let _watchers = (theme_watcher, compositor_watcher);
    while wake.recv().is_ok() {
        while wake.try_recv().is_ok() {}
        let next = read::keep(&state, sources.read());
        if next != state {
            hub.publish(next.to_line());
            state = next;
        }
    }
    Ok(())
}
