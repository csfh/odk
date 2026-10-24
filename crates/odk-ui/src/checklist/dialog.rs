use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, ParentElement, TestAppContext, VisualTestContext, point, px, size,
};
use odk_a11y::{Describe, Role};

use super::{Count, open, press, redraw, reduce_motion};
use crate::overlay::probe;
use crate::{Button, Dialog, Text, v_flex};

/// A window with a button that opens a dialog, which has a body button and two actions.
fn dialog_window<'a>(
    cx: &'a mut TestAppContext,
    is_open: Rc<Cell<bool>>,
    counts: [Count; 4],
    title: &'static str,
) -> &'a mut VisualTestContext {
    open(cx, move |_, _| {
        let [opened, inside, cancel, save] = counts.clone();
        let (to_open, to_close) = (is_open.clone(), is_open.clone());
        let (c, s) = (cancel.clone(), save.clone());
        v_flex()
            .child(
                Button::new("open")
                    .label("Rename")
                    .on_click(move |_, window, _| {
                        opened.bump();
                        to_open.set(true);
                        window.refresh();
                    }),
            )
            .child(
                Dialog::new("d", title)
                    .open(is_open.get())
                    .description("Choose a new name for the file.")
                    .on_close(move |window, _| {
                        to_close.set(false);
                        window.refresh();
                    })
                    .content(move |_, _| {
                        let i = inside.clone();
                        Button::new("inside")
                            .label("Inside")
                            .on_click(move |_, _, _| i.bump())
                    })
                    .action(move |_, _| {
                        let c = c.clone();
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(move |_, _, _| c.bump())
                    })
                    .action(move |_, _| {
                        let s = s.clone();
                        Button::new("save")
                            .label("Save")
                            .on_click(move |_, _, _| s.bump())
                    }),
            )
    })
}

fn counts() -> [Count; 4] {
    [
        Count::default(),
        Count::default(),
        Count::default(),
        Count::default(),
    ]
}

/// Items 8, 10, 13: the dialog takes focus when it opens; Tab and Shift+Tab stay inside it
/// (close, Inside, Cancel, Save); Escape asks to close it, and focus goes back to the button
/// that opened it.
#[gpui::test]
fn traps_tab_and_gives_focus_back(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(false));
    let c = counts();
    let cx = dialog_window(cx, is_open.clone(), c.clone(), "Rename file");
    let [opened, inside, _, save] = c;
    reduce_motion();
    press(cx, "tab enter");
    assert!(is_open.get());
    // Four stops inside; eight Tabs go round twice and land on Save.
    press(cx, "tab tab tab tab tab tab tab tab enter");
    assert_eq!((save.get(), opened.get()), (1, 1), "Tab stayed inside");
    press(cx, "shift-tab shift-tab enter");
    assert_eq!(inside.get(), 1, "Shift+Tab stayed inside");
    press(cx, "escape");
    assert!(!is_open.get(), "Escape asked to close");
    press(cx, "enter");
    assert_eq!(
        opened.get(),
        2,
        "focus went back to the button that opened it"
    );
}

/// A click on the scrim asks to close it; a click inside the panel does not.
#[gpui::test]
fn a_click_on_the_scrim_closes_it(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let cx = dialog_window(cx, is_open.clone(), counts(), "Rename file");
    reduce_motion();
    redraw(cx);
    let panel = cx.debug_bounds("d/panel").unwrap();
    let middle = point(panel.origin.x + px(10.0), panel.origin.y + px(10.0));
    cx.simulate_click(middle, Modifiers::default());
    redraw(cx);
    assert!(is_open.get(), "a click inside kept it open");
    cx.simulate_click(point(px(2.0), px(2.0)), Modifiers::default());
    redraw(cx);
    assert!(!is_open.get(), "a click on the scrim asked to close it");
}

/// Items 14, 15.
#[test]
fn reports_a_dialog_named_by_its_title() {
    let a = Dialog::new("d", "Rename file").accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Dialog, "Rename file"));
}

