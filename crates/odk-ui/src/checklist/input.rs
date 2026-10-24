//! `Input` (ADR 0038, `docs/spec/odk-text-input.md`) on GPUI's headless test platform.
//!
//! Typing goes through `simulate_input` and the key bindings through `press`, as a keyboard
//! would. The test platform has no way to feed an IME pre-edit, so those tests call the input
//! handler's methods on the field's entity, which is exactly what the platform calls.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    Bounds, ClipboardItem, EntityInputHandler, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, Pixels, Styled, TestAppContext, VisualTestContext, point, px,
};
use odk_a11y::{Describe, Role};

use super::stability::{Step, assert_still};
use super::{Count, open, press, redraw};
use crate::input::{entity_of, point_of, probe};
use crate::{Button, Dialog, Input, v_flex};

type Seen = Rc<RefCell<Vec<String>>>;

fn record(seen: &Seen) -> impl Fn(&str, &mut gpui::Window, &mut gpui::App) + 'static {
    let seen = seen.clone();
    move |text, _, _| seen.borrow_mut().push(text.to_owned())
}

fn text(cx: &mut VisualTestContext) -> String {
    probe(cx, "i").0
}

fn selection(cx: &mut VisualTestContext) -> Range<usize> {
    probe(cx, "i").1
}

fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
}

fn primary(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| cx.read_from_primary().and_then(|item| item.text()))
}

fn box_bounds(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds("i/box").expect("the box was drawn")
}

/// A click `count` times over, at `at`, which is how the platform numbers a double-click.
fn click_n(cx: &mut VisualTestContext, at: gpui::Point<Pixels>, count: usize) {
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: count,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: count,
    });
    redraw(cx);
}

/// Items 8, 9: Tab reaches the field and typing edits it; every edit is reported.
#[gpui::test]
fn tab_reaches_it_and_typing_edits_it(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Input::new("i", "Name").on_change(record(&s))
    });
    press(cx, "tab");
    cx.simulate_input("héllo");
    assert_eq!(text(cx), "héllo");
    assert_eq!(
        *seen.borrow(),
        ["h", "hé", "hél", "héll", "héllo"].map(String::from)
    );
}

/// Item 9: Tab leaves a field again, and types nothing into it on the way.
#[gpui::test]
fn tab_leaves_the_field_and_types_no_tab(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(Input::new("i", "Name").on_change(record(&s)))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab");
    cx.simulate_input("ab");
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab moved on to the button");
    assert_eq!(text(cx), "ab");
    assert_eq!(*seen.borrow(), ["a", "ab"].map(String::from));
}

/// Enter submits and types no line break, with a handler or without one.
#[gpui::test]
fn enter_submits_and_types_nothing(cx: &mut TestAppContext) {
    let (changes, submitted) = (Seen::default(), Seen::default());
    let (c, s) = (changes.clone(), submitted.clone());
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(
                Input::new("i", "Name")
                    .default_value("x")
                    .on_change(record(&c))
                    .on_submit(record(&s)),
            )
            .child(Input::new("bare", "Bare").default_value("y"))
    });
    press(cx, "tab enter");
    assert_eq!(*submitted.borrow(), ["x"]);
    assert_eq!(
        text(cx),
        "x",
        "the selected text was not replaced by a line break"
    );
    assert!(changes.borrow().is_empty());
    press(cx, "tab enter");
    assert_eq!(
        probe(cx, "bare").0,
        "y",
        "without a handler Enter still types nothing"
    );
}

/// Reaching the field with Tab selects its text; a click puts the caret where it landed, and
/// Tab back in selects all again.
#[gpui::test]
fn tab_selects_all_and_a_click_puts_the_caret(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Input::new("i", "Name").default_value("hello world"))
            .child(Button::new("after").label("After"))
    });
    let at = point_of(cx, "i", 5);
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    assert_eq!(
        selection(cx),
        5..5,
        "a click on an unfocused field puts the caret"
    );
    press(cx, "tab shift-tab");
    assert_eq!(selection(cx), 0..11, "Tab selects all");
}

