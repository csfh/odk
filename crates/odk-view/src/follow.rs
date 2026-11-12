//! Following the user's theme while the window is open (`docs/spec/odk-source-palette.md`,
//! "Reload"; ADR 0041, ADR 0062).
//!
//! A background thread owns the [`ThemeManager`]. It reloads when the theme's files change, when
//! `odkd` says something it reports changed (the palette, the text size, the request for reduced
//! motion), or when asked; it resolves the semantic colours again when the colour options change;
//! and it hands the window an [`Update`] only when the snapshot or what [`ThemeDiagnostics`] says
//! is different. What the window does with it is the caller's: [`crate::run`] gives it to Rune's
//! `AppHandle::update`.

use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use odk_theme::semantic::{OptionsError, ResolveOptions};
use odk_theme::{ThemeDiagnostics, ThemeManager, ThemeSnapshot, ThemeWatcher};

/// What changed: a new snapshot, when the colours or text size are not the ones before, and what
/// the manager now says about what it draws.
#[derive(Debug, Clone)]
pub struct Update {
    pub snapshot: Option<ThemeSnapshot>,
    pub diagnostics: ThemeDiagnostics,
}

enum Request {
    Reload,
    Options(ResolveOptions),
}

/// Keeps a window's theme up to date for as long as it lives. Dropping it stops the thread.
pub struct Follower {
    requests: Sender<Request>,
    _watcher: Option<ThemeWatcher>,
}

impl Follower {
    /// Asks for a reload now, for a change the files do not announce.
    pub fn reload(&self) {
        let _ = self.requests.send(Request::Reload);
    }

    /// Changes how the semantic colours are resolved (contrast minimums, strategy, the palette
    /// keys each role prefers). The files are not read again. Options that would weaken WCAG's
    /// minimums are refused here.
    pub fn set_options(&self, options: ResolveOptions) -> Result<(), OptionsError> {
        options.validate()?;
        let _ = self.requests.send(Request::Options(options));
        Ok(())
    }

    /// A way to ask from elsewhere.
    pub(crate) fn commands(&self) -> Commands {
        Commands(self.requests.clone())
    }
}

/// What can be asked of the follower from anywhere in the process.
#[derive(Clone)]
pub(crate) struct Commands(Sender<Request>);

static COMMANDS: Mutex<Option<Commands>> = Mutex::new(None);

pub(crate) fn install(commands: Commands) {
    *COMMANDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(commands);
}

fn current() -> Option<Commands> {
    COMMANDS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Asks for the theme to be read again, as a change on disk would, from anywhere. The result
/// shows on a later frame, and only if the theme changed. For an application's own "reload theme"
/// command; Omarchy's theme changes are already followed. `Err` when the application did not
/// start through [`crate::run`].
pub fn reload_theme() -> Result<(), crate::post::NoWindow> {
    let commands = current().ok_or(crate::post::NoWindow)?;
    let _ = commands.0.send(Request::Reload);
    Ok(())
}

/// Why the colour options were not set.
#[derive(Debug, Clone, PartialEq)]
pub enum ColourOptionsError {
    /// They would weaken WCAG's minimums or cannot be met at all.
    Refused(OptionsError),
    /// The application did not start through [`crate::run`].
    NoWindow,
}

impl std::fmt::Display for ColourOptionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(why) => write!(f, "{why}"),
            Self::NoWindow => crate::post::NoWindow.fmt(f),
        }
    }
}

impl std::error::Error for ColourOptionsError {}

/// Changes how the semantic colours are resolved, from anywhere, as [`Follower::set_options`]
/// does. The result shows on a later frame; if the theme's colours cannot meet the options the
/// last snapshot stays and the diagnostics say why.
pub fn set_colour_options(options: ResolveOptions) -> Result<(), ColourOptionsError> {
    options.validate().map_err(ColourOptionsError::Refused)?;
    let commands = current().ok_or(ColourOptionsError::NoWindow)?;
    let _ = commands.0.send(Request::Options(options.clone()));
    // The host remembers what was asked, so an application can read it back.
    let _ = crate::post::on_any_host(move |host| host.remember_colour_options(options));
    Ok(())
}

/// Follows `manager` from now on, calling `apply` with each [`Update`] on a background thread: a
/// snapshot that differs from the one before, or diagnostics that read differently. The
/// manager's own source is watched, and so is `odkd`'s host.
pub fn follow(manager: ThemeManager, apply: impl Fn(Update) + Send + 'static) -> Follower {
    let (requests, queue) = channel::<Request>();
    // If the watcher cannot start, the theme is followed only on request.
    let watcher = manager.source().and_then(|source| {
        let sender = requests.clone();
        ThemeWatcher::follow(source, move || {
            let _ = sender.send(Request::Reload);
        })
        .ok()
    });
    if let Some(host) = manager.host() {
        let sender = requests.clone();
        host.on_change(move || {
            let _ = sender.send(Request::Reload);
        });
    }
    std::thread::spawn(move || {
        let mut manager = manager;
        let mut said = manager.diagnostics().to_string();
        while let Ok(first) = queue.recv() {
            // Whatever queued up meanwhile is answered with one reload, with the latest options.
            let (mut reload, mut options) = (false, None);
            let mut take = |request| match request {
                Request::Reload => reload = true,
                Request::Options(o) => options = Some(o),
            };
            take(first);
            while let Ok(next) = queue.try_recv() {
                take(next);
            }
            let before = manager.snapshot();
            if let Some(options) = options {
                // Validated when asked for, so this never refuses them.
                let _ = manager.set_options(options);
            }
            if reload {
                manager.reload();
            }
            let after = manager.snapshot();
            let diagnostics = manager.diagnostics();
            let text = diagnostics.to_string();
            let changed = !Arc::ptr_eq(&before, &after);
            if changed || text != said {
                said = text;
                apply(Update {
                    snapshot: changed.then(|| (*after).clone()),
                    diagnostics,
                });
            }
        }
    });
    Follower {
        requests,
        _watcher: watcher,
    }
}
