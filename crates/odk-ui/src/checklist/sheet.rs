use std::cell::Cell;
use std::rc::Rc;

use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};

use super::{Count, open, press, reduce_motion};
use crate::{Button, Sheet, v_flex};

/// Items 8, 10, 13: the sheet takes focus when it opens, Tab and Shift+Tab stay inside it,
/// Escape asks to close it, and focus goes back to the button that opened it.
#[gpui::test]
fn traps_tab_and_gives_focus_back(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(false));
    let (opened, inside, outside) = (Count::default(), Count::default(), Count::default());
    let (o, i, x, open_flag) = (
        opened.clone(),
        inside.clone(),
        outside.clone(),
        is_open.clone(),
    );
    let cx = open(cx, move |_, _| {
        let (o, i, x) = (o.clone(), i.clone(), x.clone());
        let (to_open, to_close) = (open_flag.clone(), open_flag.clone());
        v_flex()
            .child(
                Button::new("open")
                    .label("Open")
                    .on_click(move |_, window, _| {
                        o.bump();
                        to_open.set(true);
                        window.refresh();
                    }),
            )
            .child(
                Button::new("outside")
                    .label("Outside")
                    .on_click(move |_, _, _| x.bump()),
            )
            .child(
                Sheet::new("s", "Settings")
                    .open(open_flag.get())
                    .on_close(move |window, _| {
                        to_close.set(false);
                        window.refresh();
                    })
                    .content(move |_, _| {
                        let i = i.clone();
                        Button::new("inside")
                            .label("Inside")
                            .on_click(move |_, _, _| i.bump())
                    }),
            )
    });
    reduce_motion();
    press(cx, "tab enter");
    assert!(is_open.get());
    // Two tab stops inside: the close button and Inside. Four Tabs go round twice.
    press(cx, "tab tab tab tab enter");
    assert_eq!((inside.get(), outside.get()), (1, 0), "Tab stayed inside");
    press(cx, "shift-tab shift-tab enter");
    assert_eq!(
        (inside.get(), outside.get()),
        (2, 0),
        "Shift+Tab stayed inside"
    );
    press(cx, "escape");
    assert!(!is_open.get(), "Escape asked to close");
    press(cx, "enter");
    assert_eq!(
        opened.get(),
        2,
        "focus went back to the button that opened it"
    );
}

/// Items 14, 15.
#[test]
fn reports_a_dialog_named_by_its_title() {
    let a = Sheet::new("s", "Display settings").accessible();
    assert_eq!(
        (a.role, a.name.as_str()),
        (Role::Dialog, "Display settings")
    );
}

/// Item 21: an application may clear the title and content as soon as it asks the sheet to
/// close; the sheet keeps drawing what it had open until its exit has run. (GPUI keeps the last
/// bounds of a part that is no longer drawn, so "gone" is checked by the builder no longer
/// being called.)
#[gpui::test]
fn keeps_its_title_and_content_while_it_closes(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let built = Count::default();
    let (o, b) = (is_open.clone(), built.clone());
    let cx = open(cx, move |_, _| {
        // What an application does when it clears its state on close.
        let sheet = if o.get() {
            let b = b.clone();
            Sheet::new("s", "Display settings")
                .open(true)
                .content(move |_, _| {
                    b.bump();
                    crate::Text::new("Monitors")
                })
        } else {
            Sheet::new("s", "").open(false)
        };
        v_flex().child(sheet)
    });
    let motion = crate::foundation().motion.clone();
    let (enter, exit) = (motion.enter, motion.exit);
    cx.executor().advance_clock(enter * 2);
    super::redraw(cx);
    let title = cx.debug_bounds("s/title").unwrap();
    assert!(title.size.width > gpui::px(0.0));

    is_open.set(false);
    cx.executor().advance_clock(exit / 4);
    super::redraw(cx);
    let during = built.get();
    cx.executor().advance_clock(exit / 4);
    super::redraw(cx);
    assert!(
        built.get() > during,
        "the content is still built during the exit"
    );
    assert_eq!(
        cx.debug_bounds("s/title").unwrap().size,
        title.size,
        "the title is kept"
    );

    cx.executor().advance_clock(exit);
    super::redraw(cx);
    let after = built.get();
    super::redraw(cx);
    assert_eq!(built.get(), after, "nothing is built once the exit has run");
}

