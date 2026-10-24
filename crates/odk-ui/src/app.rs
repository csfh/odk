use std::collections::HashSet;

use gpui::{
    AnyView, App, AppContext, Application, Context, Entity, Global, IntoElement, KeyBinding,
    ParentElement, Render, Styled, Window, WindowHandle, WindowId, WindowOptions,
};
use odk_theme::ThemeManager;
use odk_theme::foundation::FontFamily;

use crate::focus::{FocusNext, FocusPrevious};
use crate::layout::v_flex;
use crate::overlay::RaisedHost;
use crate::theme::{self, colour};
use crate::toast::ToastHost;

/// Starts the application: loads Omarchy's current theme, follows it as it changes, runs
/// `start` to open the windows, and quits when the last window closes.
///
/// A theme that cannot be loaded never stops the application (ADR 0041): it starts on the
/// emergency palette, switches to the theme once it loads, and keeps the last theme that loaded
/// when a later reload fails. [`crate::theme_diagnostics`] says which.
pub fn run(start: impl FnOnce(&mut App) + 'static) {
    let mut manager = ThemeManager::current();
    // The first load happens before any window, so the first frame has the theme.
    manager.reload();

    Application::new().run(move |cx: &mut App| {
        theme::follow(cx, manager, true);

        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        init(cx);
        start(cx);
        cx.activate(true);
    });
}

/// What every ODK application gets before its first window: the Tab bindings and their
/// handlers, and the keys of a text input. Tests call it on GPUI's test platform, so they see the same behaviour.
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrevious, None),
    ]);
    cx.bind_keys(crate::input::bindings());

    // Handled at the application, not on a view, so they work before anything has focus.
    cx.on_action(|_: &FocusNext, cx| with_active_window(cx, Window::focus_next));
    cx.on_action(|_: &FocusPrevious, cx| with_active_window(cx, Window::focus_prev));
}

/// Runs `f` on the focused window. An action is dispatched while its window is being
/// updated, so the window can only be reached once that update is over.
fn with_active_window(cx: &mut App, f: impl FnOnce(&mut Window) + 'static) {
    if let Some(window) = cx.active_window() {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, _| {
                f(window);
                // A focus change does not redraw by itself, and the focus ring must move.
                window.refresh();
            });
        });
    }
}

/// Opens a window whose content is the view `build` makes, wrapped in what every ODK window
/// has: the theme's base surface, the window's toasts, and the host for overlays opened inside
/// a modal (ADR 0039). Open every window with this, not with GPUI's `open_window`, which is the
/// raw escape hatch: such a window has none of that, and a toast or a popover inside a dialog
/// asked of it panics, saying so. Start the view with [`crate::root`].
pub fn open_window<V: Render + 'static>(
    cx: &mut App,
    options: WindowOptions,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> gpui::Result<WindowHandle<Shell>> {
    cx.open_window(options, |window, cx| {
        // Before the view is built, which may already raise a toast.
        register(window, cx);
        let child = build(window, cx);
        cx.new(|_| Shell {
            child: child.into(),
        })
    })
}

/// Records that the window has a [`Shell`].
fn register(window: &Window, cx: &mut App) {
    cx.default_global::<Shells>()
        .0
        .insert(window.window_handle().window_id());
}

/// Wraps `child` in the [`Shell`] of `window`, for tests that open their windows themselves.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn shell(window: &Window, cx: &mut App, child: AnyView) -> Shell {
    register(window, cx);
    Shell { child }
}

/// The root view of every window opened with [`open_window`]: the application's view over the
/// theme's base surface, with the overlay hosts beside it.
pub struct Shell {
    child: AnyView,
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let semantic = theme::theme_snapshot().semantic.clone();
        // The window's text is set in the UI face unless something below names another (ADR
        // 0043): an application's own text never names one, and text styles are inherited,
        // overlays included.
        let family = theme::family(&theme::foundation(), FontFamily::Ui);
        v_flex()
            .size_full()
            .bg(colour(semantic.surface.canvas))
            .text_color(colour(semantic.text.primary))
            .font_family(family)
            .child(self.child.clone())
            .child(ToastHost)
            .child(RaisedHost)
    }
}

/// The windows that have a [`Shell`].
#[derive(Default)]
struct Shells(HashSet<WindowId>);

impl Global for Shells {}

/// Panics, naming `what`, unless `window` was opened with [`open_window`]: without its shell
/// nothing would draw it, and it would never show.
pub(crate) fn require_shell(window: &Window, cx: &App, what: &str) {
    let has = cx
        .try_global::<Shells>()
        .is_some_and(|s| s.0.contains(&window.window_handle().window_id()));
    assert!(
        has,
        "{what} needs a window opened with odk::ui::open_window, which draws it"
    );
}
