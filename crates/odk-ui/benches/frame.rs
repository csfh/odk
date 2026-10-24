//! What drawing costs, component by component and together (`docs/spec/component-checklist.md`,
//! items 35 and 36). Run with `cargo bench -p odk-ui`; `cargo bench -p odk-ui -- Button` runs the
//! benchmarks whose names match.
//!
//! Each benchmark draws a whole window from scratch (`redraw`: every view rendered, laid out and
//! painted, as on the frame after a theme change), which is the most a frame can cost, on GPUI's
//! test platform. That platform shapes text with a stand-in, so text costs here are a floor: the
//! numbers are for comparing a component with last week's and with its neighbours, and
//! `tools/idle-cpu` measures the real program.
//!
//! A frame has 16.7 ms at 60 Hz; the budget for a page, a window full of components, is a few
//! milliseconds, which the combined benchmark and the gallery's (`examples/gallery/benches`) show.

use gpui::{ParentElement, TestAppContext};
use odk_ui::samples::{SAMPLES, Sample};
use odk_ui::test_support::{open, redraw};
use odk_ui::v_flex;

fn main() {
    divan::main();
}

/// Every sample that stands still, as a page of them would.
fn still() -> impl Iterator<Item = &'static Sample> {
    SAMPLES.iter().filter(|s| !s.animated)
}

/// A frame of one component: a window busy with it, as many instances as its sample says.
#[divan::bench(args = SAMPLES)]
fn component(bencher: divan::Bencher, sample: &Sample) {
    let sample = *sample;
    let mut app = TestAppContext::single();
    let cx = open(&mut app, move |_, _| {
        v_flex().children((0..sample.instances).map(|n| (sample.build)(n)))
    });
    cx.update(|window, cx| {
        if let Some(setup) = sample.setup {
            setup(window, cx);
        }
    });
    cx.run_until_parked();
    bencher.bench_local(|| redraw(cx));
}

/// A frame of every component together: one page that has them all.
#[divan::bench]
fn everything(bencher: divan::Bencher) {
    let mut app = TestAppContext::single();
    let cx = open(&mut app, |_, _| {
        v_flex().children(still().flat_map(|s| (0..s.instances).map(|n| (s.build)(n))))
    });
    bencher.bench_local(|| redraw(cx));
}

/// How a frame grows with the number of components in it: the cost of a button, a field, a select
/// and a paragraph, `n` of each. Linear is what a page of controls can afford; worse is a bug.
#[divan::bench(args = [1, 10, 100, 500], types = [Button, Input, Select, Text])]
fn scaling<C: Kind>(bencher: divan::Bencher, n: usize) {
    let mut app = TestAppContext::single();
    let sample = *SAMPLES.iter().find(|s| s.name == C::NAME).unwrap();
    let cx = open(&mut app, move |_, _| {
        v_flex().children((0..n).map(|i| (sample.build)(i)))
    });
    bencher.bench_local(|| redraw(cx));
}

trait Kind {
    const NAME: &'static str;
}
struct Button;
struct Input;
struct Select;
struct Text;
impl Kind for Button {
    const NAME: &'static str = "Button";
}
impl Kind for Input {
    const NAME: &'static str = "Input";
}
impl Kind for Select {
    const NAME: &'static str = "Select";
}
impl Kind for Text {
    const NAME: &'static str = "Text";
}

/// What typing into a field costs: one key, handled and drawn, in a page of every component.
#[divan::bench]
fn typing(bencher: divan::Bencher) {
    let mut app = TestAppContext::single();
    let cx = open(&mut app, |_, _| {
        v_flex().children(still().flat_map(|s| (0..s.instances).map(|n| (s.build)(n))))
    });
    // Tab on past the buttons to the first field, which takes the keys.
    for _ in 0..70 {
        cx.simulate_keystrokes("tab");
    }
    bencher.bench_local(|| {
        cx.simulate_input("a");
        cx.run_until_parked();
    });
}

/// What moving the focus costs: one Tab, handled and drawn, in a page of every component.
#[divan::bench]
fn tabbing(bencher: divan::Bencher) {
    let mut app = TestAppContext::single();
    let cx = open(&mut app, |_, _| {
        v_flex().children(still().flat_map(|s| (0..s.instances).map(|n| (s.build)(n))))
    });
    bencher.bench_local(|| {
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
    });
}
