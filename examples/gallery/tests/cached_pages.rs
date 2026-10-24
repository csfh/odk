//! A page is kept from one frame to the next and drawn again only when it changes, so a change
//! that does not come from the page itself must still reach it. These tests change something
//! from outside a page, as the program does, and look for it on the page.

use std::time::Duration;

use gallery::{Gallery, Page};
use odk::gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use odk::prelude::Density;
use odk::theme::foundation::FoundationConfig;
use odk::ui::test_support::open_view;

/// The tests share the process's theme and foundation tokens, which are global outside `odk-ui`'s
/// own tests, so they run one at a time and each starts from the defaults.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Opens the gallery on `page` with motion reduced, so that no spinner's timer redraws it while a
/// test counts what it drew.
fn open(app: &mut TestAppContext, page: Page) -> (Entity<Gallery>, &mut VisualTestContext) {
    let (gallery, cx) = open_view(app, |_, cx| cx.new(Gallery::new));
    cx.update(|_, cx| {
        odk::ui::configure(cx, |c| {
            *c = FoundationConfig {
                reduced_motion: true,
                ..Default::default()
            }
        })
    });
    gallery.update(cx, |g, cx| g.show(page, cx));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    (gallery, cx)
}

/// How many different things take the focus in `tabs` presses of Tab.
fn stops(cx: &mut VisualTestContext, tabs: usize) -> usize {
    let mut seen = Vec::new();
    for _ in 0..tabs {
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        if let Some(focused) = cx.update(|window, cx| window.focused(cx))
            && !seen.contains(&focused)
        {
            seen.push(focused);
        }
    }
    seen.len()
}

/// What makes the tests below mean something: a page is rendered again only when it changed, and
/// not when the header around it did. Without it every page would be drawn at every frame and
/// none of them could be stale.
#[test]
fn a_page_is_kept_when_only_the_header_changes() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Buttons);
    let renders = gallery::page_renders;

    let settled = renders();
    gallery.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        renders(),
        settled,
        "the page was rendered again for a change of the header"
    );

    let view = gallery
        .read_with(cx, |g, _| g.view(Page::Buttons))
        .entity_id();
    cx.update(|_, cx| cx.notify(view));
    cx.run_until_parked();
    assert_eq!(
        renders(),
        settled + 1,
        "the page was not rendered again for a change of its own"
    );
}

/// A change of the application's density, which comes from `configure` and not from the page,
/// reaches the page drawn from the last frame: its buttons change height.
#[test]
fn a_change_of_density_reaches_a_kept_page() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, Page::Buttons);
    // The first of the variants, a button at the application's density (34 px by default).
    let height =
        |cx: &mut VisualTestContext| cx.debug_bounds("variant-0/button").unwrap().size.height;
    let before = height(cx);
    cx.update(|_, cx| odk::ui::configure(cx, |c| c.density = Density::Compact));
    cx.run_until_parked();
    let after = height(cx);
    assert!(
        after < before,
        "the button is {before:?} high before and {after:?} after"
    );
}

/// The page's Tab stops are kept with it: a frame in which only the header changed leaves Tab
/// moving through the page, to as many different things as on a page drawn whole.
#[test]
fn tab_still_walks_a_kept_page_after_the_header_redraws() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, Page::Buttons);
    let whole = stops(cx, 25);

    let (gallery, cx) = open(&mut app, Page::Buttons);
    // The gallery's own view is dirty, the page's is not: the header is drawn, the page kept.
    gallery.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let kept = stops(cx, 25);

    assert!(
        whole >= 20,
        "Tab visits only {whole} things on a page drawn whole"
    );
    assert_eq!(
        kept, whole,
        "Tab visits {kept} things on a kept page, {whole} on a whole one"
    );
}
