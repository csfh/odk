use std::cell::Cell;
use std::rc::Rc;

use gpui::{ParentElement, Styled, TestAppContext, px};
use odk_a11y::{Describe, Role};
use odk_theme::component::{ProgressStyle, States};
use odk_theme::semantic::Role as Intent;

use super::{FIXTURES, open, publish_fixture, redraw, reduce_motion};
use crate::{Progress, Tone, v_flex};

type Drawn = (ProgressStyle, f32, f32);

fn drawn(id: &str) -> Drawn {
    crate::drawn::last::<Drawn>(id)
}

/// Items 14, 15, 16: a progress bar reports its name, its value as text and whether work goes on.
#[test]
fn reports_its_name_and_value() {
    let a = Progress::new("p", "Uploading photos")
        .value(42.0)
        .accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.value.as_deref()),
        (Role::Progress, "Uploading photos", Some("42%"))
    );
    assert!(a.busy);
}

/// Valid, empty, complete, negative, overflowing, NaN and infinite values all draw, the bar from
/// nothing to the whole track and never past it.
#[gpui::test]
fn any_value_draws_safely(cx: &mut TestAppContext) {
    let values = [
        ("half", 50.0, 0.5),
        ("none", 0.0, 0.0),
        ("done", 100.0, 1.0),
        ("below", -20.0, 0.0),
        ("past", 400.0, 1.0),
        ("nan", f64::NAN, 0.0),
        ("inf", f64::INFINITY, 1.0),
    ];
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(200.0))
            .children(values.map(|(id, value, _)| Progress::new(id, "Copying").value(value)))
    });
    for (id, _, want) in values {
        let (_, done, shown) = drawn(id);
        assert_eq!(
            (done, shown),
            (want, want),
            "{id}: no animation to a first value"
        );
        let track = cx.debug_bounds(leak(format!("{id}/track"))).unwrap();
        let bar = cx.debug_bounds(leak(format!("{id}/bar"))).unwrap();
        assert_eq!(bar.size.width, track.size.width * want, "{id}");
        assert!(bar.right() <= track.right() && bar.left() == track.left());
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Items 19, 20, 22: a new value moves the bar over the foundation's `normal` on its curve, and
/// at once with reduced motion.
#[gpui::test]
fn a_new_value_moves_the_bar(cx: &mut TestAppContext) {
    let value = Rc::new(Cell::new(20.0));
    let v = value.clone();
    let cx = open(cx, move |_, _| Progress::new("p", "Copying").value(v.get()));
    let normal = crate::foundation().motion.normal;
    value.set(60.0);
    redraw(cx);
    assert_eq!(drawn("p").2, 0.2, "starts where it was");
    cx.executor().advance_clock(normal / 2);
    redraw(cx);
    let half = 0.2 + 0.4 * crate::foundation().motion.easing.apply(0.5);
    assert!((drawn("p").2 - half).abs() < 0.02, "{}", drawn("p").2);
    cx.executor().advance_clock(normal);
    redraw(cx);
    assert_eq!(drawn("p").2, 0.6);
    reduce_motion();
    value.set(90.0);
    redraw(cx);
    assert_eq!(drawn("p").2, 0.9, "at once");
}

/// Items 1, 34: the bar is the accent's, or the tone's, on its tint, in every fixture; paused it
/// is grey and keeps its value; disabled both are greyer; in a monochrome theme it is grey and
/// still shows.
#[gpui::test]
fn it_draws_its_tokens_in_every_fixture(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Progress::new("a", "Upload").value(30.0))
            .child(
                Progress::new("ok", "Upload")
                    .value(100.0)
                    .tone(Tone::Success),
            )
            .child(Progress::new("paused", "Upload").value(30.0).paused(true))
            .child(Progress::new("off", "Upload").value(30.0).disabled(true))
    });
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        assert_eq!(
            drawn("a").0,
            c.progress(Intent::Accent, States::NONE),
            "{theme}"
        );
        assert_eq!(drawn("ok").0, c.progress(Intent::Success, States::NONE));
        assert_eq!(drawn("paused").0, c.progress(Intent::Neutral, States::NONE));
        assert_eq!(drawn("paused").1, 0.3, "paused keeps its value");
        assert_eq!(
            drawn("off").0,
            c.progress(Intent::Accent, States::NONE.disabled(true))
        );
        let s = drawn("a").0;
        let ground = c.semantic().surface.canvas;
        assert!(
            odk_theme::drawn_contrast(s.bar, s.track, ground) >= 3.0,
            "{theme}"
        );
    }
}

/// The label and the value are shown on one line above the bar when asked, the value from
/// `value_text` when given; the bar's height is the token's.
#[gpui::test]
fn its_label_and_value_are_text(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(240.0))
            .child(
                Progress::new("p", "Copying files")
                    .value(3.0)
                    .max(7.0)
                    .value_text("3 of 7 files")
                    .show_label(true)
                    .show_value(true),
            )
            .child(Progress::new("bare", "Copying"))
    });
    let (s, _, _) = drawn("p");
    assert_eq!(
        cx.debug_bounds("p/track").unwrap().size.height,
        px(s.height)
    );
    let (line, track) = (
        cx.debug_bounds("p/line").unwrap(),
        cx.debug_bounds("p/track").unwrap(),
    );
    assert_eq!(track.top(), line.bottom() + px(s.gap));
    assert!(cx.debug_bounds("p/value").is_some());
    assert!(
        cx.debug_bounds("bare/line").is_none(),
        "a bare bar has no line"
    );
}
