//! `InputGroup` (ADR 0038, `docs/spec/odk-text-input.md`) on GPUI's headless test platform. The
//! field inside is an `Input`, tested in `input.rs`; this file tests the box around it.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Bounds, Modifiers, ParentElement, Pixels, Styled, TestAppContext, VisualTestContext, px,
};
use odk_a11y::{Describe, Role};

use super::stability::{Step, assert_still};
use super::{Count, open, press, redraw};
use crate::input::{point_of, probe};
use crate::{Button, Dialog, Input, InputGroup, Size, v_flex};

/// A part's name that is built, which a test may leak.
fn named(name: String) -> &'static str {
    Box::leak(name.into_boxed_str())
}

/// The id of the field inside the group with id `g`.
const FIELD: &str = "g/input";

fn text(cx: &mut VisualTestContext) -> String {
    probe(cx, FIELD).0
}

fn bounds(cx: &mut VisualTestContext, part: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("{part} was drawn"))
}

/// Items 8, 9: Tab reaches the field and then the button, and Enter does what it should in each.
#[gpui::test]
fn tab_goes_from_the_field_to_the_button(cx: &mut TestAppContext) {
    let (submitted, clicked) = (Rc::new(RefCell::new(Vec::new())), Count::default());
    let (s, c) = (submitted.clone(), clicked.clone());
    let cx = open(cx, move |_, _| {
        let (s, c) = (s.clone(), c.clone());
        InputGroup::new("g", "Website")
            .prefix("https://")
            .suffix(".com")
            .action(
                Button::new("copy")
                    .label("Copy")
                    .on_click(move |_, _, _| c.bump()),
            )
            .on_submit(move |text, _, _| s.borrow_mut().push(text.to_owned()))
    });
    press(cx, "tab");
    cx.simulate_input("example");
    press(cx, "enter");
    assert_eq!(*submitted.borrow(), ["example"]);
    assert_eq!(clicked.get(), 0);
    press(cx, "tab enter");
    assert_eq!(clicked.get(), 1, "Tab went on to the button");
    assert_eq!(text(cx), "example");
    press(cx, "shift-tab end");
    cx.simulate_input("!");
    assert_eq!(text(cx), "example!", "and Shift+Tab back to the field");
}

/// The group is exactly as high as an `Input` of its size, whatever the button and whatever the
/// application's density, and the field and the button sit inside its box (item 32).
#[gpui::test]
fn it_is_as_high_as_an_input_of_its_size(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Input::new("i-sm", "A").size(Size::Sm))
            .child(
                InputGroup::new("g-sm", "A")
                    .size(Size::Sm)
                    .action(Button::new("b-sm").label("Go")),
            )
            .child(Input::new("i-md", "B"))
            .child(
                InputGroup::new("g-md", "B")
                    .prefix("$")
                    .action(Button::new("b-md").label("Go")),
            )
            .child(Input::new("i-lg", "C").size(Size::Lg))
            .child(
                InputGroup::new("g-lg", "C")
                    .size(Size::Lg)
                    .suffix("kg")
                    .action(Button::new("b-lg").label("Go")),
            )
    });
    for density in odk_theme::foundation::Density::ALL {
        cx.update(|_, cx| crate::configure(cx, |c| c.density = density));
        redraw(cx);
        assert_as_high_as_an_input(cx, density.name());
    }
}

fn assert_as_high_as_an_input(cx: &mut VisualTestContext, density: &str) {
    for size in ["sm", "md", "lg"] {
        let input = bounds(cx, named(format!("i-{size}/box")));
        let group = bounds(cx, named(format!("g-{size}/box")));
        // Exactly: both are a field's box at the same density, the button inside it lower.
        assert_eq!(
            group.size.height, input.size.height,
            "{density} {size}: {group:?} against {input:?}"
        );
        let field = bounds(cx, named(format!("g-{size}/input/box")));
        let button = bounds(cx, named(format!("b-{size}/button")));
        assert!(
            field.left() >= group.left() && field.right() <= button.left(),
            "{density} {size}: the field is between the edge and the button: {field:?} {button:?}"
        );
        assert!(
            button.top() >= group.top() && button.bottom() <= group.bottom(),
            "{density} {size}: the button is inside the box: {button:?} in {group:?}"
        );
    }
}

/// A press on the text around the field puts focus in it.
#[gpui::test]
fn a_press_on_the_text_around_the_field_focuses_it(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        InputGroup::new("g", "Website")
            .prefix("https://")
            .suffix(".com")
    });
    for part in ["g/prefix", "g/suffix"] {
        let at = bounds(cx, part).center();
        cx.simulate_click(at, Modifiers::default());
        redraw(cx);
        cx.simulate_input("x");
        redraw(cx);
    }
    assert_eq!(text(cx), "xx");
}

