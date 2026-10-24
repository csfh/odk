//! `Command` (ADR 0038, `docs/spec/odk-text-input.md`) on GPUI's headless test platform. The
//! field inside is an `Input`, tested in `input.rs`; this file tests the palette around it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    Modifiers, MouseButton, ParentElement, TestAppContext, VisualTestContext, point, px, size,
};
use odk_a11y::{Describe, Role};

use super::stability::{Step, assert_still};
use super::{Count, open, press, redraw, reduce_motion};
use crate::command::{hear, probe};
use crate::input::probe as field;
use crate::overlay::probe as overlay;
use crate::{Button, Command, CommandItem, Dialog, v_flex};

type Picked = Rc<RefCell<Vec<String>>>;

/// The items of the palettes below, by index: 0 Open file, 1 Save file, 2 Save as… (disabled),
/// 3 Close window (found by "quit" too), 4 Rename.
fn items(picked: &Picked) -> Vec<CommandItem> {
    let item = |label: &'static str| {
        let picked = picked.clone();
        CommandItem::new(label).on_select(move |_, _| picked.borrow_mut().push(label.to_owned()))
    };
    vec![
        item("Open file").hint("Ctrl+O").keywords("load"),
        item("Save file").hint("Ctrl+S"),
        item("Save as…").disabled(true),
        item("Close window").keywords("quit exit"),
        item("Rename"),
    ]
}

/// A window with a button that opens a palette.
fn palette_window(
    cx: &mut TestAppContext,
    is_open: Rc<Cell<bool>>,
    picked: Picked,
) -> &mut VisualTestContext {
    let cx = open(cx, move |_, _| {
        let (to_open, to_close) = (is_open.clone(), is_open.clone());
        let mut command = Command::new("cmd", "Commands")
            .open(is_open.get())
            .on_close(move |window, _| {
                to_close.set(false);
                window.refresh();
            });
        for item in items(&picked) {
            command = command.item(item);
        }
        v_flex()
            .child(
                Button::new("open")
                    .label("Commands")
                    .on_click(move |_, window, _| {
                        to_open.set(true);
                        window.refresh();
                    }),
            )
            .child(command)
    });
    reduce_motion();
    cx
}

/// Opens the palette from its button, and lets it settle.
fn opened(cx: &mut TestAppContext) -> (&mut VisualTestContext, Rc<Cell<bool>>, Picked) {
    let (is_open, picked) = (Rc::new(Cell::new(false)), Picked::default());
    let cx = palette_window(cx, is_open.clone(), picked.clone());
    press(cx, "tab enter");
    redraw(cx);
    assert!(is_open.get());
    (cx, is_open, picked)
}

fn query(cx: &mut VisualTestContext) -> String {
    probe(cx, "cmd").0
}

fn listed(cx: &mut VisualTestContext) -> Vec<usize> {
    probe(cx, "cmd").1
}

fn highlighted(cx: &mut VisualTestContext) -> Option<usize> {
    probe(cx, "cmd").2
}

/// Items 8, 13: opening puts focus in the field, empty; typing narrows the list, highlights
/// the first command left and Enter closes the palette and runs it.
#[gpui::test]
fn typing_narrows_the_list_and_enter_runs_the_first(cx: &mut TestAppContext) {
    let (cx, is_open, picked) = opened(cx);
    assert_eq!(query(cx), "");
    assert_eq!(listed(cx), [0, 1, 2, 3, 4]);
    assert_eq!(highlighted(cx), Some(0));
    cx.simulate_input("sa");
    redraw(cx);
    assert_eq!(query(cx), "sa");
    assert_eq!(listed(cx), [1, 2], "Save file and Save as…");
    assert_eq!(
        highlighted(cx),
        Some(1),
        "the disabled one is not highlighted"
    );
    press(cx, "enter");
    assert!(!is_open.get(), "it closed");
    assert_eq!(*picked.borrow(), ["Save file"]);
}

/// Matching ignores case, takes several words in any order, and looks at the keywords too.
#[gpui::test]
fn matching_takes_words_keywords_and_any_case(cx: &mut TestAppContext) {
    let (cx, _, _) = opened(cx);
    cx.simulate_input("FILE open");
    redraw(cx);
    assert_eq!(listed(cx), [0]);
    press(cx, "ctrl-a backspace");
    cx.simulate_input("quit");
    redraw(cx);
    assert_eq!(listed(cx), [3], "a keyword finds Close window");
    press(cx, "ctrl-a backspace");
    cx.simulate_input("load");
    redraw(cx);
    assert_eq!(listed(cx), [0]);
}

