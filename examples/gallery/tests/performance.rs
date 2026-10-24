//! What the gallery costs, as tests that fail when it costs too much: how often a page redraws
//! while nothing happens, and how long a frame of it takes. The benchmarks (`cargo bench -p
//! gallery`) give the numbers; these hold them.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gallery::{Gallery, Page};
use odk::gpui::{AppContext as _, TestAppContext, VisualTestContext};
use odk::theme::foundation::FoundationConfig;
use odk::ui::test_support::{open_view, redraw, take_frame_requests};

/// The tests share the process's theme and foundation tokens, which are global outside `odk-ui`'s
/// own tests, so they run one at a time and each starts from the defaults.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Opens the gallery on `page`, settled.
fn open(
    app: &mut TestAppContext,
    page: Page,
    worst_case: bool,
) -> (odk::gpui::Entity<Gallery>, &mut VisualTestContext) {
    let (gallery, cx) = open_view(app, |_, cx| cx.new(Gallery::new));
    cx.update(|_, cx| odk::ui::configure(cx, |c| *c = FoundationConfig::default()));
    gallery.update(cx, |g, cx| {
        g.set_worst_case(worst_case, cx);
        g.show(page, cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    (gallery, cx)
}

/// How often what the page shows is marked as changed while the clock runs for `span`.
fn changes_over(
    gallery: &odk::gpui::Entity<Gallery>,
    page: Page,
    cx: &mut VisualTestContext,
    span: Duration,
) -> usize {
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    let _watch = gallery.update(cx, |g, cx| {
        g.observe_page(page, cx, move || seen.set(seen.get() + 1))
    });
    let step = Duration::from_millis(16);
    let mut t = Duration::ZERO;
    while t < span {
        cx.executor().advance_clock(step);
        cx.run_until_parked();
        t += step;
    }
    count.get()
}

/// A page with nothing turning on it is not redrawn at all, and one with a spinner on it is
/// redrawn once per glyph of the spinner, ten times a second, not at every frame of the display.
#[test]
fn an_idle_page_is_not_redrawn() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let app = &mut app;
    // Pages with a spinner in view when they open: the loading buttons, the loading search field.
    let spinning = [Page::Buttons, Page::Fields];
    for page in Page::ALL {
        let (gallery, cx) = open(app, page, false);
        let changes = changes_over(&gallery, page, cx, Duration::from_secs(5));
        if spinning.contains(&page) {
            assert!(
                (1..=60).contains(&changes),
                "{page:?} redrew {changes} times in 5 s; a spinner turning is 8 glyphs in 800 ms, \
                 so about 50"
            );
        } else {
            assert_eq!(changes, 0, "{page:?} redrew while idle");
        }
    }
}

/// What a page may take to draw from scratch: a quarter of the 16.7 ms of a 60 Hz frame, in a
/// debug build as in a release one (the dependencies are optimised in both). The slowest page
/// takes under half of it.
const FRAME_BUDGET: Duration = Duration::from_millis(4);

/// `GALLERY_FRAME_BUDGET_MS` replaces the budget on a machine slower than the development one (a
/// shared CI runner); a budget that is too tight there says nothing about the code.
fn frame_budget() -> Duration {
    std::env::var("GALLERY_FRAME_BUDGET_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(FRAME_BUDGET)
}

/// A frame of a page, drawn from scratch, takes at most a quarter of a 60 Hz frame, with the demo
/// content and with the worst case. The least of several draws is held to it, which load on the
/// machine cannot make better than it is.
#[test]
fn every_page_draws_within_a_frame() {
    let mut app = TestAppContext::single();
    let app = &mut app;
    let _serial = serial();
    let budget = frame_budget();
    for worst_case in [false, true] {
        for page in Page::ALL {
            let (_, cx) = open(app, page, worst_case);
            let least = (0..15)
                .map(|_| {
                    let start = Instant::now();
                    redraw(cx);
                    start.elapsed()
                })
                .min()
                .unwrap();
            assert!(
                least < budget,
                "{page:?} (worst case: {worst_case}) took {least:?} to draw, over {budget:?}"
            );
        }
    }
}

/// Drawing a settled page asks for no more frames: nothing on it is still moving. The count of
/// redraws above cannot see a component that asks for a frame at every draw, because GPUI's test
/// platform never runs the next frame's callbacks.
#[test]
fn drawing_an_idle_page_asks_for_no_frames() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let app = &mut app;
    for page in Page::ALL {
        let (_, cx) = open(app, page, false);
        redraw(cx);
        take_frame_requests();
        redraw(cx);
        assert_eq!(
            take_frame_requests(),
            0,
            "{page:?} asked for a frame while idle"
        );
    }
}
