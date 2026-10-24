//! `PasswordInput`: a masked `Input` with a button that shows and hides its text.

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::Describe;

use super::stability::{Step, assert_still};
use super::{open, press, redraw};
use crate::input::{layout_of, probe};
use crate::text_layout::MASK;
use crate::{Button, PasswordInput, v_flex};

/// What the field draws: its text as laid out.
fn drawn_text(cx: &mut VisualTestContext) -> String {
    layout_of(cx, "p", |text, _, _| text.shown_text().to_owned())
}

fn reveal(cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds("p/reveal/button")
        .expect("the show button is drawn")
}

/// A click on the show button shows the text and keeps the field's focus, text, selection and
/// size; another hides it again (items 8, 32).
#[gpui::test]
fn showing_the_text_keeps_everything_else(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        PasswordInput::new("p", "Password").default_value("hunter2")
    });
    press(cx, "tab");
    assert_eq!(drawn_text(cx), MASK.repeat(7));
    press(cx, "end shift-left shift-left");
    let (text, selection, _) = probe(cx, "p");
    assert_eq!(selection, 5..7);
    let size = cx.debug_bounds("p/box").unwrap();

    let at = reveal(cx).center();
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    assert_eq!(drawn_text(cx), "hunter2");
    let (after, kept, _) = probe(cx, "p");
    assert_eq!(
        (after.as_str(), kept.clone()),
        (text.as_str(), selection.clone())
    );
    assert_eq!(cx.debug_bounds("p/box").unwrap(), size);
    // Focus stayed in the field: typing replaces the selection.
    cx.simulate_input("!");
    assert_eq!(probe(cx, "p").0, "hunte!");

    let at = reveal(cx).center();
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    assert_eq!(drawn_text(cx), MASK.repeat(6));
}

/// Shown or not, the text stays a password: Ctrl+C copies nothing, and `Describe` reports no
/// value.
#[gpui::test]
fn shown_text_is_still_secret(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        PasswordInput::new("p", "Password").default_value("hunter2")
    });
    press(cx, "tab");
    let at = reveal(cx).center();
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    press(cx, "ctrl-a ctrl-c");
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|i| i.text()));
    assert_eq!(copied, None);
    let a = PasswordInput::new("p", "Password")
        .default_value("hunter2")
        .accessible();
    assert_eq!(a.value, None);
}

/// From the keyboard: Tab reaches the show button after the field, Space presses it, focus
/// stays on the button, and its name says what it will do.
#[gpui::test]
fn the_show_button_works_from_the_keyboard(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        PasswordInput::new("p", "Password").default_value("abc")
    });
    press(cx, "tab tab space");
    redraw(cx);
    assert_eq!(drawn_text(cx), "abc");
    assert!(
        crate::button::probe::last("p/reveal").style.focus_visible,
        "focus is still on the button"
    );
    press(cx, "space");
    redraw(cx);
    assert_eq!(drawn_text(cx), MASK.repeat(3));
}

/// Item 6: a disabled password field's show button is disabled too and skipped by Tab.
#[gpui::test]
fn a_disabled_field_disables_its_button(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                PasswordInput::new("p", "Password")
                    .default_value("abc")
                    .disabled(true),
            )
            .child(Button::new("after").label("After"))
    });
    press(cx, "tab");
    redraw(cx);
    assert!(crate::button::probe::last("after").style.focus_visible);
    assert!(!crate::button::probe::last("p/reveal").style.responds);
}

/// Item 32: showing and hiding moves nothing.
#[gpui::test]
fn nothing_moves_when_it_is_shown(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(260.0))
            .child(PasswordInput::new("p", "Password").default_value("correct horse"))
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &["p/box", "p/reveal/button", "after/button"],
        &[
            Step::Keys("tab"),
            Step::Press("p/reveal/button"),
            Step::Keys("tab space"),
            Step::Keys("space"),
        ],
    );
}
