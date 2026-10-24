use std::time::{Duration, Instant};

use gpui::{IntoElement as _, ParentElement as _, TestAppContext, VisualTestContext};

use super::{open, press, redraw, reduce_motion};
use crate::overlay::probe;
use crate::overlay::{Presence, Transition};
use crate::{Density, DropdownMenu, MenuItem, Select, Text};
use odk_theme::foundation::MotionDuration;

/// Items 19, 20: overlays use the foundation's durations, each its own way in and out, and
/// nothing with reduced motion.
#[gpui::test]
fn transitions_follow_the_motion_tokens(cx: &mut TestAppContext) {
    open(cx, |_, _| Text::new(""));
    let m = crate::foundation().motion.clone();
    let dialog = Transition::of(MotionDuration::Enter, MotionDuration::Exit);
    assert_eq!((dialog.enter, dialog.exit), (m.enter, m.exit));
    assert_ne!(m.enter, m.exit, "a dialog is slower in than out");
    reduce_motion();
    let dialog = Transition::of(MotionDuration::Enter, MotionDuration::Exit);
    assert_eq!(
        (dialog.enter, dialog.exit),
        (Duration::ZERO, Duration::ZERO)
    );
}

/// Item 22: a presence that comes in slowly and goes quickly reverses without a jump, at the
/// speed of the way it is going now.
#[test]
fn a_transition_of_two_lengths_reverses_without_a_jump() {
    let t = Transition {
        enter: Duration::from_millis(200),
        exit: Duration::from_millis(100),
    };
    let t0 = Instant::now();
    let mut p = Presence::default();
    p.set(true, t0, t);
    let half = t0 + Duration::from_millis(100);
    assert_close(p.amount(half, t), 0.5);
    p.set(false, half, t);
    assert_close(p.amount(half, t), 0.5);
    // Going out takes `exit`: half of it is left.
    assert_close(p.amount(half + Duration::from_millis(25), t), 0.25);
    assert_eq!(p.amount(half + Duration::from_millis(50), t), None);
}

/// Item 21: a closing overlay is still drawn until its exit has run.
#[test]
fn exits_run_before_an_overlay_goes() {
    let d = Duration::from_millis(100);
    let t0 = Instant::now();
    let mut p = Presence::default();
    p.set(true, t0, d);
    p.set(false, t0 + d, d);
    assert!(p.amount(t0 + d + d / 2, d).is_some());
    assert!(p.amount(t0 + d * 2, d).is_none());
}

/// Item 22: a transition reversed partway carries on from where it had got to, at the
/// same speed, in both directions.
#[test]
fn a_reversed_transition_does_not_jump() {
    let d = Duration::from_millis(100);
    let t0 = Instant::now();
    let mut p = Presence::default();
    p.set(true, t0, d);
    p.set(false, t0 + d, d);
    // Reopened a quarter of the way through closing, so still 0.75 open.
    let reopened = t0 + d + d / 4;
    p.set(true, reopened, d);
    assert_close(p.amount(reopened, d), 0.75);
    // Back to fully open in the quarter of the duration that is left.
    assert_close(p.amount(reopened + d / 8, d), 0.875);
    assert_close(p.amount(reopened + d / 4, d), 1.0);
    // Closed again halfway through opening: it closes from 0.5 and is gone after half.
    let mut p = Presence::default();
    p.set(true, t0, d);
    let closed = t0 + d / 2;
    p.set(false, closed, d);
    assert_close(p.amount(closed, d), 0.5);
    assert_close(p.amount(closed + d / 4, d), 0.25);
    assert_eq!(p.amount(closed + d / 2, d), None);
}

fn assert_close(amount: Option<f32>, want: f32) {
    let amount = amount.expect("still drawn");
    assert!((amount - want).abs() < 1e-3, "{amount} is not {want}");
}

/// Moves GPUI's clock on by `by` and draws a frame at the new time.
fn wait(cx: &mut VisualTestContext, by: Duration) {
    cx.executor().advance_clock(by);
    redraw(cx);
}

