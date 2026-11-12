//! `odkd` tells every ODK application what the host says (ADR 0062): the user's request for
//! reduced motion, text size and font, and the theme's palette. It runs as the user, outside
//! every sandbox, and is private: `/usr/lib/odk/odkd`, or `~/.local/lib/odk/odkd` for one user.
//! It is started by `odkd.socket` when an application first connects.
//!
//! It never reads from a client of `odkd.sock`. It writes one line of JSON on connect and one on
//! every change, and it waits on watchers and on the compositor's event socket, never on a clock.
//! Beside it, `files.sock` opens the desktop's file chooser for an application and answers with
//! the paths the user chose (ADR 0068).

mod a11y;
mod files;
mod hub;
mod portal;
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
    let listeners = serve::listeners(socket_path())?;
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

    // A stand-in session bus for a sandboxed window's accessibility (ADR 0096), kept up to date
    // with the user's own. Without `dbus-daemon` or a session bus there is none, and a window
    // is as it was: no tree for a screen reader.
    let _standin = socket_path()
        .as_deref()
        .and_then(std::path::Path::parent)
        .and_then(|dir| match a11y::Standin::start(dir) {
            Ok(standin) => Some(standin),
            Err(why) => {
                eprintln!("odkd: no accessibility stand-in: {why}");
                None
            }
        });
    if let Some(standin) = &_standin {
        let sender = standin.sender();
        std::thread::spawn(move || {
            let report = |state| {
                let _ = sender.try_send(state);
            };
            if let Err(why) = futures_lite::future::block_on(a11y::follow_session(report)) {
                eprintln!("odkd: not following the session's accessibility: {why}");
            }
        });
    }

    let mut state = sources.read();
    hub.publish(state.to_line());
    serve::accept_in_background(listeners.host, hub.clone());
    match listeners.files {
        Some(files) => files::serve_in_background(
            files,
            Arc::new(portal::Portal),
            files::Folders::from_environment(),
        ),
        None => eprintln!("odkd: no file chooser: there is nowhere to make its socket"),
    }

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
