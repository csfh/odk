//! Reaching the window's state from another thread, or from a handler that sees only the
//! application's own: the process's posters, one for each window, installed by [`crate::run`] or
//! [`crate::Windows`] from Rune's `AppHandle::update`.

use std::any::{Any, TypeId};
use std::sync::{Arc, Mutex};

use crate::host::{AnyHost, Hosted};

/// What gets a job onto the host from another thread.
pub(crate) type Post = Arc<dyn Fn(Box<dyn FnOnce(&mut dyn Any) + Send>) + Send + Sync>;

/// The posters of the process's windows, each with the type of state its window holds: a job
/// that names a state type reaches the first window of that type.
static POSTS: Mutex<Vec<(TypeId, Post)>> = Mutex::new(Vec::new());

/// The same for what does not depend on the application's state type: the theme, its preview and
/// its colour options. A job of this kind reaches every window.
pub(crate) type PostAny = Arc<dyn Fn(Box<dyn FnOnce(&mut dyn AnyHost) + Send>) + Send + Sync>;

static POSTS_ANY: Mutex<Vec<PostAny>> = Mutex::new(Vec::new());

/// Makes these the process's posters, in place of any there were: one for each window, with the
/// type of its state.
pub(crate) fn install_all(posts: Vec<(TypeId, Post)>, any: Vec<PostAny>) {
    *POSTS.lock().unwrap_or_else(|e| e.into_inner()) = posts;
    *POSTS_ANY.lock().unwrap_or_else(|e| e.into_inner()) = any;
}

/// The poster of the first window whose state is a `T`, when a window was started through
/// [`crate::run`] or [`crate::Windows`].
pub(crate) fn current<T: 'static>() -> Option<Post> {
    let id = TypeId::of::<T>();
    POSTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(t, _)| *t == id)
        .map(|(_, post)| post.clone())
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

/// Runs `job` on the host of the first window whose state is a `T`, from anywhere, once that
/// window has a turn. `Err` when there is no such window: the application did not start through
/// [`crate::run`] or [`crate::Windows`].
pub fn on_host<T: 'static>(
    job: impl FnOnce(&mut Hosted<T>) + Send + 'static,
) -> Result<(), NoWindow> {
    let post = current::<T>().ok_or(NoWindow)?;
    send(&post, job);
    Ok(())
}

/// Runs `job` on every window's host without naming the application's state, once each has a turn.
/// `Err` when the application did not start through [`crate::run`] or [`crate::Windows`].
pub(crate) fn on_any_host(
    job: impl Fn(&mut dyn AnyHost) + Send + Sync + 'static,
) -> Result<(), NoWindow> {
    let posts: Vec<PostAny> = POSTS_ANY.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if posts.is_empty() {
        return Err(NoWindow);
    }
    let job = Arc::new(job);
    for post in posts {
        let job = job.clone();
        post(Box::new(move |host| job(host)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use super::*;

    struct Main;
    struct Settings;

    /// A poster that runs the job on a host of the given kind right here, and says which.
    fn poster<U: 'static>(name: &'static str, ran: std::sync::mpsc::Sender<&'static str>) -> Post {
        Arc::new(move |job| {
            let mut host = crate::host::Hosted::new(
                std::marker::PhantomData::<U>,
                odk_theme::ThemeSnapshot::emergency(),
                crate::face::Face::bundled(),
            );
            let _ = ran.send(name);
            job(&mut host as &mut dyn Any);
        })
    }

    fn any_poster(name: &'static str, ran: std::sync::mpsc::Sender<&'static str>) -> PostAny {
        Arc::new(move |job| {
            let mut host = crate::host::Hosted::new(
                (),
                odk_theme::ThemeSnapshot::emergency(),
                crate::face::Face::bundled(),
            );
            let _ = ran.send(name);
            job(&mut host as &mut dyn AnyHost);
        })
    }

    /// The only test that touches the process's posters, since they are the process's.
    #[test]
    fn a_job_reaches_the_window_of_its_state_and_the_theme_reaches_every_window() {
        let (ran, heard) = channel();
        install_all(
            vec![
                (TypeId::of::<Main>(), poster::<Main>("main", ran.clone())),
                (
                    TypeId::of::<Settings>(),
                    poster::<Settings>("settings", ran.clone()),
                ),
            ],
            vec![
                any_poster("main", ran.clone()),
                any_poster("settings", ran.clone()),
            ],
        );
        let drain = || heard.try_iter().collect::<Vec<_>>();

        // A job that names a state type goes to the window with that state, and to no other.
        // (The posters here run it on a host of their own kind, which is what a window does.)
        on_host::<Settings>(|_| {}).unwrap();
        assert_eq!(drain(), ["settings"]);
        on_host::<Main>(|_| {}).unwrap();
        assert_eq!(drain(), ["main"]);
        // A type no window has is no window.
        assert_eq!(on_host::<u8>(|_| {}), Err(NoWindow));

        // A job that does not name one goes to every window, once each.
        on_any_host(|_| {}).unwrap();
        assert_eq!(drain(), ["main", "settings"]);

        // With no windows there is nobody to answer to.
        install_all(Vec::new(), Vec::new());
        assert_eq!(on_any_host(|_| {}), Err(NoWindow));
        assert_eq!(on_host::<Main>(|_| {}), Err(NoWindow));
    }
}