/// Down and Up move the highlight, skip the disabled command and wrap round.
#[gpui::test]
fn down_and_up_move_the_highlight_and_wrap(cx: &mut TestAppContext) {
    let (cx, _, picked) = opened(cx);
    assert_eq!(highlighted(cx), Some(0));
    press(cx, "down");
    assert_eq!(highlighted(cx), Some(1));
    press(cx, "down");
    assert_eq!(highlighted(cx), Some(3), "Save as… is skipped");
    press(cx, "down down");
    assert_eq!(highlighted(cx), Some(0), "wrapped past the end");
    press(cx, "up");
    assert_eq!(highlighted(cx), Some(4), "and back past the start");
    press(cx, "enter");
    assert_eq!(*picked.borrow(), ["Rename"]);
}

/// A new query puts the highlight back on the first command left.
#[gpui::test]
fn a_new_query_highlights_the_first_command_left(cx: &mut TestAppContext) {
    let (cx, _, _) = opened(cx);
    press(cx, "down down");
    assert_eq!(highlighted(cx), Some(3));
    cx.simulate_input("e");
    redraw(cx);
    assert_eq!(highlighted(cx), Some(0));
}

/// With nothing left to run Enter does nothing, and the palette says so.
#[gpui::test]
fn with_no_match_enter_does_nothing_and_it_says_so(cx: &mut TestAppContext) {
    let (cx, is_open, picked) = opened(cx);
    cx.simulate_input("zzz");
    redraw(cx);
    assert_eq!(listed(cx), Vec::<usize>::new());
    assert_eq!(highlighted(cx), None);
    assert!(
        cx.debug_bounds("cmd/empty").is_some(),
        "No results is shown"
    );
    press(cx, "enter down up");
    assert!(is_open.get());
    assert!(picked.borrow().is_empty());
}

/// Items 10, 13: Escape asks to close, and focus goes back to the button that opened it; the
/// next time it opens, nothing is typed.
#[gpui::test]
fn escape_closes_it_and_gives_focus_back(cx: &mut TestAppContext) {
    let (cx, is_open, _) = opened(cx);
    cx.simulate_input("sa");
    redraw(cx);
    press(cx, "escape");
    assert!(!is_open.get());
    redraw(cx);
    press(cx, "enter");
    redraw(cx);
    assert!(is_open.get(), "focus was back on the button");
    assert_eq!(query(cx), "", "what was typed is not kept");
    assert_eq!(listed(cx).len(), 5);
    cx.simulate_input("r");
    redraw(cx);
    assert_eq!(query(cx), "r", "and focus is in the field again");
}

/// Item 9, 13: Tab stays inside the palette, where the field is its only tab stop.
#[gpui::test]
fn tab_stays_in_the_field(cx: &mut TestAppContext) {
    let (cx, is_open, _) = opened(cx);
    press(cx, "tab tab shift-tab");
    cx.simulate_input("o");
    redraw(cx);
    assert_eq!(query(cx), "o");
    assert!(is_open.get());
}

/// A click on a command runs it as Enter does; hovering one highlights it; a click on a
/// disabled command does nothing.
#[gpui::test]
fn the_pointer_highlights_and_runs(cx: &mut TestAppContext) {
    let (cx, is_open, picked) = opened(cx);
    let row = |cx: &mut VisualTestContext, name: &'static str| {
        cx.debug_bounds(name).expect("the row was drawn").center()
    };
    let at = row(cx, "cmd/row-3");
    cx.simulate_mouse_move(at, None, Modifiers::default());
    redraw(cx);
    assert_eq!(highlighted(cx), Some(3), "hovering highlights");
    let disabled = row(cx, "cmd/row-2");
    cx.simulate_mouse_move(disabled, None, Modifiers::default());
    cx.simulate_click(disabled, Modifiers::default());
    redraw(cx);
    assert!(
        is_open.get() && picked.borrow().is_empty(),
        "a disabled command is inert"
    );
    assert_eq!(highlighted(cx), Some(3));
    let at = row(cx, "cmd/row-4");
    cx.simulate_click(at, Modifiers::default());
    redraw(cx);
    assert!(!is_open.get());
    assert_eq!(*picked.borrow(), ["Rename"]);
}

/// A click on the scrim asks to close; a click inside the panel, even on its padding, keeps
/// focus in the field.
#[gpui::test]
fn a_click_outside_closes_and_one_inside_keeps_the_field(cx: &mut TestAppContext) {
    let (cx, is_open, _) = opened(cx);
    let panel = cx.debug_bounds("cmd/panel").unwrap();
    cx.simulate_click(
        point(panel.origin.x + px(2.0), panel.origin.y + px(2.0)),
        Modifiers::default(),
    );
    redraw(cx);
    cx.simulate_input("x");
    redraw(cx);
    assert!(is_open.get(), "a click inside did not close it");
    assert_eq!(query(cx), "x", "the field still has focus");
    cx.simulate_click(point(px(1.0), px(1.0)), Modifiers::default());
    redraw(cx);
    assert!(!is_open.get(), "a click on the scrim closed it");
}