/// Items 19, 21, 22, in a window: a menu fades in over the foundation's `normal` and out over
/// `exit`, and reopened a quarter of the way through closing it carries on from three-quarters
/// open instead of starting again from nothing.
#[gpui::test]
fn a_menu_reopened_while_closing_does_not_jump(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DropdownMenu::new("file", "File").menu(|m| m.item(MenuItem::new("New")))
    });
    // The menu surface is on component tokens: in over `normal`, out over `exit`.
    let (normal, exit) = {
        let m = crate::foundation().motion.clone();
        (m.normal, m.exit)
    };
    press(cx, "tab enter");
    assert_close(probe::last("file"), 0.0);
    wait(cx, normal / 2);
    assert_close(probe::last("file"), 0.5);
    wait(cx, normal);
    assert_close(probe::last("file"), 1.0);

    press(cx, "escape");
    wait(cx, exit / 4);
    assert_close(probe::last("file"), 0.75);
    // Focus went back to the button, so Enter opens the menu again.
    press(cx, "enter");
    assert_close(probe::last("file"), 0.75);
    wait(cx, normal / 8);
    assert_close(probe::last("file"), 0.875);
    wait(cx, normal / 8);
    assert_close(probe::last("file"), 1.0);
}

/// Items 21, 22: a select's list fades in and out on the motion tokens, and reopened a quarter
/// of the way through closing it carries on from there.
#[gpui::test]
fn a_select_list_reopened_while_closing_does_not_jump(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Select::new("sl", "Fruit")
            .item("a", "Apple")
            .item("b", "Banana")
    });
    let (normal, exit) = {
        let m = crate::foundation().motion.clone();
        (m.normal, m.exit)
    };
    press(cx, "tab enter");
    assert_close(probe::last("sl"), 0.0);
    wait(cx, normal / 2);
    assert_close(probe::last("sl"), 0.5);
    wait(cx, normal);
    assert_close(probe::last("sl"), 1.0);

    press(cx, "escape");
    wait(cx, exit / 4);
    assert_close(probe::last("sl"), 0.75);
    // Focus went back to the box, so Enter opens the list again.
    press(cx, "enter");
    assert_close(probe::last("sl"), 0.75);
    wait(cx, normal / 4);
    assert_close(probe::last("sl"), 1.0);
}

/// Item 20: an application can turn reduced motion on for its users. Transitions and the
/// spinner follow at once, the choice survives a theme reload, and it can be turned off again.
#[gpui::test]
fn an_application_can_reduce_motion(cx: &mut TestAppContext) {
    use crate::spinner::animates;
    let cx = open(cx, |_, _| Text::new(""));
    assert!(!crate::foundation().motion.reduced);
    cx.update(|_, cx| crate::set_reduced_motion(cx, true));
    assert_eq!(
        Transition::of(MotionDuration::Exit, MotionDuration::Exit).exit,
        Duration::ZERO
    );
    let spinner = || crate::components().spinner(Density::Default);
    assert!(!animates(&spinner()));

    // Omarchy changes theme: the new tokens arrive without the preference.
    let reloaded = odk_theme::ThemeSnapshot::from_toml(crate::theme::TEST_COLORS, None).unwrap();
    assert!(!reloaded.reduced_motion);
    crate::theme::publish(std::sync::Arc::new(reloaded));
    assert!(
        crate::foundation().motion.reduced,
        "the preference outlives the reload"
    );

    cx.update(|_, cx| crate::set_reduced_motion(cx, false));
    assert_eq!(
        Transition::of(MotionDuration::Exit, MotionDuration::Exit).exit,
        crate::foundation().motion.exit
    );
    assert!(animates(&spinner()));
}

/// Item 20: with reduced motion a select's list, on the foundation's motion, is fully shown on
/// the frame it opens and gone on the frame it closes.
#[gpui::test]
fn a_select_list_appears_at_once_with_reduced_motion(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Select::new("sl", "Fruit")
            .item("a", "Apple")
            .item("b", "Banana")
    });
    reduce_motion();
    press(cx, "tab enter");
    assert_eq!(probe::last("sl"), Some(1.0), "shown at once");
    press(cx, "escape");
    assert_eq!(probe::last("sl"), None, "gone at once");
}

