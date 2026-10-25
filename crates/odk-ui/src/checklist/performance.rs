//! Items 35 and 36: what a component costs. A window is drawn whole whenever any view in it is
//! dirty, so a component that asks for redraws it does not need makes every other component on
//! the page pay for it, and a page of idle components must cost nothing at all.
//!
//! These tests count redraws, which are exact and do not depend on the machine; how long a frame
//! takes is measured by the benchmarks in `benches/`, which draw the same `samples`.

use std::time::Duration;

use gpui::{ParentElement, TestAppContext, VisualTestContext};

use super::source::COMPONENTS;
use super::{Count, open, redraw};
use crate::interaction::take_frame_requests;
use crate::samples::{SAMPLES, Sample};
use crate::v_flex;

/// Item 36: every component has a sample for the benchmarks to draw, and nothing else does.
#[test]
fn every_component_has_a_sample() {
    let mut samples: Vec<&str> = SAMPLES.iter().map(|s| s.name).collect();
    let mut components: Vec<&str> = COMPONENTS.iter().map(|(name, _, _)| *name).collect();
    samples.sort_unstable();
    components.sort_unstable();
    assert_eq!(samples, components);
}

/// How often the host window is drawn while the clock runs on for `span`, in the 16 ms steps of
/// a display, once it has settled.
fn redraws_over(cx: &mut VisualTestContext, count: &Count, span: Duration) -> usize {
    let before = count.get();
    let step = Duration::from_millis(16);
    let mut t = Duration::ZERO;
    while t < span {
        cx.executor().advance_clock(step);
        cx.run_until_parked();
        t += step;
    }
    count.get() - before
}

/// Opens a window with `samples` in it, as many instances of each as makes it busy, and says how
/// often it is drawn while nothing happens to it for `span`.
fn idle_redraws(cx: &mut TestAppContext, samples: &'static [Sample], span: Duration) -> usize {
    let count = Count::default();
    let c = count.clone();
    let cx = open(cx, move |_, _| {
        c.bump();
        v_flex().children(
            samples
                .iter()
                .flat_map(|s| (0..s.instances).map(|n| (s.build)(n))),
        )
    });
    cx.update(|window, cx| {
        for setup in samples.iter().filter_map(|s| s.setup) {
            setup(window, cx);
        }
    });
    cx.run_until_parked();
    // Let anything that settles settle: a first-frame measurement, a toast's entrance and its
    // going again when its time is up.
    cx.executor().advance_clock(Duration::from_secs(30));
    cx.run_until_parked();
    redraws_over(cx, &count, span)
}

