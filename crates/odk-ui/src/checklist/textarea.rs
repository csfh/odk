//! `Textarea` (ADR 0038, `docs/spec/odk-text-input.md`) on GPUI's headless test platform. It
//! shares its state and element with `Input`, so the behaviour they have in common is tested
//! in `input.rs`; this file tests what rows add. The test platform's font is monospaced, six
//! tenths of an em to a character, and two of those to anything outside the BMP.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    Bounds, ClipboardItem, InteractiveElement, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, Pixels, ScrollDelta, ScrollWheelEvent, Styled, TestAppContext,
    VisualTestContext, point, px,
};
use odk_a11y::{Describe, Role};

use super::stability::{Step, assert_still};
use super::{Count, open, press, redraw};
use crate::input::{layout_of, point_of, probe};
use crate::{Button, Dialog, Textarea, v_flex};

type Seen = Rc<RefCell<Vec<String>>>;

fn text(cx: &mut VisualTestContext) -> String {
    probe(cx, "t").0
}

fn selection(cx: &mut VisualTestContext) -> Range<usize> {
    probe(cx, "t").1
}

fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
}

fn primary(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| cx.read_from_primary().and_then(|item| item.text()))
}

fn box_bounds(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds("t/box").expect("the box was drawn")
}

/// How far down the screen the row holding `offset` is.
fn row_y(cx: &mut VisualTestContext, offset: usize) -> Pixels {
    point_of(cx, "t", offset).y
}

fn scrolled(cx: &mut VisualTestContext) -> Pixels {
    layout_of(cx, "t", |_, _, scroll| scroll.y)
}

fn wheel(cx: &mut VisualTestContext, over: gpui::Point<Pixels>, lines: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: over,
        delta: ScrollDelta::Lines(point(0.0, lines)),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    redraw(cx);
}

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

/// Ten lines of text, one to a row in a wide box.
const LINES: &str =
    "line 0\nline 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9";

/// Enter ends a line instead of submitting, and every edit is reported with the line break.
#[gpui::test]
fn enter_ends_a_line(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        let s = s.clone();
        Textarea::new("t", "Notes").on_change(move |text, _, _| s.borrow_mut().push(text.into()))
    });
    press(cx, "tab");
    cx.simulate_input("ab");
    press(cx, "enter");
    cx.simulate_input("c");
    assert_eq!(text(cx), "ab\nc");
    assert_eq!(
        *seen.borrow(),
        ["a", "ab", "ab\n", "ab\nc"].map(String::from)
    );
}

/// The line break ends a run of typing for the history, as a space does.
#[gpui::test]
fn a_line_break_ends_a_run_of_typing_for_undo(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Textarea::new("t", "Notes"));
    press(cx, "tab");
    cx.simulate_input("ab");
    press(cx, "enter");
    cx.simulate_input("cd");
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "ab\n");
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "");
}

/// Item 9: Tab reaches the field and leaves it again, typing no tab on the way.
#[gpui::test]
fn tab_leaves_the_field_and_types_no_tab(cx: &mut TestAppContext) {
    let after = Count::default();
    let a = after.clone();
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex().child(Textarea::new("t", "Notes")).child(
            Button::new("after")
                .label("After")
                .on_click(move |_, _, _| a.bump()),
        )
    });
    press(cx, "tab");
    cx.simulate_input("ab");
    press(cx, "tab enter");
    assert_eq!(after.get(), 1);
    assert_eq!(text(cx), "ab");
}

/// Up and Down move between lines and keep their column across a short one.
#[gpui::test]
fn up_and_down_keep_their_column_across_a_short_line(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").default_value("abcdefghij\nab\nabcdefghij")
    });
    press(cx, "tab ctrl-home");
    press(cx, "right right right right right right right right");
    assert_eq!(selection(cx), 8..8);
    press(cx, "down");
    assert_eq!(selection(cx), 13..13, "the short line ends before column 8");
    press(cx, "down");
    assert_eq!(
        selection(cx),
        22..22,
        "back at column 8, which it remembered"
    );
    press(cx, "up up");
    assert_eq!(selection(cx), 8..8);
    press(cx, "left up");
    assert_eq!(
        selection(cx),
        0..0,
        "above the first row is the start of the text"
    );
    press(cx, "ctrl-end down");
    assert_eq!(selection(cx), 24..24, "below the last row is the end");
}