/// Items 20, 22, 35, 39: an interrupted transition lands where the state is. A switch toggled
/// repeatedly, faster than it can slide, ends on the last state it was given; reduced motion
/// turned on halfway moves it there at once; a theme change halfway neither restarts nor stops
/// it; and once it has arrived the window asks for no more frames.
#[gpui::test]
fn an_interrupted_transition_lands_on_the_final_state(cx: &mut TestAppContext) {
    use super::{FIXTURES, publish_fixture};
    use crate::Switch;
    use crate::interaction::take_frame_requests;
    use crate::switch::drawn;

    let cx = open(cx, |_, _| Switch::new("s", "Wi-Fi"));
    let normal = crate::foundation().motion.normal;
    let amount = || drawn::last("s").amount;
    assert_eq!(amount(), 0.0);

    // Five toggles, a tenth of the slide apart: the switch is on, and gets there.
    press(cx, "tab");
    for _ in 0..5 {
        press(cx, "space");
        wait(cx, normal / 10);
    }
    let partway = amount();
    assert!(partway > 0.0 && partway < 1.0, "still moving: {partway}");
    wait(cx, normal * 2);
    assert_eq!(amount(), 1.0, "odd toggles: on");

    // A theme change halfway through sliding back keeps sliding, from where it was.
    press(cx, "space");
    wait(cx, normal / 2);
    let before = amount();
    assert!(before > 0.0 && before < 1.0);
    publish_fixture(FIXTURES[1].1);
    redraw(cx);
    let after = amount();
    assert!(
        (after - before).abs() < 0.05,
        "no restart or jump: {before} then {after}"
    );
    wait(cx, normal * 2);
    assert_eq!(amount(), 0.0, "and it arrives");

    // Reduced motion turned on halfway: the switch is where it is going, at once.
    press(cx, "space");
    wait(cx, normal / 2);
    assert!(amount() > 0.0 && amount() < 1.0);
    cx.update(|_, cx| crate::set_reduced_motion(cx, true));
    redraw(cx);
    assert_eq!(amount(), 1.0);

    // Arrived: nothing asks for another frame.
    take_frame_requests();
    redraw(cx);
    redraw(cx);
    assert_eq!(take_frame_requests(), 0, "idle once it has arrived");
}

/// Items 20, 35, 39: a component that goes while it is moving takes its animation with it: the
/// window asks for no frame on its behalf afterwards, and a fresh one starts settled.
#[gpui::test]
fn a_component_removed_mid_transition_leaves_no_animation_behind(cx: &mut TestAppContext) {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::interaction::take_frame_requests;
    use crate::{Progress, Switch};

    let present = Rc::new(Cell::new(true));
    let value = Rc::new(Cell::new(10.0));
    let (p, v) = (present.clone(), value.clone());
    let cx = open(cx, move |_, _| {
        if p.get() {
            crate::v_flex()
                .child(Switch::new("s", "Wi-Fi"))
                .child(Progress::new("p", "Upload").value(v.get()))
                .into_any_element()
        } else {
            Text::new("gone").into_any_element()
        }
    });
    let normal = crate::foundation().motion.normal;
    press(cx, "tab space");
    value.set(90.0);
    wait(cx, normal / 4);
    take_frame_requests();
    redraw(cx);
    assert!(take_frame_requests() > 0, "they are moving");

    present.set(false);
    redraw(cx);
    take_frame_requests();
    wait(cx, normal * 3);
    redraw(cx);
    assert_eq!(take_frame_requests(), 0, "nothing moves on behalf of them");

    // They come back settled where their state is, not partway from where they were.
    present.set(true);
    redraw(cx);
    assert_eq!(crate::switch::drawn::last("s").amount, 0.0, "off, as new");
    take_frame_requests();
    redraw(cx);
    assert_eq!(take_frame_requests(), 0);
}

/// Items 21, 35, 39: an overlay torn down while it fades, a menu on its way in or out, leaves
/// nothing running: once it has gone no frame is asked for on its behalf, and a new one starts
/// closed.
#[gpui::test]
fn an_overlay_removed_mid_fade_leaves_nothing_running(cx: &mut TestAppContext) {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::interaction::take_frame_requests;

    let present = Rc::new(Cell::new(true));
    let p = present.clone();
    let cx = open(cx, move |_, _| {
        if p.get() {
            DropdownMenu::new("file", "File")
                .menu(|m| m.item(MenuItem::new("New")))
                .into_any_element()
        } else {
            Text::new("gone").into_any_element()
        }
    });
    let normal = crate::foundation().motion.normal;
    press(cx, "tab enter");
    wait(cx, normal / 4);
    take_frame_requests();
    redraw(cx);
    assert!(take_frame_requests() > 0, "the menu is fading in");
    assert_close(probe::last("file"), 0.25);

    present.set(false);
    redraw(cx);
    take_frame_requests();
    wait(cx, normal * 3);
    redraw(cx);
    assert_eq!(take_frame_requests(), 0, "nothing fades on its behalf");

    present.set(true);
    redraw(cx);
    assert_eq!(probe::last("file"), None, "a new menu starts closed");
}
