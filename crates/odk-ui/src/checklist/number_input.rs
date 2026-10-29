//! `NumberInput`: an `Input` whose text is a number, with steps and commits. What the text means
//! is tested in `number.rs`; these drive the field.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, TestAppContext, VisualTestContext, px};
use odk_a11y::Describe;

use super::stability::{Step, assert_still};
use super::{open, press, redraw};
use crate::input::{drawn, probe};
use crate::{Button, FormField, Icon, NumberInput, v_flex};

type Seen = Rc<RefCell<Vec<Option<f64>>>>;

fn text(cx: &mut VisualTestContext) -> String {
    probe(cx, "n").0
}

fn quantity() -> NumberInput {
    NumberInput::new("n", "Quantity").min(-5.0).max(10.0)
}

/// Half-typed numbers stand while they are typed: nothing is clamped or rewritten, and
/// `on_change` hears `None` until the text is a number.
#[gpui::test]
fn half_typed_numbers_stand_while_typed(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        let s = s.clone();
        quantity().on_change(move |v, _, _| s.borrow_mut().push(v))
    });
    press(cx, "tab");
    cx.simulate_input("-");
    assert_eq!(text(cx), "-");
    cx.simulate_input("1.");
    assert_eq!(text(cx), "-1.");
    cx.simulate_input("5");
    cx.simulate_input("0");
    assert_eq!(text(cx), "-1.50", "not tidied while typed");
    cx.simulate_input("0");
    assert_eq!(text(cx), "-1.500");
    press(cx, "ctrl-a");
    cx.simulate_input("99");
    assert_eq!(text(cx), "99", "not clamped while typed");
    assert_eq!(seen.borrow()[0], None, "`-` is no number yet");
    assert_eq!(seen.borrow()[1], Some(-1.0));
    assert!(
        drawn::last("n").invalid_mark.is_none(),
        "not judged while typed"
    );
    assert!(
        drawn::description("n").is_none(),
        "no error sentence while it is being typed"
    );
}

/// Enter and leaving the field commit: a number is clamped and tidied; text that is no number
/// is kept, and the field shows itself invalid until it is one.
#[gpui::test]
fn enter_and_leaving_commit(cx: &mut TestAppContext) {
    let submitted = Seen::default();
    let s = submitted.clone();
    let cx = open(cx, move |_, _| {
        let s = s.clone();
        v_flex()
            .child(quantity().on_submit(move |v, _, _| s.borrow_mut().push(v)))
            .child(Button::new("after").label("After"))
    });
    press(cx, "tab");
    cx.simulate_input("50");
    press(cx, "enter");
    assert_eq!(text(cx), "10");
    assert_eq!(*submitted.borrow(), [Some(10.0)]);
    press(cx, "ctrl-a");
    cx.simulate_input("2.50");
    press(cx, "tab");
    redraw(cx);
    assert_eq!(text(cx), "2.5", "tidied when it lost focus");
    press(cx, "shift-tab ctrl-a");
    cx.simulate_input("abc");
    press(cx, "tab");
    redraw(cx);
    assert_eq!(text(cx), "abc", "nothing typed is lost");
    assert!(drawn::last("n").invalid_mark.is_some(), "shown wrong");
    assert_eq!(
        drawn::description("n").as_deref(),
        Some("Enter a number"),
        "the error is text, not only the mark"
    );
    assert!(
        cx.debug_bounds("n/error").is_some(),
        "the sentence is drawn"
    );
    // Typing over the selection is a step, and the run of typing after it another.
    press(cx, "shift-tab ctrl-z ctrl-z");
    assert_eq!(text(cx), "2.5", "what was typed undoes like any edit");
}

/// A `FormField` description is kept, and the number error is joined onto it.
#[gpui::test]
fn a_form_field_joins_the_number_error_onto_its_description(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                FormField::new("qty", "Quantity")
                    .description("How many")
                    .child(quantity()),
            )
            .child(Button::new("after").label("After"))
    });
    press(cx, "tab");
    cx.simulate_input("abc");
    press(cx, "tab");
    redraw(cx);
    assert_eq!(
        drawn::description("n").as_deref(),
        Some("How many. Enter a number")
    );
    assert!(cx.debug_bounds("n/error").is_some());
}