/// A new column is remembered once the caret has moved sideways or the text has changed.
#[gpui::test]
fn the_remembered_column_is_forgotten_when_the_caret_moves_otherwise(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").default_value("abcdef\nab\nabcdef")
    });
    press(cx, "tab ctrl-home right right right right down");
    assert_eq!(selection(cx), 9..9, "the end of the short line");
    press(cx, "left down");
    assert_eq!(selection(cx), 11..11, "column 1, not the old column 4");
}

/// Shift with Up and Down selects over rows, and the selection reaches the primary selection.
#[gpui::test]
fn shift_up_and_down_select_rows(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").default_value("one\ntwo\nthree")
    });
    press(cx, "tab ctrl-home shift-down shift-down");
    assert_eq!(selection(cx), 0..8);
    assert_eq!(primary(cx).as_deref(), Some("one\ntwo\n"));
    press(cx, "shift-up");
    assert_eq!(selection(cx), 0..4);
}

/// Text too long for a row wraps, and Up and Down follow the rows it made.
#[gpui::test]
fn long_text_wraps_and_the_arrows_follow_the_rows(cx: &mut TestAppContext) {
    let words = "alpha beta gamma delta epsilon zeta eta theta";
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(140.0))
            .child(Textarea::new("t", "Notes").rows(6).default_value(words))
    });
    let (before, end) = (box_bounds(cx), words.len());
    assert!(
        row_y(cx, end) > row_y(cx, 0),
        "the text runs over several rows"
    );
    let field = box_bounds(cx);
    for offset in 0..=end {
        let at = point_of(cx, "t", offset);
        assert!(
            at.x >= field.left() && at.x <= field.right(),
            "offset {offset} is inside the box: {at:?} in {field:?}"
        );
    }
    press(cx, "tab ctrl-home down");
    let first = selection(cx).start;
    assert!(
        first > 0 && first < end,
        "Down went to the next row, not the next line"
    );
    assert!(row_y(cx, first) > row_y(cx, 0));
    press(cx, "ctrl-end");
    redraw(cx);
    assert_eq!(box_bounds(cx), before, "the box did not grow");
}

/// A word too long for any row is broken inside, and its characters all stay in the box.
#[gpui::test]
fn a_word_that_does_not_fit_breaks_inside(cx: &mut TestAppContext) {
    let long = "Pneumonoultramicroscopicsilicovolcanoconiosis";
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(120.0))
            .child(Textarea::new("t", "Notes").rows(8).default_value(long))
    });
    let field = box_bounds(cx);
    assert!(row_y(cx, long.len()) > row_y(cx, 0));
    for offset in 0..=long.len() {
        let at = point_of(cx, "t", offset);
        assert!(
            at.x >= field.left() && at.x <= field.right(),
            "offset {offset} is outside the box: {at:?} in {field:?}"
        );
    }
}

/// The caret at the start of a wrapped row is drawn on that row, and a click there lands on it.
#[gpui::test]
fn a_wrap_belongs_to_the_row_after_it(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(100.0)).child(
            Textarea::new("t", "Notes")
                .rows(6)
                .default_value("aaaa bbbb cccc dddd"),
        )
    });
    let start = layout_of(cx, "t", |layout, _, _| {
        (0..=19)
            .find(|&offset| layout.point_of(offset).y > px(0.0))
            .expect("the text wraps")
    });
    assert!(start > 0);
    let at = point_of(cx, "t", start);
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    assert_eq!(selection(cx), start..start);
    assert_eq!(
        layout_of(cx, "t", |layout, _, _| layout
            .offset_at(point(px(0.0), px(1.0)))),
        0
    );
}

/// PageDown and PageUp move by what the box shows, less a row, and the view follows.
#[gpui::test]
fn page_keys_move_by_the_height_of_the_box(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").rows(4).default_value(LINES)
    });
    press(cx, "tab ctrl-home pagedown");
    assert_eq!(
        selection(cx),
        21..21,
        "three rows down: the start of line 3"
    );
    press(cx, "pagedown pagedown");
    assert_eq!(selection(cx), 63..63, "the last line");
    press(cx, "pagedown");
    assert_eq!(selection(cx), LINES.len()..LINES.len(), "and then the end");
    press(cx, "pageup");
    assert_eq!(selection(cx), 42..42, "back at column 0, where it began");
    press(cx, "shift-pageup");
    assert_eq!(selection(cx), 21..42);
}