/// Items 19, 20: it fades in with a slight rise over the foundation's `enter` and ends centred
/// in the window.
#[gpui::test]
fn it_fades_and_rises_into_the_centre(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(false));
    let cx = dialog_window(cx, is_open.clone(), counts(), "Rename file");
    let enter = crate::foundation().motion.enter;
    press(cx, "tab enter");
    redraw(cx);
    assert_eq!(probe::last("d"), Some(0.0));
    cx.executor().advance_clock(enter / 2);
    redraw(cx);
    let rising = cx.debug_bounds("d/panel").unwrap();
    cx.executor().advance_clock(enter);
    redraw(cx);
    assert_eq!(probe::last("d"), Some(1.0));
    let settled = cx.debug_bounds("d/panel").unwrap();
    assert!(rising.top() > settled.top(), "it rose into place");
    let viewport = cx.update(|window, _| window.viewport_size());
    let centre = settled.origin.y + settled.size.height / 2.0;
    assert!(
        (centre - viewport.height / 2.0).abs() <= px(1.0),
        "centred: {settled:?}"
    );
    let middle = settled.origin.x + settled.size.width / 2.0;
    assert!((middle - viewport.width / 2.0).abs() <= px(1.0));
}

/// Item 23: in a small window the dialog fits with a margin, a long title wraps beside the
/// close button, and a body too tall scrolls while the actions stay in the window.
#[gpui::test]
fn it_fits_a_small_window_and_its_body_scrolls(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let cx = open(cx, {
        let is_open = is_open.clone();
        move |_, _| {
            Dialog::new(
                "d",
                "Anzeigeeinstellungen für externe Bildschirme und Projektoren",
            )
            .open(is_open.get())
            .content(|_, _| {
                let mut body = v_flex();
                for n in 0..60 {
                    body = body.child(Text::new(format!("Zeile {n}")));
                }
                body
            })
            .action(|_, _| Button::new("cancel").label("Abbrechen"))
            .action(|_, _| Button::new("save").label("Umbenennen und verschieben"))
        }
    });
    reduce_motion();
    cx.simulate_resize(size(px(320.0), px(400.0)));
    redraw(cx);
    let viewport = cx.update(|window, _| window.viewport_size());
    let screen = gpui::Bounds::new(point(px(0.0), px(0.0)), viewport);
    let panel = cx.debug_bounds("d/panel").unwrap();
    let inside = |b: gpui::Bounds<gpui::Pixels>, o: gpui::Bounds<gpui::Pixels>| {
        b.left() >= o.left()
            && b.right() <= o.right()
            && b.top() >= o.top()
            && b.bottom() <= o.bottom()
    };
    assert!(
        inside(panel, screen),
        "the dialog fits the window: {panel:?}"
    );
    let close = cx.debug_bounds("d/close/button").unwrap();
    assert!(inside(close, panel), "the close button stays in reach");
    assert!(cx.debug_bounds("d/title").unwrap().right() <= close.left());
    let footer = cx.debug_bounds("d/footer").unwrap();
    assert!(
        inside(footer, panel),
        "the actions stay in the window: {footer:?}"
    );
    // Each action, not only the row: a row of buttons that does not wrap overflows its box.
    for action in ["cancel/button", "save/button"] {
        let b = cx.debug_bounds(action).unwrap();
        assert!(
            inside(b, panel),
            "{action} is out of reach: {b:?} in {panel:?}"
        );
    }
    let body = cx.debug_bounds("d/body").unwrap();
    assert!(
        body.size.height < px(60.0 * 19.0),
        "the body is shorter than its content"
    );
}