/// Items 8, 10: the movement, selection and deletion keys of the spec.
#[gpui::test]
fn keys_move_select_and_delete(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Input::new("i", "Name").default_value("one two three")
    });
    press(cx, "tab");
    assert_eq!(selection(cx), 0..13);
    press(cx, "end");
    assert_eq!(selection(cx), 13..13);
    press(cx, "ctrl-left ctrl-left");
    assert_eq!(selection(cx), 4..4, "two words back");
    press(cx, "shift-right");
    assert_eq!(selection(cx), 4..5);
    press(cx, "ctrl-shift-right");
    assert_eq!(
        selection(cx),
        4..7,
        "the selection grew to the end of the word"
    );
    press(cx, "left");
    assert_eq!(
        selection(cx),
        4..4,
        "Left collapses to the start of a selection"
    );
    press(cx, "ctrl-delete");
    assert_eq!(text(cx), "one  three");
    press(cx, "backspace");
    assert_eq!(text(cx), "one three");
    press(cx, "home ctrl-end");
    assert_eq!(selection(cx), 9..9);
    press(cx, "shift-home");
    assert_eq!(selection(cx), 0..9);
    press(cx, "delete");
    assert_eq!(text(cx), "");
}

/// A grapheme is one step, whatever it is made of.
#[gpui::test]
fn backspace_removes_a_whole_grapheme(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Input::new("i", "Name").default_value("a👨‍👩‍👧"));
    press(cx, "tab end backspace");
    assert_eq!(text(cx), "a");
}

/// The system clipboard and the primary selection.
#[gpui::test]
fn the_clipboard_and_the_primary_selection(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Input::new("i", "Name").default_value("hello world")
    });
    press(cx, "tab");
    assert_eq!(
        primary(cx),
        None,
        "reaching it with Tab is not a selection by the user"
    );
    press(cx, "ctrl-c");
    assert_eq!(clipboard(cx).as_deref(), Some("hello world"));
    press(cx, "home right shift-right shift-right");
    assert_eq!(
        primary(cx).as_deref(),
        Some("el"),
        "selecting writes the primary selection"
    );
    press(cx, "ctrl-x");
    assert_eq!(clipboard(cx).as_deref(), Some("el"));
    assert_eq!(text(cx), "hlo world");
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("A\nB\tC".into())));
    press(cx, "ctrl-v");
    assert_eq!(
        text(cx),
        "hA B Clo world",
        "line breaks and tabs became spaces"
    );
}

/// A masked field gives away neither its text nor its words.
#[gpui::test]
fn a_masked_field_keeps_its_text_to_itself(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Input::new("i", "Password")
            .default_value("secret word")
            .masked(true)
    });
    press(cx, "tab ctrl-c");
    assert_eq!(clipboard(cx), None);
    assert_eq!(primary(cx), None);
    press(cx, "ctrl-x");
    assert_eq!(
        text(cx),
        "secret word",
        "Cut does not remove what it cannot copy"
    );
    press(cx, "right shift-left shift-left");
    assert_eq!(
        primary(cx),
        None,
        "selecting does not write the primary selection"
    );
    press(cx, "home ctrl-right");
    assert_eq!(
        selection(cx),
        11..11,
        "Ctrl+Right goes to the end, not to a word"
    );
    let e = entity_of("i");
    let given =
        cx.update(|window, cx| e.update(cx, |s, cx| s.text_for_range(0..3, &mut None, window, cx)));
    assert_eq!(given, None, "the input method is told nothing of the text");
    assert_eq!(
        Input::new("i", "Password")
            .default_value("secret")
            .masked(true)
            .accessible()
            .value,
        None
    );
}

/// A read-only field can be selected and copied, not changed.
#[gpui::test]
fn a_read_only_field_selects_and_copies_but_does_not_change(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Input::new("i", "Name")
            .default_value("hello")
            .read_only(true)
    });
    press(cx, "tab");
    cx.simulate_input("x");
    press(cx, "backspace ctrl-z");
    assert_eq!(text(cx), "hello");
    press(cx, "ctrl-c");
    assert_eq!(clipboard(cx).as_deref(), Some("hello"));
    assert!(
        Input::new("i", "Name")
            .read_only(true)
            .accessible()
            .read_only
    );
}

