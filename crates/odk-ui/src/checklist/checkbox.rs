use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};

use super::{open, press, redraw};
use crate::{Button, Checkbox, v_flex};

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

/// Items 8, 9, 10: Tab reaches the box, Space toggles it and Enter does not.
#[gpui::test]
fn space_toggles_and_enter_does_not(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Checkbox::new("c", "Wrap lines").on_change(record(&s))
    });
    press(cx, "tab space enter space");
    assert_eq!(*seen.borrow(), [true, false]);
}

/// A click on the box toggles it once; a click on the label toggles it once and moves focus
/// to the box, so Space then toggles it again.
#[gpui::test]
fn clicks_on_the_box_and_the_label_toggle_once(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Checkbox::new("c", "Wrap lines").on_change(record(&s))
    });
    click(cx, "c/box");
    assert_eq!(*seen.borrow(), [true]);
    click(cx, "c/label");
    assert_eq!(*seen.borrow(), [true, false]);
    press(cx, "space");
    assert_eq!(
        *seen.borrow(),
        [true, false, true],
        "the label click focused the box"
    );
}

/// Item 6: a disabled checkbox is skipped by Tab and ignores clicks.
#[gpui::test]
fn a_disabled_checkbox_is_skipped_and_ignores_clicks(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Rc::new(Cell::new(0)));
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(
                Checkbox::new("c", "Wrap lines")
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
    click(cx, "c/box");
    click(cx, "c/label");
    assert!(seen.borrow().is_empty());
    assert!(Checkbox::new("c", "x").disabled(true).accessible().disabled);
}

/// A mixed box checks on a press; uncontrolled, it keeps its own state.
#[gpui::test]
fn a_mixed_box_checks_on_a_press(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Checkbox::new("c", "All files")
            .checked(false)
            .indeterminate(true)
            .on_change(record(&s))
    });
    press(cx, "tab space");
    assert_eq!(*seen.borrow(), [true]);
}

/// Controlled, the box shows the value it is given and only reports presses; uncontrolled, it
/// follows them.
#[gpui::test]
fn controlled_and_uncontrolled(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(
                Checkbox::new("held", "Held")
                    .checked(false)
                    .on_change(record(&s)),
            )
            .child(Checkbox::new("free", "Free").on_change(record(&s)))
    });
    press(cx, "tab space space tab space space");
    assert_eq!(*seen.borrow(), [true, true, true, false]);
}

/// Items 14, 15, 16: a checkbox reports its label, whether it is checked, and a mixed state.
#[test]
fn reports_its_state() {
    let a = Checkbox::new("c", "Wrap lines").checked(true).accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.selected),
        (Role::Checkbox, "Wrap lines", Some(true))
    );
    let mixed = Checkbox::new("c", "All").indeterminate(true).accessible();
    assert!(mixed.mixed);
    assert_eq!(mixed.selected, Some(false));
}

/// Item 32: checking, focusing, hovering and pressing moves nothing.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    let off = Rc::new(Cell::new(false));
    let o = off.clone();
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(Checkbox::new("c", "Wrap lines").disabled(o.get()))
            .child(Button::new("after").label("After"))
    });
    let disable = off.clone();
    assert_still(
        cx,
        &["c/row", "c/box", "c/label", "after/button"],
        &[
            Step::Keys("tab"),
            Step::Keys("space"),
            Step::Hover("c/box"),
            Step::Press("c/box"),
            Step::Press("c/label"),
            Step::Keys("tab"),
            Step::Change("disabling it", Box::new(move || disable.set(true))),
        ],
    );
}

/// Item 23: a long label wraps in a narrow column; the box keeps its size and stays on the
/// label's first line, and everything stays inside the column.
#[gpui::test]
fn a_long_label_wraps_beside_the_box(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(200.0)).child(Checkbox::new(
            "c",
            "Automatischer Zeilenumbruch am Fensterrand, auch in eingebetteten Ansichten",
        ))
    });
    let (row, check, label) = (
        cx.debug_bounds("c/row").unwrap(),
        cx.debug_bounds("c/box").unwrap(),
        cx.debug_bounds("c/label").unwrap(),
    );
    // A square of the density's icon size.
    let side = px(crate::checkbox::drawn::last("c").geometry.size);
    assert_eq!(side, px(crate::foundation().controls.metrics().icon_size));
    assert_eq!(
        (check.size.width, check.size.height),
        (side, side),
        "the box keeps its size"
    );
    assert!(
        row.size.width <= px(200.0) && label.right() <= px(200.0),
        "it fits: {row:?}"
    );
    assert!(
        label.size.height > check.size.height * 2.0,
        "the label wrapped"
    );
    assert!(
        check.top() < label.top() + check.size.height,
        "the box is on the first line"
    );
}

/// A click on the label moves focus to the box, as for a native checkbox, so Space then
/// toggles it.
#[gpui::test]
fn a_click_on_the_label_focuses_the_box(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Checkbox::new("c", "Wrap lines").on_change(record(&s))
    });
    click(cx, "c/label");
    press(cx, "space");
    assert_eq!(*seen.borrow(), [true, false]);
}

/// The box is a square of its density's icon size, 14, 16 or 18, with the application's
/// corners; a wrong one keeps the danger edge under the pointer and in focus and has a `!` after
/// its label, and a disabled checked one keeps its mark (items 5, 18).
#[gpui::test]
fn each_density_has_its_square_and_a_wrong_box_its_edge(cx: &mut TestAppContext) {
    use odk_theme::foundation::Density;
    let cx = open(cx, |_, _| {
        v_flex()
            .children(Density::ALL.map(|d| Checkbox::new(d.name(), "Wrap").density(d)))
            .child(Checkbox::new("bad", "Agree").invalid(true))
            .child(Checkbox::new("off", "Managed").checked(true).disabled(true))
    });
    for d in Density::ALL {
        let b = cx
            .debug_bounds(Box::leak(format!("{}/box", d.name()).into_boxed_str()))
            .unwrap();
        let side = px(d.metrics().icon_size);
        assert_eq!((b.size.width, b.size.height), (side, side), "{d:?}");
    }
    press(cx, "tab tab tab tab");
    redraw(cx);
    let bad = crate::checkbox::drawn::last("bad");
    let s = crate::components().semantic().clone();
    assert!(bad.focus_visible);
    assert_eq!(bad.current.border, s.border.danger);
    assert_eq!(bad.colours.hover.border, s.border.danger);
    assert_eq!(bad.focus.colour, s.border.danger);
    assert_eq!(
        bad.invalid_mark,
        Some(s.roles.danger.subtle.default.foreground)
    );
    assert!(
        cx.debug_bounds("bad/invalid-mark").is_some(),
        "a wrong box has a mark, not only a colour"
    );
    let off = crate::checkbox::drawn::last("off");
    assert_eq!(off.check, odk_theme::component::Check::Checked);
    assert!(!off.responds && !off.focus_visible);
}