/// Up and Down step and stop at the bounds; the buttons step too, and the one that cannot move
/// is disabled; a click on one keeps focus in the field.
#[gpui::test]
fn up_down_and_the_buttons_step(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| quantity().default_value(9.0));
    press(cx, "tab up");
    assert_eq!(text(cx), "10");
    press(cx, "up");
    assert_eq!(text(cx), "10", "held at the top");
    redraw(cx);
    assert!(!crate::button::probe::last("n/increment").style.responds);
    let down = cx.debug_bounds("n/decrement/button").unwrap();
    for _ in 0..3 {
        cx.simulate_click(down.center(), Modifiers::default());
    }
    redraw(cx);
    assert_eq!(text(cx), "7");
    cx.simulate_input("5");
    assert_eq!(
        text(cx),
        "75",
        "focus stayed in the field, the caret at the end"
    );
}

/// Tenths step without drift, from the keyboard.
#[gpui::test]
fn tenths_step_without_drift(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        NumberInput::new("n", "Gain").step(0.1).default_value(0.0)
    });
    press(cx, "tab");
    for _ in 0..7 {
        press(cx, "up");
    }
    assert_eq!(text(cx), "0.7");
}

/// A controlled field follows a new value, and an echo of what was typed does not overwrite
/// half-typed text.
#[gpui::test]
fn a_controlled_field_follows_new_values_only(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(3.0));
    let v = value.clone();
    let cx = open(cx, move |_, _| {
        let set = v.clone();
        quantity().value(*v.borrow()).on_change(move |n, _, _| {
            if let Some(n) = n {
                *set.borrow_mut() = n;
            }
        })
    });
    assert_eq!(text(cx), "3");
    press(cx, "tab");
    cx.simulate_input("-");
    redraw(cx);
    assert_eq!(text(cx), "-", "the old value did not come back");
    cx.simulate_input("2");
    redraw(cx);
    assert_eq!((text(cx).as_str(), *value.borrow()), ("-2", -2.0));
    *value.borrow_mut() = 7.0;
    redraw(cx);
    assert_eq!(text(cx), "7", "a new value from outside replaces the text");
    assert_eq!(
        quantity().value(7.0).accessible().value.as_deref(),
        Some("7")
    );
}

/// Item 6: a read-only field does not step, and its buttons are disabled.
#[gpui::test]
fn a_read_only_field_does_not_step(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| quantity().default_value(1.0).read_only(true));
    press(cx, "tab up down");
    assert_eq!(text(cx), "1");
    redraw(cx);
    assert!(!crate::button::probe::last("n/increment").style.responds);
    assert!(!crate::button::probe::last("n/decrement").style.responds);
}

/// Item 32: stepping and committing move nothing.
#[gpui::test]
fn nothing_moves_when_it_steps(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(quantity().default_value(1.0))
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &[
            "n/box",
            "n/decrement/button",
            "n/increment/button",
            "after/button",
        ],
        &[
            Step::Keys("tab up up"),
            Step::Press("n/increment/button"),
            Step::Keys("ctrl-a 9 9 enter"),
            Step::Keys("tab"),
        ],
    );
}

type Committed = Rc<RefCell<Vec<f64>>>;

/// `on_commit` hears the number a field settles on and not the keystrokes on the way to it:
/// typing `80` passes through `8`, which `on_change` hears and a setting must not be written
/// from. Enter settles on a number, clamped, and a second Enter on the same number is not a
/// commit.
#[gpui::test]
fn a_commit_is_the_number_the_field_settles_on(cx: &mut TestAppContext) {
    let (committed, changed) = (Committed::default(), Seen::default());
    let (c, s) = (committed.clone(), changed.clone());
    let cx = open(cx, move |_, _| {
        let (c, s) = (c.clone(), s.clone());
        quantity()
            .default_value(2.0)
            .on_change(move |v, _, _| s.borrow_mut().push(v))
            .on_commit(move |v, _, _| c.borrow_mut().push(v))
    });
    press(cx, "tab");
    cx.simulate_input("7");
    assert_eq!(
        *changed.borrow(),
        [Some(7.0)],
        "the edit is heard as it is made"
    );
    assert!(committed.borrow().is_empty(), "and is no commit");
    press(cx, "enter");
    assert_eq!(*committed.borrow(), [7.0]);
    press(cx, "enter");
    assert_eq!(
        *committed.borrow(),
        [7.0],
        "the same number again is not a commit"
    );

    press(cx, "ctrl-a");
    cx.simulate_input("99");
    press(cx, "enter");
    assert_eq!(
        *committed.borrow(),
        [7.0, 10.0],
        "a number out of bounds commits clamped"
    );
}