/// Undo and redo, one run of typing at a time.
#[gpui::test]
fn undo_and_redo_step_through_runs_of_typing(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Input::new("i", "Name"));
    press(cx, "tab");
    cx.simulate_input("hello world");
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "hello ");
    press(cx, "ctrl-shift-z");
    assert_eq!(text(cx), "hello world");
    press(cx, "ctrl-z ctrl-z");
    assert_eq!(text(cx), "");
    press(cx, "ctrl-y");
    assert_eq!(text(cx), "hello ");
}

/// Double-click selects a word, triple-click everything.
#[gpui::test]
fn a_double_click_selects_a_word_and_a_triple_click_all(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Input::new("i", "Name").default_value("one two"));
    let at = point_of(cx, "i", 5);
    click_n(cx, at, 2);
    assert_eq!(selection(cx), 4..7);
    assert_eq!(primary(cx).as_deref(), Some("two"));
    click_n(cx, at, 3);
    assert_eq!(selection(cx), 0..7);
}

/// A drag selects on after the pointer has left the field, in both directions, and stops when
/// the button comes up (ADR 0038).
#[gpui::test]
fn a_drag_selects_on_past_the_edges_of_the_field(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(300.0))
            .child(Input::new("i", "Name").default_value("hello world"))
    });
    let (field, y) = (box_bounds(cx), point_of(cx, "i", 0).y);
    let left = Modifiers::default();
    let start = point_of(cx, "i", 0);
    cx.simulate_mouse_down(start, MouseButton::Left, left);
    cx.simulate_mouse_move(
        point(field.right() + px(80.0), y + px(60.0)),
        MouseButton::Left,
        left,
    );
    redraw(cx);
    assert_eq!(
        selection(cx),
        0..11,
        "the pointer is outside the field and past its end"
    );
    cx.simulate_mouse_up(point(field.right() + px(80.0), y), MouseButton::Left, left);
    cx.simulate_mouse_move(point(field.left() - px(40.0), y), None, left);
    redraw(cx);
    assert_eq!(
        selection(cx),
        0..11,
        "nothing selects once the button is up"
    );

    let at = point_of(cx, "i", 8);
    cx.simulate_mouse_down(at, MouseButton::Left, left);
    cx.simulate_mouse_move(point(field.left() - px(60.0), y), MouseButton::Left, left);
    cx.simulate_mouse_up(at, MouseButton::Left, left);
    redraw(cx);
    assert_eq!(selection(cx), 0..8, "dragging back past the start");
    assert_eq!(primary(cx).as_deref(), Some("hello wo"));
}

/// A middle click pastes the primary selection where it lands.
#[gpui::test]
fn a_middle_click_pastes_the_primary_selection(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Input::new("i", "Name").default_value("abcd"));
    cx.update(|_, cx| cx.write_to_primary(ClipboardItem::new_string("XY".into())));
    let at = point_of(cx, "i", 2);
    cx.simulate_mouse_down(at, MouseButton::Middle, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Middle, Modifiers::default());
    redraw(cx);
    assert_eq!(text(cx), "abXYcd");
}

/// A controlled field shows its value: an edit the application does not take is undone, and
/// a new value from outside replaces the text and forgets the history.
#[gpui::test]
fn a_controlled_field_follows_its_value(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(String::from("abc")));
    let accept = Rc::new(Cell::new(false));
    let (v, a) = (value.clone(), accept.clone());
    let cx = open(cx, move |_, _| {
        let (v, a) = (v.clone(), a.clone());
        let value = v.borrow().clone();
        Input::new("i", "Name")
            .value(value)
            .on_change(move |text, _, _| {
                if a.get() {
                    *v.borrow_mut() = text.to_owned();
                }
            })
    });
    press(cx, "tab end");
    cx.simulate_input("x");
    redraw(cx);
    assert_eq!(text(cx), "abc", "the application kept its value");
    accept.set(true);
    press(cx, "end");
    cx.simulate_input("x");
    redraw(cx);
    assert_eq!(
        (text(cx), value.borrow().clone()),
        ("abcx".into(), "abcx".into())
    );
    *value.borrow_mut() = "zzz".into();
    redraw(cx);
    assert_eq!(text(cx), "zzz");
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "zzz", "a value from outside has no history");
}