/// Opens a window with `samples` in it and settles it as `idle_redraws` does, then draws it from
/// scratch once more and says how many frames the components asked for while drawing.
///
/// This is what the counts of redraws cannot see: GPUI's test platform never runs the callbacks of
/// the next frame, so a component that asks for a frame at every draw, as `with_animation` does or
/// a transition that never settles, shows no redraws there while it keeps the real window busy.
/// Drawing is when a component asks, and so drawing is what is counted.
fn frames_asked_for_when_idle(cx: &mut TestAppContext, samples: &'static [Sample]) -> usize {
    let cx = open(cx, move |_, _| {
        v_flex().children(
            samples
                .iter()
                .flat_map(|s| (0..s.instances).map(|n| (s.build)(n))),
        )
    });
    cx.update(|window, cx| {
        for setup in samples.iter().filter_map(|s| s.setup) {
            setup(window, cx);
        }
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(30));
    cx.run_until_parked();
    redraw(cx);
    take_frame_requests();
    redraw(cx);
    take_frame_requests()
}

/// Item 35: drawing a component that has settled asks for no more frames, whatever it is, a
/// spinner included: it wakes itself with a timer.
#[gpui::test]
fn drawing_an_idle_window_of_each_component_asks_for_no_frames(cx: &mut TestAppContext) {
    let mut asking = Vec::new();
    for sample in SAMPLES {
        let asked = frames_asked_for_when_idle(cx, std::slice::from_ref(sample));
        if asked != 0 {
            asking.push(format!("{} ({asked})", sample.name));
        }
    }
    assert!(
        asking.is_empty(),
        "frames asked for by a settled window, by component: {}",
        asking.join(", ")
    );
}

/// Item 35: nor does a page of all of them together.
#[gpui::test]
fn drawing_an_idle_window_of_every_component_asks_for_no_frames(cx: &mut TestAppContext) {
    assert_eq!(frames_asked_for_when_idle(cx, SAMPLES), 0);
}

/// Item 35: a window that shows a component and has nothing happening to it is not drawn again,
/// whatever the component is. A component that animates on its own says so in its sample and is
/// held to its own test.
#[gpui::test]
fn an_idle_window_of_each_component_is_not_redrawn(cx: &mut TestAppContext) {
    let mut busy = Vec::new();
    for sample in SAMPLES.iter().filter(|s| !s.animated) {
        let redraws = idle_redraws(cx, std::slice::from_ref(sample), Duration::from_secs(5));
        if redraws != 0 {
            busy.push(format!("{} ({redraws})", sample.name));
        }
    }
    assert!(
        busy.is_empty(),
        "idle windows redrawn in 5 s, by component: {}",
        busy.join(", ")
    );
}

/// Item 35: nor is a page of all of them together.
#[gpui::test]
fn an_idle_window_of_every_component_is_not_redrawn(cx: &mut TestAppContext) {
    static STILL: std::sync::LazyLock<Vec<Sample>> =
        std::sync::LazyLock::new(|| SAMPLES.iter().filter(|s| !s.animated).copied().collect());
    assert_eq!(idle_redraws(cx, &STILL, Duration::from_secs(5)), 0);
}

/// What a window busy with a sample, or a page of several components, may take to draw from
/// scratch: a quarter of the 16.7 ms of a 60 Hz frame, so that the rest of the frame is left for
/// the application. The same in a debug build as in a release one: the workspace's dependencies
/// are optimised in both (`Cargo.toml`), and the slowest sample takes under half of it.
const BUDGET: Duration = Duration::from_millis(4);

/// `DRAW_BUDGET_MS` replaces the budget on a machine slower than the development one (a shared
/// CI runner); a budget that is too tight there says nothing about the code.
fn budget() -> Duration {
    std::env::var("DRAW_BUDGET_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(BUDGET)
}

/// What a thousand controls in one window may take: six frames. No page has that many; this keeps
/// the cost of a big window from running away.
const STRESS_BUDGET: Duration = Duration::from_millis(100);

/// How many times the sum of the components' own draws they may cost drawn together in one
/// window. They cost about twice that, from GPUI's per-element work growing with the size of the
/// window; a component that does work over all the others, or an effect growing as the square of
/// the controls in a window, would be far more.
const TOGETHER: u32 = 3;

/// The least of several from-scratch draws of a window holding `count` instances of each of
/// `samples`. The least is what load on the machine cannot make better than it is.
fn least_draw(cx: &mut TestAppContext, samples: Vec<(Sample, usize)>) -> Duration {
    let cx = open(cx, move |_, _| {
        v_flex().children(
            samples
                .iter()
                .flat_map(|(s, count)| (0..*count).map(|n| (s.build)(n)))
                .collect::<Vec<_>>(),
        )
    });
    (0..15)
        .map(|_| {
            let start = std::time::Instant::now();
            redraw(cx);
            start.elapsed()
        })
        .min()
        .unwrap()
}

/// Item 36: a window busy with any one component is drawn from scratch within the budget.
#[gpui::test]
fn a_window_busy_with_each_component_draws_within_the_budget(cx: &mut TestAppContext) {
    let mut over = Vec::new();
    for sample in SAMPLES {
        let least = least_draw(cx, vec![(*sample, sample.instances)]);
        if least > budget() {
            over.push(format!("{} ({least:?})", sample.name));
        }
    }
    assert!(
        over.is_empty(),
        "over the budget of {:?} to draw: {}",
        budget(),
        over.join(", ")
    );
}

/// Item 36: a page with one of every component, the components together as a page has them, is
/// drawn within the same budget.
#[gpui::test]
fn a_page_with_one_of_every_component_draws_within_the_budget(cx: &mut TestAppContext) {
    let page = SAMPLES.iter().map(|s| (*s, s.instances.min(1))).collect();
    let least = least_draw(cx, page);
    assert!(
        least <= budget(),
        "{least:?} to draw, over the budget of {:?}",
        budget()
    );
}

/// Item 36: components drawn together cost little more than drawn apart, and a thousand of them in
/// one window stay within a ceiling. A component that is cheap alone and dear beside others, or
/// whose cost grows with the window it is in, shows here and in no test of one component.
#[gpui::test]
fn the_components_together_cost_little_more_than_apart(cx: &mut TestAppContext) {
    let apart: Duration = SAMPLES
        .iter()
        .map(|s| least_draw(cx, vec![(*s, s.instances)]))
        .sum();
    let together = least_draw(cx, SAMPLES.iter().map(|s| (*s, s.instances)).collect());
    assert!(
        together <= apart * TOGETHER,
        "together {together:?}, apart {apart:?} in sum: more than {TOGETHER} times"
    );
    assert!(
        together <= STRESS_BUDGET,
        "a window of every sample's instances took {together:?}, over {STRESS_BUDGET:?}"
    );
}