/// A step is a commit, by Up and Down and by the buttons, and a step that holds at a bound
/// leaves the number where it was, so it is none.
#[gpui::test]
fn a_step_commits_unless_it_holds_at_a_bound(cx: &mut TestAppContext) {
    let committed = Committed::default();
    let c = committed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        quantity()
            .default_value(9.0)
            .on_commit(move |v, _, _| c.borrow_mut().push(v))
    });
    press(cx, "tab up");
    assert_eq!(*committed.borrow(), [10.0]);
    press(cx, "up");
    assert_eq!(*committed.borrow(), [10.0], "held at the top: still 10");
    press(cx, "down");
    assert_eq!(*committed.borrow(), [10.0, 9.0]);
    let up = cx.debug_bounds("n/increment/button").unwrap();
    cx.simulate_click(up.center(), Modifiers::default());
    redraw(cx);
    assert_eq!(
        *committed.borrow(),
        [10.0, 9.0, 10.0],
        "the button steps too"
    );
}

/// Leaving a field commits what was typed, and leaving one untouched, or holding text that is
/// no number, commits nothing: tabbing through a form must not write a setting.
#[gpui::test]
fn leaving_commits_a_changed_number_and_nothing_else(cx: &mut TestAppContext) {
    let committed = Committed::default();
    let c = committed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        v_flex()
            .child(
                quantity()
                    .default_value(2.0)
                    .on_commit(move |v, _, _| c.borrow_mut().push(v)),
            )
            .child(Button::new("after").label("After"))
    });
    press(cx, "tab tab shift-tab");
    press(cx, "tab");
    assert!(
        committed.borrow().is_empty(),
        "passed through untouched: {:?}",
        committed.borrow()
    );

    press(cx, "shift-tab ctrl-a");
    cx.simulate_input("abc");
    press(cx, "tab");
    assert!(
        committed.borrow().is_empty(),
        "text that is no number is no commit"
    );

    press(cx, "shift-tab ctrl-a");
    cx.simulate_input("4");
    press(cx, "tab");
    assert_eq!(
        *committed.borrow(),
        [4.0],
        "a changed number commits when the field is left"
    );
}

/// A number the application gives is where a commit is measured from: the field is not told it
/// committed what it was handed, and a commit of another number after it is one.
#[gpui::test]
fn a_given_number_is_not_a_commit(cx: &mut TestAppContext) {
    let committed = Committed::default();
    let c = committed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        quantity()
            .value(6.0)
            .on_commit(move |v, _, _| c.borrow_mut().push(v))
    });
    press(cx, "tab enter");
    assert!(
        committed.borrow().is_empty(),
        "Enter on the number it was given"
    );
    press(cx, "ctrl-a");
    cx.simulate_input("3");
    press(cx, "enter");
    assert_eq!(*committed.borrow(), [3.0]);
}

/// The steppers draw ODK's minus and plus icons (ADR 0080), which a font cannot draw at two
/// sizes, each in the middle of its button.
#[gpui::test]
fn the_steppers_draw_the_minus_and_plus_icons(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| quantity().default_value(3.0));
    redraw(cx);
    crate::drawn::last_icon(Icon::Minus);
    crate::drawn::last_icon(Icon::Plus);
    for part in ["n/decrement", "n/increment"] {
        let slot: &'static str = Box::leak(format!("{part}/icon").into_boxed_str());
        let button: &'static str = Box::leak(format!("{part}/button").into_boxed_str());
        let off =
            cx.debug_bounds(button).unwrap().center() - cx.debug_bounds(slot).unwrap().center();
        assert!(
            off.x.abs() <= px(0.5) && off.y.abs() <= px(0.5),
            "{part}'s icon is off its middle by {off:?}"
        );
    }
}

/// An icon takes its button's text colour in each state, as a character would, so the stepper
/// that cannot move shows its icon dimmed.
#[gpui::test]
fn a_stepper_icon_takes_its_buttons_colours(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| quantity().default_value(10.0));
    redraw(cx);
    for (part, icon) in [("n/decrement", Icon::Minus), ("n/increment", Icon::Plus)] {
        let c = crate::button::probe::last(part).style.colours;
        assert_eq!(
            crate::drawn::last_icon(icon),
            [c.default, c.hover, c.pressed].map(|t| t.foreground),
            "{part}"
        );
    }
    assert!(!crate::button::probe::last("n/increment").style.responds);
    assert_ne!(
        crate::drawn::last_icon(Icon::Plus)[0],
        crate::drawn::last_icon(Icon::Minus)[0],
        "held at the top, the plus is dimmed and the minus is not"
    );
}
