//! What the gallery's integration tests share: a window with the gallery in it, the theme
//! fixtures, and keys and clicks as a person makes them.

#![allow(dead_code)]

use std::time::Duration;

use gallery::{Gallery, Page};
use odk::gpui::{
    AppContext as _, Entity, KeyUpEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext,
};
use odk::theme::foundation::FoundationConfig;
use odk::ui::test_support::open_view;

/// The tests share the process's theme and foundation tokens, which are global outside `odk-ui`'s
/// own tests, so they run one at a time and each starts from the defaults.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The theme fixtures `odk-theme`'s own tests use: `tokyo-night`, `nord`, `catppuccin`,
/// `catppuccin-latte` (a light theme) and `vantablack` (a monochrome one). A theme directory, as
/// `GALLERY_THEME` takes it.
pub fn fixture(name: &str) -> String {
    format!(
        "{}/../../crates/odk-theme/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

pub const FIXTURES: [&str; 5] = [
    "tokyo-night",
    "nord",
    "catppuccin",
    "catppuccin-latte",
    "vantablack",
];

/// Opens the gallery on `page` with the foundation at its defaults and motion reduced, so that no
/// spinner's timer redraws it while a test counts what it drew.
pub fn open(app: &mut TestAppContext, page: Page) -> (Entity<Gallery>, &mut VisualTestContext) {
    let (gallery, cx) = open_view(app, |_, cx| cx.new(Gallery::new));
    cx.update(|_, cx| {
        odk::ui::end_theme_preview(cx);
        odk::ui::configure(cx, |c| {
            *c = FoundationConfig {
                reduced_motion: true,
                ..Default::default()
            }
        })
    });
    gallery.update(cx, |g, cx| g.show(page, cx));
    settle(cx);
    (gallery, cx)
}

pub fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
}

/// A key pressed and released, as a keyboard does.
pub fn press(cx: &mut VisualTestContext, keys: &str) {
    for key in keys.split(' ') {
        let keystroke = Keystroke::parse(key).unwrap();
        cx.update(|window, cx| {
            window.dispatch_keystroke(keystroke.clone(), cx);
        });
        cx.simulate_event(KeyUpEvent { keystroke });
        cx.run_until_parked();
    }
}

/// Clicks the middle of the element `id` names.
pub fn click(cx: &mut VisualTestContext, id: &'static str) {
    let bounds = cx
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("{id} is not on the screen"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

/// What has the keyboard focus: the application's id for the component, then its part.
pub fn focused(cx: &mut VisualTestContext) -> String {
    cx.update(|window, cx| {
        odk::ui::focused_component(window, cx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    })
}

/// Tabs until the focus is on a component whose id starts with `prefix`.
pub fn tab_to(cx: &mut VisualTestContext, prefix: &str) {
    for _ in 0..40 {
        press(cx, "tab");
        if focused(cx).starts_with(prefix) {
            return;
        }
    }
    panic!(
        "Tab never reached {prefix}; the focus is on {}",
        focused(cx)
    );
}