/// Item 21: an application may clear its state as soon as it asks the dialog to close; the
/// dialog keeps drawing its body until its exit has run.
#[gpui::test]
fn keeps_its_body_while_it_closes(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let built = Count::default();
    let (o, b) = (is_open.clone(), built.clone());
    let cx = open(cx, move |_, _| {
        if o.get() {
            let b = b.clone();
            Dialog::new("d", "Rename file")
                .open(true)
                .content(move |_, _| {
                    b.bump();
                    Text::new("Body")
                })
        } else {
            Dialog::new("d", "").open(false)
        }
    });
    let motion = crate::foundation().motion.clone();
    let (enter, exit) = (motion.enter, motion.exit);
    cx.executor().advance_clock(enter * 2);
    redraw(cx);
    let title = cx.debug_bounds("d/title").unwrap();
    is_open.set(false);
    cx.executor().advance_clock(exit / 4);
    redraw(cx);
    let during = built.get();
    cx.executor().advance_clock(exit / 4);
    redraw(cx);
    assert!(
        built.get() > during,
        "the body is still built during the exit"
    );
    assert_eq!(
        cx.debug_bounds("d/title").unwrap().size,
        title.size,
        "the title is kept"
    );
    cx.executor().advance_clock(exit);
    redraw(cx);
    let after = built.get();
    redraw(cx);
    assert_eq!(built.get(), after, "nothing is built once the exit has run");
}

/// Item 32: moving focus and the pointer around inside the dialog moves nothing.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    let is_open = Rc::new(Cell::new(true));
    let cx = dialog_window(cx, is_open, counts(), "Rename file");
    reduce_motion();
    redraw(cx);
    assert_still(
        cx,
        &[
            "d/panel",
            "d/body",
            "d/footer",
            "save/button",
            "cancel/button",
        ],
        &[
            Step::Keys("tab"),
            Step::Keys("tab"),
            Step::Hover("save/button"),
            Step::Press("cancel/button"),
            Step::Keys("tab"),
        ],
    );
}

/// A dialog that is open from the start, with `inside` as its body. `closed` counts its
/// requests to close. For item 33: overlays opened from inside a modal one.
pub(crate) fn dialog_around<E: IntoElement + 'static>(
    cx: &mut TestAppContext,
    closed: Count,
    inside: impl Fn() -> E + 'static,
) -> &mut VisualTestContext {
    let inside = Rc::new(inside);
    let cx = open(cx, move |_, _| {
        let (closed, inside) = (closed.clone(), inside.clone());
        Dialog::new("dlg", "Choose")
            .open(true)
            .on_close(move |_, _| closed.bump())
            .content(move |_, _| inside())
    });
    reduce_motion();
    redraw(cx);
    cx
}

/// Item 33: a dialog opened from inside a sheet is drawn above it, takes the focus, and closes
/// before the sheet does.
#[gpui::test]
fn a_dialog_in_a_sheet(cx: &mut TestAppContext) {
    use crate::Sheet;
    let (sheet_closed, dialog_closed) = (Count::default(), Count::default());
    let (s, d) = (sheet_closed.clone(), dialog_closed.clone());
    let cx = open(cx, move |_, _| {
        let d = d.clone();
        Sheet::new("sh", "Settings")
            .open(true)
            .on_close({
                let s = s.clone();
                move |_, _| s.bump()
            })
            .content(move |_, _| {
                let d = d.clone();
                Dialog::new("inner", "Confirm")
                    .open(true)
                    .on_close(move |_, _| d.bump())
            })
    });
    reduce_motion();
    redraw(cx);
    assert!(cx.debug_bounds("inner/panel").is_some());
    // A click inside the dialog reaches the dialog, not the sheet's scrim under it.
    let at = cx.debug_bounds("inner/panel").unwrap().center();
    cx.simulate_click(at, gpui::Modifiers::none());
    assert_eq!(sheet_closed.get(), 0, "the click did not reach the sheet");
    super::press(cx, "escape");
    assert_eq!(dialog_closed.get(), 1, "Escape closed the dialog");
    assert_eq!(sheet_closed.get(), 0, "and not the sheet");
    // The dialog's own scrim covers the sheet: a click there closes the dialog, not the sheet.
    cx.simulate_click(
        gpui::point(gpui::px(5.0), gpui::px(5.0)),
        gpui::Modifiers::none(),
    );
    assert_eq!(dialog_closed.get(), 2);
    assert_eq!(sheet_closed.get(), 0);
}

