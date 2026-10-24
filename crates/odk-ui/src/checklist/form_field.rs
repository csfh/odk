//! `FormField`: a control's label, description and message, and how a form spaces its fields.

use gpui::{Modifiers, ParentElement, TestAppContext, VisualTestContext, px};
use odk_a11y::Describe;

use super::{FIXTURES, open, press, publish_fixture, redraw};
use crate::input::{drawn, probe};
use crate::{Checkbox, FormField, Input, Select, form};

fn bounds(cx: &mut VisualTestContext, part: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("no part named {part}"))
}

/// A press on the label focuses a text field or a select, as an HTML label does; Tab stops on
/// the control, never on the label (items 8, 9).
#[gpui::test]
fn a_press_on_the_label_focuses_the_control(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        form()
            .child(FormField::new("name", "Name").child(Input::new("i", "Name")))
            .child(
                FormField::new("country", "Country").child(
                    Select::new("s", "Country")
                        .item("dk", "Denmark")
                        .item("se", "Sweden"),
                ),
            )
    });
    let at = bounds(cx, "name/label").center();
    cx.simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(at, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    cx.simulate_input("Ada");
    assert_eq!(probe(cx, "i").0, "Ada");
    press(cx, "tab");
    redraw(cx);
    assert!(
        drawn::last("s/trigger").focus_visible,
        "the next stop is the select, not a label"
    );
    let at = bounds(cx, "country/label").center();
    press(cx, "shift-tab");
    cx.simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(at, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert!(drawn::last("s/trigger").focus_visible);
}

/// An error marks the control invalid and says why, with the danger mark, so it never rests on
/// colour, also in a monochrome theme (items 16, 18).
#[gpui::test]
fn an_error_marks_the_control_and_says_why(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        FormField::new("email", "Email")
            .required(true)
            .error("Enter an address with an @")
            .child(Input::new("i", "Email").default_value("ada"))
    });
    let [(_, vantablack)] = [FIXTURES[3]];
    publish_fixture(vantablack);
    redraw(cx);
    let s = drawn::last("i");
    let semantic = crate::components().semantic().clone();
    assert_eq!(s.current.border, semantic.border.danger);
    assert!(s.invalid_mark.is_some(), "the field has its mark");
    assert!(cx.debug_bounds("i/invalid-mark").is_some());
    assert!(
        cx.debug_bounds("email/message-mark").is_some(),
        "and the message its mark"
    );
    assert!(cx.debug_bounds("email/required").is_some());
    press(cx, "tab");
    redraw(cx);
    let focused = drawn::last("i");
    assert!(focused.focus_visible);
    assert_eq!(
        (focused.current.border, focused.focus.colour),
        (semantic.border.danger, semantic.border.danger),
        "focus keeps the error"
    );
}

/// The field reports what the control reports, with its state: invalid, required, and the
/// description and the message as the description.
#[test]
fn it_reports_the_control_with_the_field_s_state() {
    let a = FormField::new("pw", "Password")
        .description("At least 12 characters.")
        .error("Too short")
        .required(true)
        .child(Input::new("i", "Password"))
        .accessible();
    assert!(a.invalid && a.required);
    assert_eq!(
        a.description.as_deref(),
        Some("At least 12 characters. Too short")
    );
    let helped = FormField::new("n", "Nickname")
        .help("Shown to others")
        .child(Input::new("i", "Nickname"))
        .accessible();
    assert!(!helped.invalid, "help is not an error");
    assert_eq!(helped.description.as_deref(), Some("Shown to others"));
    let joined = FormField::new("n", "Name")
        .description("As on your passport")
        .error("Required")
        .child(Input::new("i", "Name"))
        .accessible();
    assert_eq!(
        joined.description.as_deref(),
        Some("As on your passport. Required")
    );
}

/// The label, description, control and message are `field_gap` (6 px) apart, and a form's
/// fields `form_gap` (16 px) apart.
#[gpui::test]
fn its_parts_and_a_form_s_fields_are_spaced_by_the_foundation(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        form()
            .child(
                FormField::new("a", "Name")
                    .description("As on your passport")
                    .help("We never share it")
                    .child(Input::new("ia", "Name")),
            )
            .child(
                FormField::new("b", "Nickname")
                    .optional(true)
                    .child(Input::new("ib", "Nickname")),
            )
    });
    let layout = crate::foundation().layout.clone();
    let gap = px(layout.field_gap);
    let (label, description, control, message) = (
        bounds(cx, "a/label"),
        bounds(cx, "a/description"),
        bounds(cx, "ia/box"),
        bounds(cx, "a/message"),
    );
    assert_eq!(description.top() - label.bottom(), gap);
    assert_eq!(control.top() - description.bottom(), gap);
    assert_eq!(message.top() - control.bottom(), gap);
    assert_eq!(
        bounds(cx, "b/field").top() - bounds(cx, "a/field").bottom(),
        px(layout.form_gap)
    );
    assert!(cx.debug_bounds("b/optional").is_some());
    assert!(cx.debug_bounds("b/required").is_none());
}

/// A disabled field draws its label and description disabled and disables its control (item 6).
#[gpui::test]
fn a_disabled_field_disables_its_control(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        FormField::new("f", "Owner")
            .disabled(true)
            .child(Input::new("i", "Owner").default_value("ada"))
    });
    press(cx, "tab");
    cx.simulate_input("x");
    assert_eq!(probe(cx, "i").0, "ada");
    assert!(!drawn::last("i").responds);
    let a = FormField::new("f", "Agree")
        .disabled(true)
        .child(Checkbox::new("c", "I agree"))
        .accessible();
    assert!(a.disabled);
}