/// Home and End go to the ends of the line, Ctrl+Home and Ctrl+End to the ends of the text.
#[gpui::test]
fn home_and_end_go_to_the_ends_of_the_line(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").default_value("one\ntwo three\nfour")
    });
    press(cx, "tab ctrl-home down end");
    assert_eq!(selection(cx), 13..13);
    press(cx, "home");
    assert_eq!(selection(cx), 4..4);
    press(cx, "ctrl-end");
    assert_eq!(selection(cx), 18..18);
    press(cx, "ctrl-shift-home");
    assert_eq!(selection(cx), 0..18);
}

/// A paste keeps its line breaks, with any style of them, and its tabs.
#[gpui::test]
fn a_paste_keeps_its_lines(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Textarea::new("t", "Notes"));
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("a\r\nb\rc\td".into())));
    press(cx, "tab ctrl-v");
    assert_eq!(text(cx), "a\nb\nc\td");
}

/// A triple-click selects the line, a double-click a word; a click on a later row lands there.
#[gpui::test]
fn clicks_select_words_and_lines(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").default_value("one two\nthree four\nfive")
    });
    let at = point_of(cx, "t", 10);
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    assert_eq!(selection(cx), 10..10, "a click on the second line");
    click_n(cx, at, 2);
    assert_eq!(selection(cx), 8..13);
    click_n(cx, at, 3);
    assert_eq!(selection(cx), 8..18, "the line, without its break");
    assert_eq!(primary(cx).as_deref(), Some("three four"));
}

/// A drag selects over rows, and keeps going when the pointer is above or below the box.
#[gpui::test]
fn a_drag_selects_over_rows_and_past_the_box(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").rows(3).default_value(LINES)
    });
    let field = box_bounds(cx);
    let left = Modifiers::default();
    let start = point_of(cx, "t", 3);
    cx.simulate_mouse_down(start, MouseButton::Left, left);
    cx.simulate_mouse_move(
        point(field.right() + px(80.0), field.bottom() + px(2000.0)),
        MouseButton::Left,
        left,
    );
    redraw(cx);
    assert_eq!(
        selection(cx),
        3..LINES.len(),
        "below and beyond the last row is the end"
    );
    cx.simulate_mouse_move(
        point(field.left() + px(5.0), field.top() - px(2000.0)),
        MouseButton::Left,
        left,
    );
    cx.simulate_mouse_up(
        point(field.left(), field.top() - px(2000.0)),
        MouseButton::Left,
        left,
    );
    redraw(cx);
    assert_eq!(selection(cx), 0..3, "above the box is the start");
}

/// The wheel scrolls the field and the view stays where it was put; typing brings the caret
/// back into view.
#[gpui::test]
fn the_wheel_scrolls_and_typing_brings_the_caret_back(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").rows(3).default_value(LINES)
    });
    let over = point_of(cx, "t", 0);
    press(cx, "tab ctrl-home");
    assert_eq!(scrolled(cx), px(0.0));
    wheel(cx, over, -2.0);
    let moved = scrolled(cx);
    assert!(moved > px(0.0), "it scrolled down: {moved:?}");
    redraw(cx);
    assert_eq!(
        scrolled(cx),
        moved,
        "painting did not pull it back to the caret"
    );
    wheel(cx, over, 100.0);
    assert_eq!(scrolled(cx), px(0.0), "it stops at the top");
    wheel(cx, over, -100.0);
    let end = scrolled(cx);
    wheel(cx, over, -1.0);
    assert_eq!(scrolled(cx), end, "and at the end");
    cx.simulate_input("x");
    redraw(cx);
    assert_eq!(scrolled(cx), px(0.0), "typing shows the caret again");
}

/// A field that has scrolled as far as it can leaves the wheel to what is around it; one that
/// still scrolls keeps it.
#[gpui::test]
fn a_field_at_its_end_leaves_the_wheel_to_what_is_around_it(cx: &mut TestAppContext) {
    let around = Count::default();
    let a = around.clone();
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .on_scroll_wheel(move |_, _, _| a.bump())
            .child(
                Textarea::new("short", "Short")
                    .rows(3)
                    .default_value("short"),
            )
            .child(Textarea::new("t", "Notes").rows(3).default_value(LINES))
    });
    let short = point_of(cx, "short", 0);
    wheel(cx, short, -3.0);
    assert_eq!(
        around.get(),
        1,
        "a field with nothing to scroll passes it on"
    );
    let over = point_of(cx, "t", 0);
    wheel(cx, over, -2.0);
    assert!(scrolled(cx) > px(0.0));
    assert_eq!(around.get(), 1, "a field that scrolled kept it");
    wheel(cx, over, -100.0);
    wheel(cx, over, -1.0);
    assert_eq!(around.get(), 2, "once it is at its end the wheel goes on");
}