/// The window's overlay hosts take no room in the view's column: a gap on the view's root does
/// not open up above its first child.
#[gpui::test]
fn the_overlay_host_takes_no_room_in_the_root(cx: &mut TestAppContext) {
    use gpui::{
        AppContext as _, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled,
        Window, div, px,
    };
    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            crate::root()
                .gap(px(16.0))
                .child(div().h(px(10.0)).debug_selector(|| "first".to_string()))
        }
    }
    crate::theme::install_test_theme();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let page = cx.new(|_| Page);
        crate::app::shell(window, cx, page.into())
    });
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("first").unwrap().top(), px(0.0));
}

/// Item 33: a list that is open in a sheet is below a dialog opened over it, though the dialog
/// is declared first: a press on the list goes to the dialog's scrim, not to an option.
#[gpui::test]
fn a_popover_in_a_sheet_is_below_a_dialog_opened_over_it(cx: &mut TestAppContext) {
    use std::cell::{Cell, RefCell};

    use crate::{Select, Sheet, v_flex};
    use gpui::{ParentElement, px};
    let (shown, picks, closed) = (
        Rc::new(Cell::new(false)),
        Rc::new(RefCell::new(Vec::<String>::new())),
        Count::default(),
    );
    let (s, p, c) = (shown.clone(), picks.clone(), closed.clone());
    let cx = open(cx, move |_, _| {
        let (s, p, c) = (s.clone(), p.clone(), c.clone());
        Sheet::new("sh", "Settings")
            .open(true)
            .content(move |_, _| {
                let (p, c) = (p.clone(), c.clone());
                v_flex()
                    .child(
                        Dialog::new("inner", "Confirm")
                            .open(s.get())
                            .on_close(move |_, _| c.bump()),
                    )
                    .child(
                        Select::new("sl", "Fruit")
                            .item("a", "Apple")
                            .item("b", "Banana")
                            .on_change(move |v, _, _| p.borrow_mut().push(v.to_string())),
                    )
            })
    });
    reduce_motion();
    redraw(cx);
    let at = cx.debug_bounds("sl/trigger").unwrap().center();
    cx.simulate_click(at, gpui::Modifiers::none());
    redraw(cx);
    assert!(cx.debug_bounds("sl/row-1").is_some(), "the list is open");
    shown.set(true);
    redraw(cx);
    let row = cx.debug_bounds("sl/row-1").unwrap().center();
    assert!(row.y > px(0.0));
    cx.simulate_click(row, gpui::Modifiers::none());
    assert!(
        picks.borrow().is_empty(),
        "the press did not reach the list"
    );
    assert_eq!(closed.get(), 1, "it reached the dialog's scrim");
}

/// A window opened with GPUI's own `open_window` has no overlay host, so an overlay opened
/// inside a modal says so instead of never showing.
#[gpui::test]
#[should_panic(expected = "odk::ui::open_window")]
fn a_raw_window_cannot_show_an_overlay_inside_a_modal(cx: &mut TestAppContext) {
    use gpui::{Context, IntoElement, Render, Window};
    struct Raw;
    impl Render for Raw {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Dialog::new("raw", "Choose")
                .open(true)
                .content(|_, _| crate::Select::new("sl", "Fruit").item("a", "Apple"))
        }
    }
    crate::theme::install_test_theme();
    let (_, cx) = cx.add_window_view(|_, _| Raw);
    cx.run_until_parked();
    // Open the list, which is what lifts something.
    let at = cx.debug_bounds("sl/trigger").unwrap().center();
    cx.simulate_click(at, gpui::Modifiers::none());
}

/// A window that shows an open dialog with the given dismissal policy; `closed` counts how many
/// times it was asked to close.
fn policy_window(
    cx: &mut TestAppContext,
    closed: Count,
    escape: bool,
    backdrop: bool,
    button: bool,
) -> &mut VisualTestContext {
    open(cx, move |_, _| {
        let closed = closed.clone();
        Dialog::new("d", "Delete 12 files?")
            .open(true)
            .close_on_escape(escape)
            .close_on_backdrop(backdrop)
            .close_button(button)
            .on_close(move |_, _| closed.bump())
            .action(|_, _| Button::new("keep").label("Keep"))
    })
}