/// A list longer than the box scrolls, and the highlighted command stays in view (item 23).
#[gpui::test]
fn the_highlighted_command_stays_in_view(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let o = is_open.clone();
    let cx = open(cx, move |_, _| {
        let o2 = o.clone();
        let mut command = Command::new("cmd", "Commands")
            .open(o.get())
            .on_close(move |_, _| o2.set(false));
        for n in 0..60 {
            command = command.item(CommandItem::new(format!("Command number {n}")));
        }
        command
    });
    reduce_motion();
    redraw(cx);
    redraw(cx);
    for _ in 0..40 {
        press(cx, "down");
    }
    redraw(cx);
    let (list, row) = (
        cx.debug_bounds("cmd/list").unwrap(),
        cx.debug_bounds("cmd/row-40").unwrap(),
    );
    assert_eq!(highlighted(cx), Some(40));
    assert!(
        row.top() >= list.top() - px(1.0) && row.bottom() <= list.bottom() + px(1.0),
        "row 40 is in view: {row:?} in {list:?}"
    );
}

/// Item 23: long labels truncate, many hints and Japanese stay inside a panel that fits a
/// small window.
#[gpui::test]
fn worst_case_content_stays_inside_the_panel_and_the_window(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Command::new("cmd", "Commands")
            .open(true)
            .placeholder("Pneumonoultramicroscopicsilicovolcanoconiosis und Bildschirmprojektoren")
            .item(
                CommandItem::new(
                    "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed",
                )
                .hint("Ctrl+Shift+Alt+K"),
            )
            .item(CommandItem::new("日本語のとても長いコマンドの名前がここに入ります 😀😀😀"))
    });
    reduce_motion();
    cx.simulate_resize(size(px(320.0), px(260.0)));
    redraw(cx);
    redraw(cx);
    let panel = cx.debug_bounds("cmd/panel").unwrap();
    assert!(
        panel.left() >= px(0.0) && panel.right() <= px(320.0),
        "{panel:?}"
    );
    assert!(
        panel.top() >= px(0.0) && panel.bottom() <= px(260.0),
        "{panel:?}"
    );
    for part in ["cmd/row-0", "cmd/label-0", "cmd/row-1", "cmd/label-1"] {
        let b = cx.debug_bounds(part).unwrap();
        assert!(
            b.left() >= panel.left() && b.right() <= panel.right(),
            "{part} is inside the panel: {b:?} in {panel:?}"
        );
    }
}

/// Item 21: while it closes it keeps drawing the commands it had, even when the application
/// has stopped giving them.
#[gpui::test]
fn it_keeps_its_commands_while_it_fades_out(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let o = is_open.clone();
    let cx = open(cx, move |_, _| {
        let command = Command::new("cmd", "Commands").open(o.get());
        if o.get() {
            command
                .item(CommandItem::new("Open"))
                .item(CommandItem::new("Save"))
        } else {
            command
        }
    });
    let fast = crate::foundation().motion.fast;
    redraw(cx);
    cx.executor().advance_clock(fast * 2);
    redraw(cx);
    assert_eq!(overlay::last("cmd"), Some(1.0));
    is_open.set(false);
    redraw(cx);
    cx.executor().advance_clock(fast / 2);
    redraw(cx);
    let amount = overlay::last("cmd").expect("still drawn");
    assert!(amount > 0.0 && amount < 1.0, "mid-exit: {amount}");
    assert_eq!(probe(cx, "cmd").3, 2, "it still has its two commands");
    cx.executor().advance_clock(fast * 2);
    redraw(cx);
    assert_eq!(overlay::last("cmd"), None, "and then it is gone");
}