/// The box is `rows` rows high with no text, with a little and with a great deal; the caret
/// stays in it, and nothing around it moves (items 23, 32).
#[gpui::test]
fn the_box_is_as_high_as_its_rows_whatever_it_holds(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .w(px(180.0))
            .child(Textarea::new("t", "Notes").rows(4))
            .child(Button::new("after").label("After"))
    });
    assert_still(
        cx,
        &["t/box", "after/button"],
        &[
            Step::Hover("t/box"),
            Step::Press("t/box"),
            Step::Keys("tab"),
            Step::Keys("a b c enter d e f enter g h i enter j k l enter m n o"),
            Step::Keys("ctrl-a backspace"),
        ],
    );
    cx.simulate_input(&"word ".repeat(80));
    redraw(cx);
    let field = box_bounds(cx);
    let end = text(cx).len();
    let caret = point_of(cx, "t", end);
    assert!(
        caret.y >= field.top() && caret.y <= field.bottom(),
        "the caret is in the box: {caret:?} in {field:?}"
    );
    assert_still(cx, &["t/box", "after/button"], &[Step::Keys("ctrl-home")]);
}

/// Item 23: long unbroken text, Japanese and emoji stay inside the box in a narrow window.
#[gpui::test]
fn worst_case_text_stays_inside_the_box(cx: &mut TestAppContext) {
    let worst = "https://example.com/an/extremely/long/path/without/any/spaces/at/all?q=1\n日本語のとても長い文章がここに続きます😀😀😀😀😀😀😀😀\n\n\nEnd";
    let cx = open(cx, move |_, _| {
        v_flex()
            .w(px(130.0))
            .child(Textarea::new("t", "Notes").rows(5).default_value(worst))
            .child(Button::new("after").label("After"))
    });
    let (field, after) = (
        box_bounds(cx),
        cx.debug_bounds("after/button").expect("drawn"),
    );
    assert!(
        after.top() >= field.bottom(),
        "nothing was pushed over the button"
    );
    let inside = field;
    press(cx, "tab ctrl-home");
    for _ in 0..30 {
        press(cx, "down");
        let head = selection(cx).start;
        let at = point_of(cx, "t", head);
        assert!(
            at.x >= inside.left()
                && at.x <= inside.right()
                && at.y >= inside.top()
                && at.y <= inside.bottom(),
            "the caret left the box: {at:?} in {inside:?}"
        );
    }
    assert_eq!(selection(cx).start, worst.len(), "Down reached the end");
}

/// A masked field of rows draws dots, keeps its line breaks and tells nothing.
#[gpui::test]
fn a_masked_field_of_rows_keeps_its_text_to_itself(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Secret")
            .default_value("ab\ncd")
            .masked(true)
    });
    press(cx, "tab ctrl-c");
    assert_eq!(clipboard(cx), None);
    assert_eq!(primary(cx), None);
    press(cx, "ctrl-home down");
    assert_eq!(selection(cx), 3..3);
    assert!(
        row_y(cx, 3) > row_y(cx, 0),
        "the second line is a second row"
    );
    assert_eq!(
        Textarea::new("t", "Secret").masked(true).accessible().value,
        None
    );
}

