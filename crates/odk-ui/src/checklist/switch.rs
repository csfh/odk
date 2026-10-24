use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};

use super::{open, press, redraw, reduce_motion};
use crate::{Button, Switch, v_flex};

type Seen = Rc<RefCell<Vec<bool>>>;

fn record(seen: &Seen) -> impl Fn(bool, &mut gpui::Window, &mut gpui::App) + 'static {
    let seen = seen.clone();
    move |v, _, _| seen.borrow_mut().push(v)
}

fn click(cx: &mut VisualTestContext, part: &'static str) {
    let b = cx.debug_bounds(part).unwrap();
    let at = gpui::point(
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    );
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
}

/// How far the thumb is from the track's left edge.
fn thumb_offset(cx: &mut VisualTestContext) -> gpui::Pixels {
    let track = cx.debug_bounds("s/track").unwrap();
    let thumb = cx.debug_bounds("s/thumb").unwrap();
    thumb.left() - track.left()
}

/// Items 8, 9, 10: Tab reaches the switch; Space and Enter both toggle it.
#[gpui::test]
fn space_and_enter_toggle(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Switch::new("s", "Wi-Fi").on_change(record(&s))
    });
    press(cx, "tab space enter");
    assert_eq!(*seen.borrow(), [true, false]);
}

/// A click on the track or the label toggles it once; the label gives the track focus.
#[gpui::test]
fn clicks_toggle_once_and_the_label_focuses_the_track(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Switch::new("s", "Wi-Fi").on_change(record(&s))
    });
    click(cx, "s/label");
    press(cx, "space");
    click(cx, "s/track");
    assert_eq!(*seen.borrow(), [true, false, true]);
}

/// Item 6: a disabled switch is skipped by Tab and ignores clicks.
#[gpui::test]
fn a_disabled_switch_is_skipped(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Rc::new(Cell::new(0)));
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(
                Switch::new("s", "Wi-Fi")
                    .disabled(true)
                    .on_change(record(&s)),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.set(a.get() + 1)),
            )
    });
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab went past it");
    click(cx, "s/track");
    click(cx, "s/label");
    assert!(seen.borrow().is_empty());
}

/// Items 14, 15, 16.
#[test]
fn reports_a_switch_and_its_state() {
    let a = Switch::new("s", "Wi-Fi").checked(true).accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.selected),
        (Role::Switch, "Wi-Fi", Some(true))
    );
    assert!(Switch::new("s", "x").disabled(true).accessible().disabled);
}

/// Items 19, 22: the thumb slides over the foundation's `normal`, on its curve, starting where
/// the switch is.
#[gpui::test]
fn the_thumb_slides_over_motion_normal(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Switch::new("s", "Wi-Fi"));
    let motion = crate::foundation().motion.clone();
    let normal = motion.normal;
    let off = thumb_offset(cx);
    press(cx, "tab space");
    redraw(cx);
    assert_eq!(thumb_offset(cx), off, "the slide starts where the thumb is");
    cx.executor().advance_clock(normal / 2);
    redraw(cx);
    let half = thumb_offset(cx);
    assert_eq!(
        crate::switch::drawn::last("s").amount,
        motion.easing.apply(0.5)
    );
    cx.executor().advance_clock(normal);
    redraw(cx);
    let on = thumb_offset(cx);
    assert!(off < half && half < on, "{off:?} < {half:?} < {on:?}");
    let eased = off + (on - off) * motion.easing.apply(0.5);
    assert!((half - eased).abs() < px(0.5), "eased at half the time");
}

/// Item 20: with reduced motion the thumb moves at once.
#[gpui::test]
fn the_thumb_moves_at_once_with_reduced_motion(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Switch::new("s", "Wi-Fi").default_checked(false));
    reduce_motion();
    let off = thumb_offset(cx);
    press(cx, "tab space");
    redraw(cx);
    assert!(thumb_offset(cx) > off, "already at the end");
    cx.executor().advance_clock(Duration::from_secs(1));
    redraw(cx);
    let settled = thumb_offset(cx);
    press(cx, "space");
    redraw(cx);
    assert_eq!(thumb_offset(cx), off);
    assert!(settled > off);
}

/// Item 32: toggling, focusing, hovering, pressing and disabling move nothing but the thumb.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    let off = Rc::new(Cell::new(false));
    let o = off.clone();
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(Switch::new("s", "Wi-Fi").disabled(o.get()))
            .child(Button::new("after").label("After"))
    });
    reduce_motion();
    let disable = off.clone();
    assert_still(
        cx,
        &["s/row", "s/track", "s/label", "after/button"],
        &[
            Step::Keys("tab"),
            Step::Keys("space"),
            Step::Hover("s/track"),
            Step::Press("s/track"),
            Step::Press("s/label"),
            Step::Keys("tab"),
            Step::Change("disabling it", Box::new(move || disable.set(true))),
        ],
    );
}

/// Item 23: a long label wraps in a narrow column beside a track of fixed size.
#[gpui::test]
fn a_long_label_wraps_beside_the_track(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(200.0)).child(Switch::new(
            "s",
            "Automatisch mit bekannten Netzwerken verbinden, auch im Energiesparmodus",
        ))
    });
    let (row, track, label) = (
        cx.debug_bounds("s/row").unwrap(),
        cx.debug_bounds("s/track").unwrap(),
        cx.debug_bounds("s/label").unwrap(),
    );
    assert!(
        row.size.width <= px(200.0) && label.right() <= px(200.0),
        "it fits"
    );
    assert!(
        label.size.height > track.size.height * 2.0,
        "the label wrapped"
    );
    assert!(
        track.top() < label.top() + track.size.height,
        "the track is on the first line"
    );
}

/// A switch that starts on shows its thumb at the end at once, without sliding there.
#[gpui::test]
fn a_switch_that_starts_on_does_not_slide_in(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Switch::new("s", "Wi-Fi").default_checked(true));
    let first = thumb_offset(cx);
    cx.executor()
        .advance_clock(crate::foundation().motion.fast * 2);
    redraw(cx);
    assert_eq!(thumb_offset(cx), first);
}

/// The track is a pill as high as its density's icon, 24, 28 or 32 wide, and its ring is drawn
/// outside it (items 11, 32).
#[gpui::test]
fn each_density_has_its_track(cx: &mut TestAppContext) {
    use odk_theme::foundation::Density;
    let cx = open(cx, |_, _| {
        v_flex().children(Density::ALL.map(|d| Switch::new(d.name(), "Wi-Fi").density(d)))
    });
    for (d, width) in Density::ALL.into_iter().zip([24.0, 28.0, 32.0]) {
        let b = cx
            .debug_bounds(Box::leak(format!("{}/track", d.name()).into_boxed_str()))
            .unwrap();
        assert_eq!(
            (b.size.width, b.size.height),
            (px(width), px(d.metrics().icon_size)),
            "{d:?}"
        );
    }
    press(cx, "tab");
    redraw(cx);
    assert!(crate::switch::drawn::last("compact").style.focus_visible);
}