/// Item 33: a palette opened from inside a dialog is drawn above it, takes the keys, and
/// closes before the dialog does.
#[gpui::test]
fn a_palette_opened_in_a_dialog_works_there(cx: &mut TestAppContext) {
    let (dialog_closed, palette_open) = (Count::default(), Rc::new(Cell::new(false)));
    let (d, p) = (dialog_closed.clone(), palette_open.clone());
    let picked = Picked::default();
    let pk = picked.clone();
    let cx =
        open(cx, move |_, _| {
            let (d, p, pk) = (d.clone(), p.clone(), pk.clone());
            Dialog::new("dlg", "Tools")
                .open(true)
                .on_close(move |_, _| d.bump())
                .content(move |_, _| {
                    let (p1, p2, pk) = (p.clone(), p.clone(), pk.clone());
                    let mut command =
                        Command::new("cmd", "Commands")
                            .open(p.get())
                            .on_close(move |window, _| {
                                p1.set(false);
                                window.refresh();
                            });
                    for item in items(&pk) {
                        command = command.item(item);
                    }
                    v_flex()
                        .child(Button::new("open").label("Commands").on_click(
                            move |_, window, _| {
                                p2.set(true);
                                window.refresh();
                            },
                        ))
                        .child(command)
                })
        });
    reduce_motion();
    redraw(cx);
    press(cx, "tab tab enter");
    redraw(cx);
    assert!(palette_open.get(), "the button in the dialog opened it");
    cx.simulate_input("ren");
    redraw(cx);
    assert_eq!(query(cx), "ren");
    press(cx, "escape");
    assert!(!palette_open.get(), "Escape closed the palette");
    assert_eq!(dialog_closed.get(), 0, "and not the dialog");
    redraw(cx);
    press(cx, "enter");
    redraw(cx);
    cx.simulate_input("sa");
    redraw(cx);
    press(cx, "enter");
    assert_eq!(*picked.borrow(), ["Save file"]);
    let _ = (field, MouseButton::Left);
}

/// Items 14, 15, 17: what it reports.
#[test]
fn it_reports_a_dialog_with_a_list_of_options() {
    let command = Command::new("cmd", "Commands")
        .item(CommandItem::new("Open"))
        .item(CommandItem::new("Save").disabled(true));
    let a = command.accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Dialog, "Commands"));
    let all = command.item_accessibles(Some(0));
    assert_eq!(all[0].role, Role::Listbox);
    assert_eq!(
        (all[1].role, all[1].selected, all[1].disabled),
        (Role::Option, Some(true), false)
    );
    assert_eq!(
        (all[2].name.as_str(), all[2].selected, all[2].disabled),
        ("Save", Some(false), true)
    );
}

/// Opening it again while it is still fading out starts with nothing typed.
#[gpui::test]
fn reopening_it_mid_exit_starts_empty(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let o = is_open.clone();
    let cx = open(cx, move |_, _| {
        let mut command = Command::new("cmd", "Commands").open(o.get());
        for item in items(&Picked::default()) {
            command = command.item(item);
        }
        command
    });
    let fast = crate::foundation().motion.fast;
    redraw(cx);
    cx.executor().advance_clock(fast * 2);
    redraw(cx);
    cx.simulate_input("sa");
    redraw(cx);
    assert_eq!(query(cx), "sa");
    is_open.set(false);
    redraw(cx);
    cx.executor().advance_clock(fast / 4);
    redraw(cx);
    assert!(
        overlay::last("cmd").is_some_and(|a| a > 0.0),
        "still fading"
    );
    is_open.set(true);
    redraw(cx);
    assert_eq!(query(cx), "", "what was typed is gone");
    assert_eq!(listed(cx).len(), 5);
}

/// Enter runs what the palette has heard, not what the last frame showed: the frame after the
/// last key may not have been drawn when Enter arrives.
#[gpui::test]
fn enter_runs_what_was_heard_before_the_next_frame(cx: &mut TestAppContext) {
    let (cx, _, picked) = opened(cx);
    hear(cx, "cmd", "ren", None);
    press(cx, "enter");
    assert_eq!(
        *picked.borrow(),
        ["Rename"],
        "not Open file, the frame's highlight"
    );
}

/// Down moves from the highlight the palette has heard of, not the last frame's.
#[gpui::test]
fn down_follows_the_highlight_heard_before_the_next_frame(cx: &mut TestAppContext) {
    let (cx, _, picked) = opened(cx);
    hear(cx, "cmd", "", Some(1));
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        *picked.borrow(),
        ["Close window"],
        "from Save file, past Save as…, not from Open file"
    );
}

/// Item 32: the field stays where it is whatever the pointer, the keys and the list do, and
/// so does the top of the panel, however few commands are left (items 23, 32).
#[gpui::test]
fn nothing_moves_when_it_is_used(cx: &mut TestAppContext) {
    let (cx, _, _) = opened(cx);
    assert_still(
        cx,
        &["cmd/input/box"],
        &[
            Step::Hover("cmd/row-1"),
            Step::Keys("down up down"),
            Step::Keys("s a v e"),
            Step::Keys("z z z"),
            Step::Keys("ctrl-a backspace"),
        ],
    );
    let top = cx.debug_bounds("cmd/panel").unwrap().top();
    cx.simulate_input("zzz");
    redraw(cx);
    assert_eq!(cx.debug_bounds("cmd/panel").unwrap().top(), top);
    press(cx, "ctrl-a backspace");
    redraw(cx);
    assert_eq!(cx.debug_bounds("cmd/panel").unwrap().top(), top);
}
