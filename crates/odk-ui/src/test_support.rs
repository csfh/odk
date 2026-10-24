//! A window on GPUI's test platform with the content a test or a benchmark builds, as an
//! application's window would be: inside `root()`, the window's shell and the bindings every ODK
//! application gets from `run`. The checklist's tests and the benchmarks (`benches/`) open their
//! windows with it, so both measure what an application draws.
//!
//! Not part of the library's API: it exists for this workspace's tests and benchmarks, behind the
//! `test-support` feature, and may change at any time.

use gpui::{
    AnyElement, AppContext as _, Context, IntoElement, ParentElement, Render, TestAppContext,
    VisualTestContext, Window,
};

type Build = Box<dyn Fn(&mut Window, &mut Context<Host>) -> AnyElement>;

/// A window whose whole content is what the test builds, inside `root()` and the window's
/// shell, as in an application.
pub struct Host {
    build: Build,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::root().child((self.build)(window, cx))
    }
}

/// Makes `snapshot` the followed theme, as a reload the watcher asked for does: for a test that
/// stands in for the system changing something while the application runs, such as the user's
/// request for reduced motion, which is the compositor's (ADR 0048) and which a previewed theme
/// never carries of its own.
pub fn publish_followed(snapshot: std::sync::Arc<odk_theme::ThemeSnapshot>) {
    crate::theme::publish(snapshot);
}

/// Opens a window showing `build` with the test theme and the bindings every ODK application
/// gets from `run`.
pub fn open<E: IntoElement>(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut Context<Host>) -> E + 'static,
) -> &mut VisualTestContext {
    crate::theme::install_test_theme();
    cx.update(crate::app::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host {
            build: Box::new(move |w, cx| build(w, cx).into_any_element()),
        });
        crate::app::shell(window, cx, host.into())
    });
    // The compositor activates a new window; the test platform does not.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx
}

/// Draws the window again from scratch: every view is rendered, laid out and painted, as on a
/// frame after a theme change. What a benchmark of one frame measures.
pub fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Opens a window whose view is the one `build` makes, as `open_window` does, and says which view
/// it is, for a program's own view such as the gallery's.
pub fn open_view<V: Render + 'static>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut gpui::App) -> gpui::Entity<V>,
) -> (gpui::Entity<V>, &mut VisualTestContext) {
    crate::theme::install_test_theme();
    cx.update(crate::app::init);
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let child = build(window, cx);
        view = Some(child.clone());
        crate::app::shell(window, cx, child.into())
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view.expect("the window built its view"), cx)
}

/// How many frames the components asked for on this thread since this was last called. A window
/// that is idle asks for none, even when it is drawn again (checklist item 35).
pub fn take_frame_requests() -> usize {
    crate::interaction::take_frame_requests()
}
