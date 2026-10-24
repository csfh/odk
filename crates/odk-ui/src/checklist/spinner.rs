use gpui::{ParentElement, Styled, TestAppContext};
use odk_a11y::{Describe, Role};
use odk_theme::component::SpinnerStyle;

use super::{FIXTURES, open, publish_fixture, redraw, reduce_motion};
use crate::spinner::animates;
use crate::{Button, Density, Size, Spinner, components, h_flex};

/// Items 14, 16: a spinner is a busy Progress named "Loading" unless told otherwise.
#[test]
fn reports_progress_and_busy() {
    let a = Spinner::new("s").accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.busy),
        (Role::Progress, "Loading", true)
    );
    assert_eq!(
        Spinner::new("s").label("Saving").accessible().name,
        "Saving"
    );
}

/// Items 19, 20: it turns once per the foundation's cycle, and stands still, still shown, with
/// reduced motion.
#[gpui::test]
fn stands_still_with_reduced_motion(cx: &mut TestAppContext) {
    open(cx, |_, _| Spinner::new("s"));
    let drawn = || crate::drawn::last::<SpinnerStyle>("s");
    assert!(animates(&drawn()));
    assert_eq!(drawn().turn, crate::foundation().motion.cycle);
    reduce_motion();
    assert!(!animates(&components().spinner(Density::Default)));
}

/// A spinner is a square of the density's icon size, a `Size` a step from it, its own density
/// winning; standing alone it is secondary text in every fixture.
#[gpui::test]
fn its_size_follows_the_density(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        h_flex()
            .child(Spinner::new("sm").size(Size::Sm))
            .child(Spinner::new("md"))
            .child(Spinner::new("lg").size(Size::Lg))
            .child(
                Spinner::new("own")
                    .density(Density::Comfortable)
                    .size(Size::Sm),
            )
    });
    for (id, d) in [
        ("sm", Density::Compact),
        ("md", Density::Default),
        ("lg", Density::Comfortable),
        ("own", Density::Comfortable),
    ] {
        let part: &'static str = Box::leak(format!("{id}/spinner").into_boxed_str());
        let b = cx.debug_bounds(part).unwrap();
        let side = gpui::px(d.metrics().icon_size);
        assert_eq!((b.size.width, b.size.height), (side, side), "{id}");
    }
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let s = crate::drawn::last::<SpinnerStyle>("md");
        assert_eq!(s, components().spinner(Density::Default), "{theme}");
        assert_eq!(s.colour, components().semantic().text.secondary);
    }
}

/// One spinner everywhere: a loading button's is the same `Spinner`, at the button's icon size.
#[gpui::test]
fn a_loading_button_uses_the_same_spinner(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Button::new("b").label("Save").loading(true));
    let b = cx.debug_bounds("b/spinner/spinner").unwrap();
    let side = gpui::px(Density::Default.metrics().icon_size);
    assert_eq!((b.size.width, b.size.height), (side, side));
}

/// Renders of the host window while the clock runs for `span` in 16 ms steps, as a display
/// would present them.
fn renders_over(
    cx: &mut gpui::VisualTestContext,
    count: &super::Count,
    span: std::time::Duration,
) -> usize {
    let before = count.get();
    let step = std::time::Duration::from_millis(16);
    let mut t = std::time::Duration::ZERO;
    while t < span {
        cx.executor().advance_clock(step);
        cx.run_until_parked();
        t += step;
    }
    count.get() - before
}

/// Item 19, performance: a spinner redraws when its glyph changes, `FRAMES` times per turn, and
/// not on every frame the display could show: an idle window with one spinner in it is not a
/// window that repaints at 60 Hz.
#[gpui::test]
fn it_redraws_once_per_glyph_not_once_per_frame(cx: &mut TestAppContext) {
    let count = super::Count::default();
    let c = count.clone();
    let cx = open(cx, move |_, _| {
        c.bump();
        Spinner::new("s")
    });
    let turn = crate::foundation().motion.cycle;
    let renders = renders_over(cx, &count, turn);
    let glyphs = 8;
    assert!(
        (glyphs - 1..=glyphs + 2).contains(&renders),
        "{renders} renders in one turn of {glyphs} glyphs (a 16 ms display would draw {})",
        turn.as_millis() / 16
    );
}

/// Item 19, performance: a spinner that cannot be seen, scrolled or clipped out of its parent,
/// asks for nothing: a gallery with spinners further down the page is idle at its top.
#[gpui::test]
fn a_clipped_spinner_causes_no_redraws(cx: &mut TestAppContext) {
    let count = super::Count::default();
    let c = count.clone();
    let cx = open(cx, move |_, _| {
        c.bump();
        crate::v_flex()
            .w(gpui::px(100.0))
            .h(gpui::px(10.0))
            .overflow_hidden()
            .child(gpui::div().h(gpui::px(50.0)).flex_shrink_0())
            .child(Spinner::new("s"))
    });
    let turn = crate::foundation().motion.cycle;
    assert_eq!(renders_over(cx, &count, turn * 2), 0);
}

/// Item 20: with reduced motion nothing is ever asked for.
#[gpui::test]
fn a_still_spinner_causes_no_redraws(cx: &mut TestAppContext) {
    reduce_motion();
    let count = super::Count::default();
    let c = count.clone();
    let cx = open(cx, move |_, _| {
        c.bump();
        Spinner::new("s")
    });
    let turn = crate::foundation().motion.cycle;
    assert_eq!(renders_over(cx, &count, turn * 2), 0);
}

/// Item 19, performance: spinners turn together, so a window of them is redrawn once per glyph,
/// not once per glyph per spinner: eight loading buttons that started at different moments must
/// not cost eight times the redraws.
#[gpui::test]
fn many_spinners_share_their_redraws(cx: &mut TestAppContext) {
    let count = super::Count::default();
    let c = count.clone();
    let cx = open(cx, move |_, _| {
        c.bump();
        crate::h_flex()
            .child(Spinner::new("a"))
            .child(Spinner::new("b"))
            .child(Spinner::new("c"))
            .child(Spinner::new("d"))
    });
    // Spinners that appear later than the first are not in step with it unless the phase is shared.
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(37));
    cx.run_until_parked();
    let turn = crate::foundation().motion.cycle;
    let renders = renders_over(cx, &count, turn);
    assert!(
        renders <= 10,
        "{renders} renders in one turn of 8 glyphs for 4 spinners"
    );
}
