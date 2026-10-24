//! `SearchInput`: an `Input` with a search icon, a clear button, a shortcut and Escape to clear.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};

use super::stability::{Step, assert_still};
use super::{Count, open, press, redraw};
use crate::input::probe;
use crate::{Button, Dialog, SearchInput, v_flex};

fn text(cx: &mut VisualTestContext) -> String {
    probe(cx, "s").0
}

fn part(cx: &mut VisualTestContext, name: &'static str) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(name)
}

/// What the field draws at its end now. GPUI keeps the bounds of parts drawn earlier, so only
/// this says that a part went away.
fn ends() -> Vec<&'static str> {
    crate::input::drawn::ends("s")
}

/// Escape empties a field with text, as one edit Ctrl+Z undoes; the clear button does the
/// same and keeps focus in the field (items 8, 10).
#[gpui::test]
fn escape_and_the_clear_button_empty_it(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        let s = s.clone();
        SearchInput::new("s", "Search settings")
            .on_change(move |t, _, _| s.borrow_mut().push(t.to_owned()))
    });
    press(cx, "tab");
    assert!(!ends().contains(&"clear"), "no clear button while empty");
    cx.simulate_input("wifi");
    redraw(cx);
    assert!(ends().contains(&"clear"));
    press(cx, "escape");
    assert_eq!(text(cx), "");
    redraw(cx);
    assert!(!ends().contains(&"clear"), "gone with the text");
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "wifi", "one edit, undone at once");

    let clear = part(cx, "s/clear/button").unwrap();
    cx.simulate_click(clear.center(), Modifiers::default());
    redraw(cx);
    assert_eq!(text(cx), "");
    cx.simulate_input("x");
    assert_eq!(text(cx), "x", "focus stayed in the field");
    assert_eq!(seen.borrow().last().unwrap(), "x");
    assert!(
        seen.borrow().contains(&String::new()),
        "clearing is reported"
    );
}

/// Item 13 and ADR 0037: Escape in an empty search field is left to the dialog around it;
/// with text, Escape clears it first and the dialog stays open.
#[gpui::test]
fn escape_in_an_empty_field_closes_the_dialog(cx: &mut TestAppContext) {
    let closed = Count::default();
    let c = closed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Dialog::new("d", "Find")
            .open(true)
            .content(|_, _| SearchInput::new("s", "Search").default_value("abc"))
            .on_close(move |_, _| c.bump())
    });
    press(cx, "tab tab");
    press(cx, "escape");
    assert_eq!((text(cx).as_str(), closed.get()), ("", 0), "cleared first");
    press(cx, "escape");
    assert_eq!(closed.get(), 1, "then the dialog closes");
}

/// The shortcut shows while the field is empty and not focused; Enter submits the search.
#[gpui::test]
fn the_shortcut_shows_while_empty_and_enter_submits(cx: &mut TestAppContext) {
    let submitted = Rc::new(RefCell::new(None::<String>));
    let s = submitted.clone();
    let cx = open(cx, move |_, _| {
        let s = s.clone();
        SearchInput::new("s", "Search")
            .shortcut("Ctrl+K")
            .on_submit(move |t, _, _| *s.borrow_mut() = Some(t.to_owned()))
    });
    assert_eq!(ends(), ["shortcut"]);
    press(cx, "tab");
    redraw(cx);
    assert_eq!(ends(), Vec::<&str>::new(), "hidden while focused");
    cx.simulate_input("dock");
    press(cx, "enter");
    assert_eq!(submitted.borrow().as_deref(), Some("dock"));
    assert!(part(cx, "s/leading-icon").is_some(), "the search icon");
}

/// Item 7: while loading it shows a spinner and reports itself busy, and stays editable.
#[gpui::test]
fn loading_shows_a_spinner_and_reports_busy(cx: &mut TestAppContext) {
    let loading = Rc::new(Cell::new(false));
    let l = loading.clone();
    let cx = open(cx, move |_, _| {
        SearchInput::new("s", "Search").loading(l.get())
    });
    let rest = part(cx, "s/box").unwrap();
    assert!(part(cx, "s/spinner").is_none());
    loading.set(true);
    redraw(cx);
    assert!(part(cx, "s/spinner").is_some());
    assert_eq!(part(cx, "s/box").unwrap(), rest, "the box keeps its size");
    press(cx, "tab");
    cx.simulate_input("a");
    assert_eq!(text(cx), "a");
    let a = SearchInput::new("s", "Search").loading(true).accessible();
    assert!(a.busy);
    assert_eq!(a.role, Role::TextInput);
}

/// Item 32: typing, which brings the clear button, and clearing move nothing around it.
#[gpui::test]
fn nothing_moves_when_it_is_used(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(260.0))
            .child(SearchInput::new("s", "Search").shortcut("Ctrl+K"))
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &["s/box", "after/button"],
        &[
            Step::Hover("s/box"),
            Step::Keys("tab"),
            Step::Keys("a b c"),
            Step::Hover("s/clear/button"),
            Step::Press("s/clear/button"),
            Step::Keys("x escape"),
        ],
    );
}
