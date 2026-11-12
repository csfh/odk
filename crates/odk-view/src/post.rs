//! Reaching the window's state from another thread, or from a handler that sees only the
//! application's own: the process's one poster, installed by [`crate::run`] from Rune's
//! `AppHandle::update`.

use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::host::Hosted;

/// What gets a job onto the host from another thread.
pub(crate) type Post = Arc<dyn Fn(Box<dyn FnOnce(&mut dyn Any) + Send>) + Send + Sync>;

static POST: Mutex<Option<Post>> = Mutex::new(None);

/// Makes `post` the way a job reaches the window, for the process.
pub(crate) fn install(post: Post) {
    *POST.lock().unwrap_or_else(|e| e.into_inner()) = Some(post);
}

/// The poster, when a window was started through [`crate::run`].
pub(crate) fn current() -> Option<Post> {
    POST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Runs `job` on the host of state type `T` through `post`. A job for another type of state does
/// nothing.
pub(crate) fn send<T: 'static>(post: &Post, job: impl FnOnce(&mut Hosted<T>) + Send + 'static) {
    post(Box::new(move |host: &mut dyn Any| {
        if let Some(host) = host.downcast_mut::<Hosted<T>>() {
            job(host);
        }
    }));
}

/// Why a job could not be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoWindow;

impl std::fmt::Display for NoWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("there is no window to answer to: the application did not start through run")
    }
}

impl std::error::Error for NoWindow {}

/// Runs `job` on the window's host from anywhere, once the window has a turn. `Err` when the
/// application did not start through [`crate::run`].
pub fn on_host<T: 'static>(
    job: impl FnOnce(&mut Hosted<T>) + Send + 'static,
) -> Result<(), NoWindow> {
    let post = current().ok_or(NoWindow)?;
    send(&post, job);
    Ok(())
}