/// Item 32: nothing moves when the field is hovered, pressed, focused or typed in, and the
/// button stays where it was.
#[gpui::test]
fn nothing_moves_when_it_is_used(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(260.0))
            .child(
                InputGroup::new("g", "Website")
                    .prefix("https://")
                    .action(Button::new("copy").label("Copy")),
            )
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &["g/box", "g/input/box", "copy/button", "after/button"],
        &[
            Step::Hover("g/box"),
            Step::Press("g/input/box"),
            Step::Keys("tab"),
            Step::Keys("a b c d e f g h i j k l m n o p q r s t u v w x y z"),
            Step::Keys("tab"),
            Step::Keys("shift-tab ctrl-a backspace"),
        ],
    );
}

/// Item 6: a disabled group is skipped by Tab, button included, and ignores clicks.
#[gpui::test]
fn a_disabled_group_is_skipped_and_disables_its_button(cx: &mut TestAppContext) {
    let (clicked, after) = (Count::default(), Count::default());
    let (c, a) = (clicked.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (c, a) = (c.clone(), a.clone());
        v_flex()
            .child(
                InputGroup::new("g", "Website")
                    .default_value("abc")
                    .prefix("https://")
                    .disabled(true)
                    .action(
                        Button::new("copy")
                            .label("Copy")
                            .on_click(move |_, _, _| c.bump()),
                    ),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab enter");
    assert_eq!((clicked.get(), after.get()), (0, 1), "Tab went past both");
    let at = bounds(cx, "g/prefix").center();
    cx.simulate_click(at, Modifiers::default());
    let in_field = point_of(cx, FIELD, 1);
    cx.simulate_click(in_field, Modifiers::default());
    cx.simulate_input("x");
    redraw(cx);
    assert_eq!(text(cx), "abc");
    let at = bounds(cx, "copy/button").center();
    cx.simulate_click(at, Modifiers::default());
    assert_eq!(clicked.get(), 0);
}

/// Items 14, 15, 16: what it reports.
#[gpui::test]
fn it_reports_a_text_input_with_the_groups_name(_: &mut TestAppContext) {
    let a = InputGroup::new("g", "Website")
        .default_value("abc")
        .accessible();
    assert_eq!(a.role, Role::TextInput);
    assert_eq!(a.name, "Website");
    assert_eq!(a.value.as_deref(), Some("abc"));
    assert!(
        InputGroup::new("g", "Website")
            .disabled(true)
            .accessible()
            .disabled
    );
    assert!(
        InputGroup::new("g", "Website")
            .read_only(true)
            .accessible()
            .read_only
    );
    assert_eq!(
        InputGroup::new("g", "Website")
            .masked(true)
            .default_value("x")
            .accessible()
            .value,
        None
    );
}

/// Item 23: addons far too long for a narrow box truncate, the field keeps some room and the
/// button stays inside the box.
#[gpui::test]
fn long_addons_truncate_and_the_button_stays_inside(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(240.0)).child(
            InputGroup::new("g", "Website")
                .prefix("https://wiki.omarchy.org/troubleshooting/backups/")
                .suffix("日本語のとても長いドメイン名.example.com 😀😀😀")
                .placeholder("Pneumonoultramicroscopicsilicovolcanoconiosis")
                .action(Button::new("copy").label("Copy")),
        )
    });
    let group = bounds(cx, "g/box");
    for part in ["g/prefix", "g/suffix", "g/input/box", "copy/button"] {
        let b = bounds(cx, part);
        assert!(
            b.left() >= group.left() && b.right() <= group.right(),
            "{part} is inside the box: {b:?} in {group:?}"
        );
    }
    assert!(
        bounds(cx, "g/input/box").size.width > px(40.0),
        "the field was not squeezed away"
    );
}

/// A controlled group follows its value.
#[gpui::test]
fn a_controlled_group_follows_its_value(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(String::from("abc")));
    let v = value.clone();
    let cx = open(cx, move |_, _| {
        let v = v.clone();
        let current = v.borrow().clone();
        InputGroup::new("g", "Website")
            .value(current)
            .on_change(move |text, _, _| *v.borrow_mut() = text.to_owned())
    });
    press(cx, "tab end");
    cx.simulate_input("d");
    redraw(cx);
    assert_eq!(
        (text(cx), value.borrow().clone()),
        ("abcd".into(), "abcd".into())
    );
    *value.borrow_mut() = "zzz".into();
    redraw(cx);
    assert_eq!(text(cx), "zzz");
}

/// Item 13 and ADR 0037: Escape is left to the overlay around a group.
#[gpui::test]
fn escape_reaches_the_dialog_around_the_group(cx: &mut TestAppContext) {
    let closed = Count::default();
    let c = closed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Dialog::new("d", "Share")
            .open(true)
            .content(|_, _| {
                InputGroup::new("g", "Website")
                    .default_value("abc")
                    .action(Button::new("copy").label("Copy"))
            })
            .on_close(move |_, _| c.bump())
    });
    press(cx, "tab tab");
    assert_eq!(probe(cx, FIELD).1, 0..3, "the field has focus");
    press(cx, "escape");
    assert_eq!(closed.get(), 1);
}
