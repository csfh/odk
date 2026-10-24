//! What every overlay shares (`docs/spec/odk-overlays.md`): layer order, dismissal that goes no
//! further than the overlay, one callback per request, a live theme change, and the toast stack
//! above a modal.

use std::time::Duration;

use gpui::{Modifiers, ParentElement, TestAppContext, VisualTestContext};

use super::dialog::dialog_around;
use super::{Count, FIXTURES, open, press, publish_fixture, redraw, reduce_motion};
use crate::overlay::probe;
use crate::{
    Button, ContextMenu, Dialog, DropdownMenu, Input, MenuItem, Popover, Text, Toast, Tooltip,
    h_flex, toast, v_flex,
};

fn click(cx: &mut VisualTestContext, part: &'static str) {
    let at = cx.debug_bounds(part).unwrap().center();
    cx.simulate_click(at, Modifiers::none());
    redraw(cx);
}

/// A tooltip's trigger behind an open dialog is not hovered: the scrim takes the pointer, so no
/// tooltip opens above a modal (docs/spec/odk-overlays.md, "Layers").
#[gpui::test]
fn a_tooltip_behind_a_modal_does_not_open(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Tooltip::new(
                "tt",
                "Save the file",
                Button::new("save").label("Save"),
            ))
            .child(Dialog::new("dlg", "Busy").open(true))
    });
    reduce_motion();
    redraw(cx);
    let at = cx.debug_bounds("save/button").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor()
        .advance_clock(crate::foundation().motion.hover_open * 2);
    redraw(cx);
    assert_eq!(probe::last("tt"), None, "the tooltip stayed shut");
}

/// A toast raised while a dialog is open is above it: it is drawn, takes the pointer through
/// the scrim and does not take focus from the dialog.
#[gpui::test]
fn a_toast_is_above_a_modal_and_leaves_its_focus_alone(cx: &mut TestAppContext) {
    let inside = Count::default();
    let i = inside.clone();
    let cx = dialog_around(cx, Count::default(), move || {
        let i = i.clone();
        Button::new("inside")
            .label("Inside")
            .on_click(move |_, _, _| i.bump())
    });
    let t = Toast::new("Saved").persistent();
    let id = t.id();
    cx.update(|window, cx| toast(window, cx, t));
    redraw(cx);
    assert!(crate::toast::is_shown(cx, id));
    // Focus is still in the dialog, and Tab stays in it: its close button, then Inside; the
    // toast's buttons are not in that lap.
    press(cx, "tab tab enter");
    assert_eq!(inside.get(), 1, "keys went to the dialog");
    let dismiss = cx
        .debug_bounds(Box::leak(
            format!("odk-toast-dismiss-{}/button", {
                format!("{id:?}")
                    .trim_start_matches("ToastId(")
                    .trim_end_matches(')')
                    .to_owned()
            })
            .into_boxed_str(),
        ))
        .unwrap();
    cx.simulate_click(dismiss.center(), Modifiers::none());
    redraw(cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    redraw(cx);
    assert!(
        !crate::toast::is_shown(cx, id),
        "the toast's button worked through the scrim"
    );
}

/// A theme change colours what is open and moves nothing: a dialog and a popover stay open
/// where they were, and focus stays where it was.
#[gpui::test]
fn a_theme_change_keeps_open_overlays_and_focus_where_they_were(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        h_flex()
            .child(
                Popover::new("pop", "Filter", Button::new("t").label("Filter"))
                    .default_open(true)
                    .content(|_, _| Text::new("Body")),
            )
            .child(
                Dialog::new("dlg", "Rename")
                    .open(true)
                    .content(|_, _| Input::new("name", "Name").default_value("notes.md")),
            )
    });
    reduce_motion();
    redraw(cx);
    press(cx, "tab");
    let panel = cx.debug_bounds("dlg/panel").unwrap();
    let focus = cx.update(|window, cx| window.focused(cx));
    assert!(focus.is_some(), "focus is in the dialog");
    let before = crate::components().popover().surface.background;
    for (_, colours) in FIXTURES {
        publish_fixture(colours);
        redraw(cx);
        assert_eq!(probe::last("pop"), Some(1.0), "the popover stayed open");
        assert_eq!(probe::last("dlg"), Some(1.0), "the dialog stayed open");
        assert_eq!(
            cx.debug_bounds("dlg/panel").unwrap(),
            panel,
            "nothing moved"
        );
        assert_eq!(
            cx.update(|window, cx| window.focused(cx)),
            focus,
            "focus stayed"
        );
    }
    assert_ne!(
        before,
        crate::components().popover().surface.background,
        "and what components draw is the last theme's"
    );
}

