//! What the gallery costs to draw, page by page. Run with `cargo bench -p gallery`.
//!
//! `page` draws a page from scratch, as on the frame after a theme change: the most a frame can
//! cost. `tick` draws what a real frame is while a spinner turns or a field is typed in: only the
//! active page is dirty, and the rest of the window is what GPUI kept. `switch` is what choosing
//! another tab costs. On GPUI's test platform, which shapes text with a stand-in, so the numbers
//! are a floor for comparing with earlier ones; `tools/idle-cpu` measures the running program.
//!
//! A frame has 16.7 ms at 60 Hz.

use gallery::{Gallery, Page};
use odk::gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use odk::ui::test_support::{open_view, redraw};

fn main() {
    divan::main();
}

/// Opens the gallery on `page`, with the demo content or the worst case.
fn open(
    app: &mut TestAppContext,
    page: Page,
    worst_case: bool,
) -> (Entity<Gallery>, &mut VisualTestContext) {
    let (gallery, cx) = open_view(app, |_, cx| cx.new(Gallery::new));
    gallery.update(cx, |g, cx| {
        g.set_worst_case(worst_case, cx);
        g.show(page, cx);
    });
    cx.run_until_parked();
    (gallery, cx)
}

/// A page drawn from scratch.
#[divan::bench(args = Page::ALL)]
fn page(bencher: divan::Bencher, page: &Page) {
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, *page, false);
    bencher.bench_local(|| redraw(cx));
}

/// A page drawn from scratch with the worst-case content: long labels, many items, other scripts.
#[divan::bench(args = Page::ALL)]
fn page_worst_case(bencher: divan::Bencher, page: &Page) {
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, *page, true);
    bencher.bench_local(|| redraw(cx));
}

/// The frame after one thing on the page changed: its view is dirty and nothing else is.
#[divan::bench(args = Page::ALL)]
fn tick(bencher: divan::Bencher, page: &Page) {
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, *page, false);
    let view = gallery.read_with(cx, |g, _| g.view(*page)).entity_id();
    bencher.bench_local(|| {
        cx.update(|_, cx| cx.notify(view));
        cx.run_until_parked();
    });
}

/// Choosing another tab and the page it shows, and back.
#[divan::bench]
fn switch(bencher: divan::Bencher) {
    let mut app = TestAppContext::single();
    let (gallery, cx) = open(&mut app, Page::Buttons, false);
    bencher.bench_local(|| {
        for page in [Page::Fields, Page::Buttons] {
            gallery.update(cx, |g, cx| g.show(page, cx));
            cx.run_until_parked();
        }
    });
}