/// The dismissal policy (docs/spec/odk-overlays.md, "Dismissal"): a confirmation of something
/// destructive turns the backdrop off, and a busy dialog turns Escape and the close button off
/// as well. Each way out that is on still works.
#[gpui::test]
fn the_dismissal_policy_decides_what_closes_it(cx: &mut TestAppContext) {
    reduce_motion();
    let backdrop = point(px(2.0), px(2.0));

    let closed = Count::default();
    let cx = policy_window(cx, closed.clone(), true, false, true);
    redraw(cx);
    cx.simulate_click(backdrop, Modifiers::default());
    assert_eq!(closed.get(), 0, "a stray click does not dismiss it");
    assert!(
        cx.debug_bounds("d/close/button").is_some(),
        "the close button"
    );
    press(cx, "escape");
    assert_eq!(closed.get(), 1, "Escape still does");
}

#[gpui::test]
fn a_dialog_with_every_way_out_off_is_closed_only_by_its_application(cx: &mut TestAppContext) {
    reduce_motion();
    let closed = Count::default();
    let cx = policy_window(cx, closed.clone(), false, false, false);
    redraw(cx);
    cx.simulate_click(point(px(2.0), px(2.0)), Modifiers::default());
    press(cx, "escape");
    assert_eq!(closed.get(), 0, "neither the backdrop nor Escape asks");
    assert!(
        cx.debug_bounds("d/close/button").is_none(),
        "no close button"
    );
}

/// Items 19, 20, 22: it comes in over the foundation's `enter` and goes out over its `exit`,
/// two different lengths, and reduced motion skips both.
#[gpui::test]
fn it_enters_over_enter_and_leaves_over_exit(cx: &mut TestAppContext) {
    let is_open = Rc::new(Cell::new(true));
    let cx = dialog_window(cx, is_open.clone(), counts(), "Rename file");
    let motion = crate::foundation().motion.clone();
    assert!(motion.enter > motion.exit, "the two lengths differ");
    redraw(cx);
    assert_eq!(probe::last("d"), Some(0.0));
    cx.executor().advance_clock(motion.enter / 2);
    redraw(cx);
    let halfway = probe::last("d").unwrap();
    assert!((halfway - 0.5).abs() < 0.01, "{halfway} halfway in");
    cx.executor().advance_clock(motion.enter);
    redraw(cx);
    assert_eq!(probe::last("d"), Some(1.0));

    is_open.set(false);
    redraw(cx);
    cx.executor().advance_clock(motion.exit / 2);
    redraw(cx);
    let halfway = probe::last("d").unwrap();
    assert!((halfway - 0.5).abs() < 0.01, "{halfway} halfway out");
    cx.executor().advance_clock(motion.exit);
    redraw(cx);
    assert_eq!(probe::last("d"), None, "gone after `exit`, not `enter`");
}

/// Item 34: its padding and the gap between its parts are the dialog token's, which is layout
/// spacing, not a control's height.
#[gpui::test]
fn it_is_spaced_by_the_dialog_token(cx: &mut TestAppContext) {
    let cx = dialog_window(cx, Rc::new(Cell::new(true)), counts(), "Rename file");
    reduce_motion();
    redraw(cx);
    let g = crate::components().dialog().geometry;
    let panel = cx.debug_bounds("d/panel").unwrap();
    let title = cx.debug_bounds("d/title").unwrap();
    assert_eq!(
        title.left() - panel.left(),
        px(g.border_width + g.padding_x),
        "the title is a border and the padding in"
    );
    assert_eq!(title.top() - panel.top(), px(g.border_width + g.padding_y));
    let footer = cx.debug_bounds("d/footer").unwrap();
    assert_eq!(
        footer.size.height,
        px(g.gap + g.padding_y) + cx.debug_bounds("save/button").unwrap().size.height,
        "the footer holds its actions between a gap and the padding"
    );
}