/// A composition is not reported until it ends, and then is one step of the history.
#[gpui::test]
fn an_ime_composition_is_reported_when_it_ends(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        Input::new("i", "Name").on_change(record(&s))
    });
    press(cx, "tab");
    let e = entity_of("i");
    cx.update(|window, cx| {
        e.update(cx, |s, cx| {
            s.replace_and_mark_text_in_range(None, "に", Some(1..1), window, cx)
        })
    });
    redraw(cx);
    assert_eq!(probe(cx, "i"), ("に".into(), 3..3, Some(0..3)));
    cx.update(|window, cx| {
        e.update(cx, |s, cx| {
            s.replace_and_mark_text_in_range(None, "にほ", Some(2..2), window, cx)
        })
    });
    assert!(
        seen.borrow().is_empty(),
        "nothing is reported mid-composition"
    );
    let marked = cx.update(|window, cx| e.update(cx, |s, cx| s.marked_text_range(window, cx)));
    assert_eq!(marked, Some(0..2), "the IME is told in UTF-16");
    cx.update(|window, cx| {
        e.update(cx, |s, cx| {
            s.replace_text_in_range(None, "日本", window, cx)
        })
    });
    redraw(cx);
    assert_eq!(probe(cx, "i"), ("日本".into(), 6..6, None));
    assert_eq!(*seen.borrow(), ["日本"]);
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "", "the whole composition was one step");
}

/// Leaving the field takes a half-made composition as it stands, and reports it.
#[gpui::test]
fn leaving_the_field_commits_a_composition(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(Input::new("i", "Name").on_change(record(&s)))
            .child(Button::new("after").label("After"))
    });
    press(cx, "tab");
    let e = entity_of("i");
    cx.update(|window, cx| {
        e.update(cx, |s, cx| {
            s.replace_and_mark_text_in_range(None, "に", None, window, cx)
        })
    });
    press(cx, "tab");
    assert_eq!(probe(cx, "i"), ("に".into(), 3..3, None));
    assert_eq!(*seen.borrow(), ["に"]);
}

/// What the platform asks of the field: where text is, for placing the candidate window, and
/// which character is under a point.
#[gpui::test]
fn the_input_method_can_find_text_on_screen(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Input::new("i", "Name").default_value("ab日"));
    press(cx, "tab");
    let (e, field) = (entity_of("i"), box_bounds(cx));
    let range =
        cx.update(|window, cx| e.update(cx, |s, cx| s.selected_text_range(false, window, cx)));
    assert_eq!(range.map(|r| (r.range, r.reversed)), Some((0..3, false)));
    let bounds = cx
        .update(|window, cx| e.update(cx, |s, cx| s.bounds_for_range(2..3, field, window, cx)))
        .expect("the field has been drawn");
    assert!(bounds.size.width > px(0.0) && bounds.size.height > px(0.0));
    let on_line = point_of(cx, "i", 2);
    let at = point(on_line.x + px(1.0), on_line.y);
    let index =
        cx.update(|window, cx| e.update(cx, |s, cx| s.character_index_for_point(at, window, cx)));
    assert_eq!(index, Some(2), "UTF-16 index of the Japanese character");
    let far = point(px(-50.0), px(-50.0));
    let index =
        cx.update(|window, cx| e.update(cx, |s, cx| s.character_index_for_point(far, window, cx)));
    assert_eq!(index, None, "a point outside the field is not in the text");
}

/// Item 6: a disabled field is skipped by Tab, ignores clicks and says it is disabled.
#[gpui::test]
fn a_disabled_field_is_skipped_and_ignores_clicks(cx: &mut TestAppContext) {
    let after = Count::default();
    let a = after.clone();
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(Input::new("i", "Name").default_value("abc").disabled(true))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab went past it");
    let at = point_of(cx, "i", 1);
    cx.simulate_click(at, Modifiers::default());
    cx.simulate_input("x");
    redraw(cx);
    assert_eq!(text(cx), "abc");
    assert!(Input::new("i", "Name").disabled(true).accessible().disabled);
}