/// Item 33: a sheet opened from inside a dialog is drawn above it, takes the focus, and closes
/// before the dialog does.
#[gpui::test]
fn a_sheet_in_a_dialog(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let (dialog_closed, sheet_closed) = (Count::default(), Count::default());
    let s = sheet_closed.clone();
    let cx = dialog_around(cx, dialog_closed.clone(), move || {
        let s = s.clone();
        Sheet::new("inner", "Details")
            .open(true)
            .on_close(move |_, _| s.bump())
    });
    assert!(cx.debug_bounds("inner/panel").is_some());
    press(cx, "escape");
    assert_eq!(sheet_closed.get(), 1, "Escape closed the sheet");
    assert_eq!(dialog_closed.get(), 0, "and not the dialog");
}

/// A window with a button behind a sheet that is open from the start, `modal` or not.
fn sheet_over_a_button(
    cx: &mut gpui::TestAppContext,
    modal: bool,
    behind: Count,
    closed: Count,
    side: crate::Side,
) -> &mut gpui::VisualTestContext {
    let cx = open(cx, move |_, _| {
        let (behind, closed) = (behind.clone(), closed.clone());
        v_flex()
            .child(
                Button::new("behind")
                    .label("Behind")
                    .on_click(move |_, _, _| behind.bump()),
            )
            .child(
                Sheet::new("s", "Inspector")
                    .open(true)
                    .modal(modal)
                    .side(side)
                    .on_close(move |_, _| closed.bump())
                    .content(|_, _| Button::new("inside").label("Inside")),
            )
    });
    reduce_motion();
    super::redraw(cx);
    cx
}

fn click(cx: &mut gpui::VisualTestContext, part: &'static str) {
    let at = cx.debug_bounds(part).unwrap().center();
    cx.simulate_click(at, gpui::Modifiers::none());
    super::redraw(cx);
}

/// A modal sheet blocks the window behind it: a click on a button there goes to the scrim,
/// which asks to close, and does not press the button.
#[gpui::test]
fn a_modal_sheet_blocks_what_is_behind_it(cx: &mut gpui::TestAppContext) {
    let (behind, closed) = (Count::default(), Count::default());
    let cx = sheet_over_a_button(cx, true, behind.clone(), closed.clone(), crate::Side::Right);
    assert!(cx.debug_bounds("s/dim").is_some(), "there is a scrim");
    click(cx, "behind/button");
    assert_eq!(behind.get(), 0, "the button behind took nothing");
    assert_eq!(closed.get(), 1, "the scrim asked to close");
}

/// A side panel (a sheet that is not modal) leaves the window live: no scrim, the button behind
/// works, a click there does not close it, and it did not take focus when it opened.
#[gpui::test]
fn a_side_panel_leaves_the_window_live(cx: &mut gpui::TestAppContext) {
    let (behind, closed) = (Count::default(), Count::default());
    let cx = sheet_over_a_button(
        cx,
        false,
        behind.clone(),
        closed.clone(),
        crate::Side::Right,
    );
    assert!(cx.debug_bounds("s/dim").is_none(), "no scrim");
    assert!(cx.debug_bounds("s/panel").is_some(), "the panel is drawn");
    // The first Tab reaches the first stop of the window, not one inside the panel: nothing
    // moved focus into it when it opened.
    press(cx, "tab enter");
    assert_eq!(behind.get(), 1, "focus was not taken");
    click(cx, "behind/button");
    assert_eq!(behind.get(), 2, "the button behind works");
    assert_eq!(closed.get(), 0, "and the panel stayed");
}

/// A side panel is not a focus trap: Tab walks on out of it, and Escape closes it while focus
/// is inside.
#[gpui::test]
fn a_side_panel_does_not_trap_tab_and_closes_on_escape(cx: &mut gpui::TestAppContext) {
    let (behind, closed) = (Count::default(), Count::default());
    let cx = sheet_over_a_button(cx, false, behind.clone(), closed.clone(), crate::Side::Left);
    // Behind, then the close button and Inside in the panel; one more Tab goes round to Behind.
    press(cx, "tab tab tab tab enter");
    assert_eq!(behind.get(), 1, "Tab went round the window, not the panel");
    press(cx, "tab tab escape");
    assert_eq!(closed.get(), 1, "Escape in the panel asked to close it");
}

/// The panel is attached to its edge, with a border on the side that faces the window and
/// no corner of its own.
#[gpui::test]
fn a_sheet_is_attached_to_its_edge(cx: &mut gpui::TestAppContext) {
    for (side, left_edge) in [(crate::Side::Left, true), (crate::Side::Right, false)] {
        let cx = sheet_over_a_button(cx, false, Count::default(), Count::default(), side);
        let panel = cx.debug_bounds("s/panel").unwrap();
        let window = cx.update(|window, _| window.viewport_size());
        if left_edge {
            assert_eq!(panel.left(), gpui::px(0.0));
        } else {
            assert_eq!(panel.right(), window.width);
        }
        assert_eq!(panel.size.height, window.height, "the full height");
    }
}
