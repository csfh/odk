//! `NumberInput`: an `Input` whose text is a number, with steps and commits. What the text means
//! is tested in `number.rs`; these drive the field.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, TestAppContext, VisualTestContext};
use odk_a11y::Describe;

use super::stability::{Step, assert_still};
use super::{open, press, redraw};
use crate::input::{drawn, probe};
use crate::{Button, NumberInput, v_flex};

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
    // Typing over the selection is a step, and the run of typing after it another.
    press(cx, "shift-tab ctrl-z ctrl-z");
    assert_eq!(text(cx), "2.5", "what was typed undoes like any edit");
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