/// Items 14, 15, 16: what it reports.
#[gpui::test]
fn it_describes_itself_as_a_text_input(_: &mut TestAppContext) {
    let a = Input::new("i", "Name").default_value("abc").accessible();
    assert_eq!(a.role, Role::TextInput);
    assert_eq!(a.name, "Name");
    assert_eq!(a.value.as_deref(), Some("abc"));
    assert!(!a.multiline && !a.read_only && !a.disabled);
    let a = Input::new("i", "Name")
        .value("controlled")
        .default_value("abc")
        .accessible();
    assert_eq!(a.value.as_deref(), Some("controlled"));
}

/// The placeholder shows while the field is empty and truncates in a narrow one. (That it is
/// gone once there is text is not tested here: GPUI never clears the bounds it keeps for
/// tests, so a part that stops being drawn still has some.)
#[gpui::test]
fn the_placeholder_shows_while_empty_and_never_leaves_the_box(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(160.0)).child(
            Input::new("i", "Name")
                .placeholder("Anzeigename für externe Bildschirme und Projektoren"),
        )
    });
    let (field, placeholder) = (box_bounds(cx), cx.debug_bounds("i/placeholder"));
    let placeholder = placeholder.expect("the placeholder is drawn while empty");
    assert!(
        placeholder.left() >= field.left() && placeholder.right() <= field.right(),
        "it truncates inside the box: {placeholder:?} in {field:?}"
    );
}

/// Item 23: a text far longer than the box scrolls inside it, and the caret stays in view.
#[gpui::test]
fn long_text_scrolls_inside_a_box_that_stays_put(cx: &mut TestAppContext) {
    let long =
        "Pneumonoultramicroscopicsilicovolcanoconiosis 日本語のとても長い文章です 😀 ".repeat(3);
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(200.0))
            .child(Input::new("i", "Name").default_value(long.clone()))
    });
    let before = box_bounds(cx);
    let field_right = before.right();
    press(cx, "tab end");
    redraw(cx);
    assert_eq!(box_bounds(cx), before, "the box did not grow");
    let end = text(cx).len();
    let caret = point_of(cx, "i", end);
    assert!(
        caret.x <= field_right && caret.x >= before.left(),
        "the caret at the end is in view: {caret:?}"
    );
    press(cx, "home");
    redraw(cx);
    let caret = point_of(cx, "i", 0);
    assert!(
        caret.x >= before.left() && caret.x <= field_right,
        "so is the caret at the start: {caret:?}"
    );
}

/// A field that does not have focus shows the start of its text, however long it is.
#[gpui::test]
fn a_field_without_focus_shows_the_start_of_its_text(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(120.0))
            .child(Input::new("i", "Name").default_value("a very long text that does not fit"))
            .child(Button::new("after").label("After"))
    });
    let field = box_bounds(cx);
    let start = point_of(cx, "i", 0);
    assert!(start.x >= field.left(), "the start is in view: {start:?}");
    press(cx, "tab end");
    redraw(cx);
    assert!(
        point_of(cx, "i", 0).x < field.left(),
        "with focus it follows the caret"
    );
    press(cx, "tab");
    redraw(cx);
    assert!(
        point_of(cx, "i", 0).x >= field.left(),
        "and goes back when focus leaves"
    );
}

/// Item 32: nothing moves when the field is hovered, pressed, focused or typed in, however
/// much is typed.
#[gpui::test]
fn nothing_moves_when_it_is_used(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(160.0))
            .child(Input::new("i", "Name").placeholder("Name"))
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &["i/box", "after/button"],
        &[
            Step::Hover("i/box"),
            Step::Press("i/box"),
            Step::Keys("tab"),
            Step::Keys("a b c d e f g h i j k l m n o p q r s t u v w x y z"),
            Step::Keys("ctrl-a backspace"),
        ],
    );
}

/// Item 13 and ADR 0037: Escape is left to the overlay around a field, so a field in a
/// dialog still lets the dialog close.
#[gpui::test]
fn escape_reaches_the_dialog_around_the_field(cx: &mut TestAppContext) {
    let closed = Count::default();
    let c = closed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Dialog::new("d", "Rename file")
            .open(true)
            .content(|_, _| Input::new("i", "Name").default_value("abc"))
            .on_close(move |_, _| c.bump())
    });
    press(cx, "tab tab");
    assert_eq!(selection(cx), 0..3, "the field has focus");
    press(cx, "escape");
    assert_eq!(closed.get(), 1);
    assert_eq!(text(cx), "abc");
}