/// A menu open during a theme change stays open on the item the keyboard was on, and its rows
/// are drawn in the new theme's colours.
#[gpui::test]
fn an_open_menu_keeps_its_item_through_a_theme_change(cx: &mut TestAppContext) {
    use crate::menu_list::drawn;
    let cx = open(cx, |_, _| {
        DropdownMenu::new("dd", "File").menu(|m| {
            m.item(MenuItem::new("New"))
                .item(MenuItem::new("Open"))
                .item(MenuItem::new("Save"))
        })
    });
    reduce_motion();
    press(cx, "tab enter down");
    redraw(cx);
    let before = drawn::last("dd/row-1");
    assert!(before.colours.default.foreground.a > 0.0);
    publish_fixture(FIXTURES[4].1);
    redraw(cx);
    let after = drawn::last("dd/row-1");
    assert_ne!(
        before.current.background, after.current.background,
        "the highlight is drawn in the new theme's colour"
    );
    assert_eq!(probe::last("dd"), Some(1.0), "the menu stayed open");
    press(cx, "enter");
    redraw(cx);
    assert_eq!(probe::last("dd"), None, "and Down's item was still the one");
}

/// One request, one callback: Escape on a dialog runs its `on_close` once.
#[gpui::test]
fn escape_asks_a_dialog_to_close_once(cx: &mut TestAppContext) {
    let closed = Count::default();
    let c = closed.clone();
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        Dialog::new("dlg", "Hello")
            .open(true)
            .on_close(move |_, _| c.bump())
    });
    reduce_motion();
    redraw(cx);
    press(cx, "escape");
    assert_eq!(closed.get(), 1);
}

/// A context menu's choice runs once.
#[gpui::test]
fn a_context_menu_choice_runs_once(cx: &mut TestAppContext) {
    let chosen = Count::default();
    let k = chosen.clone();
    let cx = open(cx, move |_, _| {
        let k = k.clone();
        ContextMenu::new("ctx", "Row", Text::new("Row")).menu(move |m| {
            let k = k.clone();
            m.item(MenuItem::new("New").on_select(move |_, _| k.bump()))
        })
    });
    reduce_motion();
    press(cx, "tab menu enter");
    redraw(cx);
    assert_eq!(chosen.get(), 1);
    click(cx, "ctx/trigger");
    assert_eq!(chosen.get(), 1);
}

/// Item 37: at the smallest and the largest text size an overlay is still inside its window and
/// holds its parts: a popover's title and button, a dialog's title and actions, a toast's text.
#[gpui::test]
fn overlays_follow_the_text_size(cx: &mut TestAppContext) {
    use gpui::{Bounds, Pixels, point, px, size};
    let inside = |inner: Bounds<Pixels>, outer: Bounds<Pixels>| {
        inner.left() >= outer.left()
            && inner.right() <= outer.right()
            && inner.top() >= outer.top()
            && inner.bottom() <= outer.bottom()
    };
    for base in [9, 20] {
        let shell = format!("[font]\nbase-size = {base}\n");
        let cx = open(cx, |_, _| {
            v_flex()
                .child(
                    Popover::new("pop", "Filter", Button::new("t").label("Filter"))
                        .default_open(true)
                        .title("Filter results")
                        .content(|_, _| Button::new("apply").label("Apply")),
                )
                .child(
                    Dialog::new("dlg", "Rename file")
                        .open(true)
                        .description("Choose a new name.")
                        .action(|_, _| Button::new("ok").label("Rename")),
                )
        });
        reduce_motion();
        crate::theme::publish(std::sync::Arc::new(
            odk_theme::ThemeSnapshot::from_toml(crate::theme::TEST_COLORS, Some(&shell)).unwrap(),
        ));
        cx.simulate_resize(size(px(480.0), px(480.0)));
        let t = Toast::new("Saved")
            .description("notes.md was saved.")
            .persistent();
        let id = t.id();
        cx.update(|window, cx| toast(window, cx, t));
        redraw(cx);
        let window = Bounds::new(point(px(0.0), px(0.0)), size(px(480.0), px(480.0)));
        let card = format!(
            "odk-toast/card-{}",
            format!("{id:?}")
                .trim_start_matches("ToastId(")
                .trim_end_matches(')')
        );
        for part in [
            "pop/popover",
            "pop/title",
            "apply/button",
            "dlg/panel",
            "dlg/title",
            "ok/button",
            Box::leak(card.into_boxed_str()),
        ] {
            let b = cx
                .debug_bounds(part)
                .unwrap_or_else(|| panic!("{part} at {base} px"));
            assert!(inside(b, window), "{part} at {base} px: {b:?}");
        }
        let (panel, ok) = (
            cx.debug_bounds("dlg/panel").unwrap(),
            cx.debug_bounds("ok/button").unwrap(),
        );
        assert!(
            inside(ok, panel),
            "the action is in its dialog at {base} px"
        );
    }
}