/// A read-only field of rows selects and copies but does not change; a disabled one does not
/// even take focus.
#[gpui::test]
fn read_only_and_disabled(cx: &mut TestAppContext) {
    let after = Count::default();
    let a = after.clone();
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(
                Textarea::new("t", "Notes")
                    .default_value("a\nb")
                    .read_only(true),
            )
            .child(
                Textarea::new("off", "Off")
                    .default_value("x")
                    .disabled(true),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab");
    cx.simulate_input("x");
    press(cx, "enter backspace");
    assert_eq!(text(cx), "a\nb");
    press(cx, "ctrl-c");
    assert_eq!(clipboard(cx).as_deref(), Some("a\nb"));
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab went past the disabled field");
    assert_eq!(probe(cx, "off").0, "x");
}

/// A controlled field follows its value, and a value with Windows line breaks does not make it
/// replace the text on every frame.
#[gpui::test]
fn a_controlled_field_follows_its_value(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(String::from("a\r\nb")));
    let v = value.clone();
    let cx = open(cx, move |_, _| {
        let v = v.clone();
        let current = v.borrow().clone();
        Textarea::new("t", "Notes")
            .value(current)
            .on_change(move |text, _, _| *v.borrow_mut() = text.to_owned())
    });
    assert_eq!(text(cx), "a\nb");
    press(cx, "tab ctrl-end");
    cx.simulate_input("c");
    redraw(cx);
    assert_eq!(text(cx), "a\nbc");
    press(cx, "ctrl-z");
    assert_eq!(text(cx), "a\nb", "the history survived the redraws");
    *value.borrow_mut() = "zzz".into();
    redraw(cx);
    assert_eq!(text(cx), "zzz");
}

/// Items 14, 15, 16: what it reports.
#[gpui::test]
fn it_describes_itself_as_a_multiline_text_input(_: &mut TestAppContext) {
    let a = Textarea::new("t", "Notes")
        .default_value("a\nb")
        .accessible();
    assert_eq!(a.role, Role::TextInput);
    assert_eq!(a.name, "Notes");
    assert_eq!(a.value.as_deref(), Some("a\nb"));
    assert!(a.multiline && !a.read_only && !a.disabled);
    assert!(
        Textarea::new("t", "Notes")
            .disabled(true)
            .accessible()
            .disabled
    );
    assert!(
        Textarea::new("t", "Notes")
            .read_only(true)
            .accessible()
            .read_only
    );
}

/// The placeholder wraps inside the box while the field is empty: it takes several rows, and
/// the box clips what does not fit.
#[gpui::test]
fn the_placeholder_wraps_inside_the_box(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(160.0)).child(
            Textarea::new("t", "Notes")
                .rows(2)
                .placeholder("Anzeigename für externe Bildschirme und Projektoren und mehr"),
        )
    });
    let field = box_bounds(cx);
    let text = cx
        .debug_bounds("t/placeholder-text")
        .expect("the placeholder is drawn while empty");
    let line = crate::input::drawn::last("t").geometry.line_height;
    assert!(
        text.size.height > px(line * 2.5),
        "it wrapped onto more rows than the box shows: {text:?}"
    );
    assert!(text.left() >= field.left() && text.right() <= field.right());
    let hint = cx.debug_bounds("t/placeholder").unwrap();
    assert!(
        hint.top() >= field.top() && hint.bottom() <= field.bottom(),
        "and the box holds it to its rows: {hint:?} in {field:?}"
    );
}

/// Selecting over rows paints a box for each row, and a line break the selection covers shows
/// even on an empty line.
#[gpui::test]
fn a_selection_over_rows_is_one_box_for_each(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Textarea::new("t", "Notes").default_value("abc\n\ndef")
    });
    let rects = layout_of(cx, "t", |layout, _, _| {
        (
            layout.rects_of(2..6, px(4.0)),
            layout.rects_of(3..3, px(4.0)),
            layout.rects_of(0..3, px(4.0)),
        )
    });
    let (over, empty, line) = rects;
    assert_eq!(
        over.len(),
        3,
        "the end of the first row, the empty row and the start of the last"
    );
    assert!(
        over[1].size.width == px(4.0),
        "the empty line shows its break"
    );
    assert!(over[0].origin.y < over[1].origin.y && over[1].origin.y < over[2].origin.y);
    assert!(empty.is_empty());
    assert_eq!(line.len(), 1);
}

/// Item 13 and ADR 0037: Escape is left to the overlay around a field.
#[gpui::test]
fn escape_reaches_the_dialog_around_the_field(cx: &mut TestAppContext) {
    let closed = Count::default();
    let c = closed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Dialog::new("d", "Notes")
            .open(true)
            .content(|_, _| Textarea::new("t", "Notes").default_value("abc"))
            .on_close(move |_, _| c.bump())
    });
    press(cx, "tab tab");
    assert_eq!(selection(cx), 0..3, "the field has focus");
    press(cx, "enter");
    assert_eq!(closed.get(), 0, "Enter is a line break, and closes nothing");
    press(cx, "escape");
    assert_eq!(closed.get(), 1);
}
