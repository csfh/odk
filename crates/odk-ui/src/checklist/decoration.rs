//! Decorations and truncation (ADR 0057, ADR 0058).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, px, size};
use odk_a11y::Describe;

use super::{open, press, redraw};
use crate::input::drawn::ellipsis;
use crate::{
    Autosize, Breadcrumb, Button, Command, CommandItem, Dialog, DropdownMenu, EndEllipsis,
    FixedWidth, Input, MenuItem, Select, Stack, Status, StatusIndicator, Text, ToggleGroup, Wrap,
};

/// A label long enough to ellipsize in a narrow menu, crumb, status or segment.
const LONG: &str = "Pneumonoultramicroscopicsilicovolcanoconiosis und Bildschirmprojektoren";

fn tip(cx: &mut gpui::VisualTestContext, name: &'static str) -> bool {
    cx.debug_bounds(name).is_some()
}

fn centre(cx: &mut gpui::VisualTestContext, part: &'static str) -> gpui::Point<gpui::Pixels> {
    let b = cx.debug_bounds(part).unwrap();
    gpui::point(
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    )
}

/// Rest the pointer on `part` until a tooltip would have opened.
fn hover(cx: &mut gpui::VisualTestContext, part: &'static str) {
    let motion = crate::foundation().motion.clone();
    let at = centre(cx, part);
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.executor().advance_clock(motion.hover_open);
    redraw(cx);
}

/// A bounded button shows the full label in a tooltip on hover and on focus, and only when the
/// label does not fit. The tooltip adds no tab stop.
#[gpui::test]
fn a_truncated_button_explains_itself_on_hover_and_focus(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new().w(px(80.0)).child(
            Button::new("b")
                .label("Synchronisierung ausstehend")
                .full_width(),
        )
    });
    redraw(cx);
    assert!(
        !tip(cx, "b/clip/tip"),
        "nothing has hovered or focused it yet"
    );
    let motion = crate::foundation().motion.clone();
    let at = centre(cx, "b/button");
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.executor().advance_clock(motion.hover_open);
    redraw(cx);
    assert!(tip(cx, "b/clip/tip"), "hover shows the full label");
    press(cx, "tab");
    redraw(cx);
    assert!(
        tip(cx, "b/clip/tip"),
        "focus shows the full label, and the button was the only tab stop"
    );
    let name = Button::new("b")
        .label("Synchronisierung ausstehend")
        .accessible();
    assert_eq!(name.name, "Synchronisierung ausstehend");
}

/// A label that fits has no tooltip, on hover or on focus.
#[gpui::test]
fn an_untruncated_button_has_no_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new()
            .w(px(400.0))
            .child(Button::new("b").label("Save").full_width())
    });
    redraw(cx);
    let motion = crate::foundation().motion.clone();
    let at = centre(cx, "b/button");
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.executor().advance_clock(motion.hover_open);
    redraw(cx);
    assert!(
        !tip(cx, "b/clip/tip"),
        "a label that fits has no hover tooltip"
    );
    press(cx, "tab");
    redraw(cx);
    assert!(!tip(cx, "b/clip/tip"));
}

/// An unfocused one-line value ends in `…`; focusing it scrolls instead.
#[gpui::test]
fn an_unfocused_input_ends_in_an_ellipsis(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new()
            .w(px(80.0))
            .child(Input::new("n", "Name").value("Pneumonoultramicroscopicsilicovolcanoconiosis"))
    });
    redraw(cx);
    assert!(ellipsis("n"), "the unfocused value is ellipsized");
    let field = cx.debug_bounds("n/box").unwrap();
    press(cx, "tab");
    redraw(cx);
    assert!(!ellipsis("n"), "focus scrolls to the caret instead");
    let again = cx.debug_bounds("n/box").unwrap();
    assert_eq!(again.size.width, field.size.width);
}

/// A menu grows to its longest label, and stops at the window.
#[gpui::test]
fn a_menu_grows_to_its_longest_label_until_the_window(cx: &mut TestAppContext) {
    let label = "Pneumonoultramicroscopicsilicovolcanoconiosis und Bildschirmprojektoren";
    let cx = open(cx, move |_, _| {
        DropdownMenu::new("file", "File")
            .menu(move |menu| menu.item(MenuItem::new("Short")).item(MenuItem::new(label)))
    });
    cx.simulate_resize(size(px(900.0), px(500.0)));
    press(cx, "tab enter");
    redraw(cx);
    let menu = cx.debug_bounds("file/menu").unwrap();
    let trigger = cx.debug_bounds("file/button/button").unwrap();
    assert!(
        menu.size.width > trigger.size.width + px(40.0),
        "wider than the trigger: {menu:?} vs {trigger:?}"
    );
    cx.simulate_resize(size(px(280.0), px(400.0)));
    redraw(cx);
    let menu = cx.debug_bounds("file/menu").unwrap();
    assert!(
        menu.size.width <= px(280.0),
        "capped by the window: {menu:?}"
    );
}

/// A select's list does the same: at least the box, wider for a long option, never past the window.
#[gpui::test]
fn a_select_list_grows_to_its_longest_option(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new().w(px(120.0)).child(
            Select::new("sl", "Fruit")
                .item("a", "Apple")
                .item("b", "Pneumonoultramicroscopicsilicovolcanoconiosis"),
        )
    });
    cx.simulate_resize(size(px(800.0), px(500.0)));
    press(cx, "tab enter");
    redraw(cx);
    let list = cx.debug_bounds("sl/menu").unwrap();
    let trigger = cx.debug_bounds("sl/trigger").unwrap();
    assert!(list.size.width > trigger.size.width);
    assert!(list.right() <= px(800.0));
}

/// A dialog title wraps for three lines and then stops. Wrap lifts the cap. Autosize scales the
/// panel with the text size; the default width does not.
#[gpui::test]
fn a_dialog_title_stops_at_three_lines_and_autosize_scales(cx: &mut TestAppContext) {
    let title = "Satz. ".repeat(40);
    let mode = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let flag = mode.clone();
    let cx = open(cx, move |_, _| {
        let title = title.clone();
        let dialog = Dialog::new("d", title).open(true);
        match flag.get() {
            0 => dialog,
            1 => dialog.decorate(Wrap),
            2 => dialog.decorate(Autosize),
            _ => dialog.decorate(FixedWidth(600.0)),
        }
    });
    crate::theme::publish(std::sync::Arc::new(
        odk_theme::ThemeSnapshot::from_toml(
            crate::theme::TEST_COLORS,
            Some("[font]\nbase-size = 20\n"),
        )
        .unwrap(),
    ));
    cx.simulate_resize(size(px(1400.0), px(900.0)));
    redraw(cx);
    let capped = cx.debug_bounds("d/title").unwrap();
    let line = crate::foundation()
        .typography
        .style(odk_theme::foundation::TypeRole::Heading)
        .line_height;
    assert!(
        capped.size.height <= px(line * 3.0 + 4.0),
        "three lines at most: {capped:?} line {line}"
    );
    let fixed = cx.debug_bounds("d/panel").unwrap().size.width;
    assert!(
        (fixed - px(480.0)).abs() < px(2.0),
        "a dialog stays 480 px unless Autosize: {fixed:?}"
    );
    mode.set(1);
    redraw(cx);
    let wrapped = cx.debug_bounds("d/title").unwrap();
    assert!(
        wrapped.size.height > capped.size.height + px(line),
        "Wrap lifts the cap: {wrapped:?} vs {capped:?}"
    );
    mode.set(2);
    redraw(cx);
    let scaled = cx.debug_bounds("d/panel").unwrap().size.width;
    let expect = px(480.0 * 20.0 / 12.0);
    assert!(
        (scaled - expect).abs() < px(2.0),
        "Autosize follows the text size: {scaled:?} vs {expect:?}"
    );
    mode.set(3);
    redraw(cx);
    let set = cx.debug_bounds("d/panel").unwrap().size.width;
    assert!(
        (set - px(600.0)).abs() < px(2.0),
        "FixedWidth replaces the default: {set:?}"
    );
}

/// Middle ellipsis keeps both ends. The gallery and the command row opt in; this checks the
/// helper against a measured width on the test platform's font.
#[gpui::test]
fn middle_ellipsis_uses_the_measured_width(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Stack::new());
    let text = "https://wiki.omarchy.org/troubleshooting/backups/failed";
    let short = cx.update(|window, _| {
        let size = px(13.0);
        crate::middle_ellipsis(text, |sample| {
            crate::layout::text_width(window, &sample.to_owned().into(), size) <= px(80.0)
        })
    });
    assert!(short.contains('…'), "{short}");
    assert!(short.starts_with("https"), "{short}");
    assert!(short.ends_with("failed") || short.ends_with('d'), "{short}");
    assert_ne!(short, text);
}

/// EndEllipsis keeps a paragraph on one line. Describe still has the full string.
#[gpui::test]
fn end_ellipsis_keeps_text_on_one_line(cx: &mut TestAppContext) {
    let words = "Synchronisierung ausstehend für alle Fenster";
    let cx = open(cx, move |_, _| {
        Stack::new()
            .w(px(80.0))
            .child(Text::new(words).decorate(EndEllipsis))
    });
    redraw(cx);
    let text = cx.debug_bounds("odk-prose").unwrap();
    let line = crate::foundation()
        .typography
        .style(odk_theme::foundation::TypeRole::Body)
        .line_height;
    assert!(text.size.height <= px(line + 2.0), "one line: {text:?}");
    assert_eq!(Text::new(words).accessible().name, words);
}

/// A narrow menu: the long rows ellipsize. Hover opens the tooltip on the row under the
/// pointer; Down opens it on the highlighted row. The short row has none.
#[gpui::test]
fn a_truncated_menu_row_explains_itself_on_hover_and_highlight(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DropdownMenu::new("file", "File").menu(|menu| {
            menu.item(MenuItem::new("Short"))
                .item(MenuItem::new(LONG))
                .item(MenuItem::new(LONG))
        })
    });
    cx.simulate_resize(size(px(280.0), px(400.0)));
    press(cx, "tab enter");
    redraw(cx);
    assert!(!tip(cx, "file/row-1/clip/tip"), "not yet");
    assert!(!tip(cx, "file/row-2/clip/tip"), "not yet");
    hover(cx, "file/row-1");
    assert!(tip(cx, "file/row-1/clip/tip"), "hover shows the full label");
    press(cx, "down down");
    redraw(cx);
    assert!(
        tip(cx, "file/row-2/clip/tip"),
        "highlight shows the full label"
    );
}

/// Labels that fit grow the menu, and neither hover nor highlight invents a tooltip.
#[gpui::test]
fn an_untruncated_menu_row_has_no_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DropdownMenu::new("file", "File").menu(|menu| {
            menu.item(MenuItem::new("Short"))
                .item(MenuItem::new("Also"))
        })
    });
    cx.simulate_resize(size(px(900.0), px(500.0)));
    press(cx, "tab enter");
    redraw(cx);
    hover(cx, "file/row-0");
    assert!(!tip(cx, "file/row-0/clip/tip"));
    press(cx, "down");
    redraw(cx);
    assert!(!tip(cx, "file/row-1/clip/tip"));
}

/// A heading is not highlighted and is not a tab stop. A truncated one opens on hover; a
/// short one does not, and moving the highlight does not open either.
#[gpui::test]
fn a_truncated_menu_heading_explains_itself_on_hover(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DropdownMenu::new("file", "File").menu(|menu| {
            menu.heading(LONG)
                .item(MenuItem::new("Short"))
                .heading("Files")
        })
    });
    cx.simulate_resize(size(px(280.0), px(400.0)));
    press(cx, "tab enter");
    redraw(cx);
    press(cx, "down");
    redraw(cx);
    assert!(
        !tip(cx, "file/heading-0/clip/tip"),
        "the highlight is on the row, not the heading"
    );
    assert!(!tip(cx, "file/heading-2/clip/tip"));
    hover(cx, "file/heading-0");
    assert!(
        tip(cx, "file/heading-0/clip/tip"),
        "hover shows the heading"
    );
    assert!(!tip(cx, "file/heading-2/clip/tip"));
}

/// One Escape closes the menu while a truncation tooltip is showing. Down then opens it
/// again on the same first item, which is what a second Escape would not do.
#[gpui::test]
fn escape_still_closes_a_menu_whose_row_shows_a_tooltip(cx: &mut TestAppContext) {
    let picked = Rc::new(Cell::new(false));
    let flag = picked.clone();
    let cx = open(cx, move |_, _| {
        let flag = flag.clone();
        DropdownMenu::new("file", "File").menu(move |menu| {
            menu.item(MenuItem::new(LONG).on_select(move |_, _| flag.set(true)))
                .item(MenuItem::new("Short"))
        })
    });
    cx.simulate_resize(size(px(280.0), px(400.0)));
    press(cx, "tab enter");
    redraw(cx);
    assert!(
        tip(cx, "file/row-0/clip/tip"),
        "the highlighted row is truncated"
    );
    press(cx, "escape down enter");
    assert!(
        picked.get(),
        "Escape closed the menu: Down opened it again and Enter chose the first item"
    );
}

/// The highlighted command is not a tab stop. Down opens its tooltip; hovering another
/// truncated row opens that one; a short row has none.
#[gpui::test]
fn a_truncated_command_row_explains_itself_on_highlight_and_hover(cx: &mut TestAppContext) {
    let open_flag = Rc::new(Cell::new(false));
    let flag = open_flag.clone();
    let cx = open(cx, move |_, _| {
        let (flag, to_open) = (flag.clone(), flag.clone());
        Stack::new()
            .child(
                Button::new("open")
                    .label("Commands")
                    .on_click(move |_, window, _| {
                        to_open.set(true);
                        window.refresh();
                    }),
            )
            .child(
                Command::new("cmd", "Commands")
                    .open(flag.get())
                    .item(CommandItem::new(LONG))
                    .item(CommandItem::new(LONG))
                    .item(CommandItem::new("Open")),
            )
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    press(cx, "tab enter");
    redraw(cx);
    assert!(
        tip(cx, "cmd/row-0/clip/tip"),
        "the highlighted row shows the full label"
    );
    assert!(!tip(cx, "cmd/row-1/clip/tip"));
    hover(cx, "cmd/row-1");
    assert!(tip(cx, "cmd/row-1/clip/tip"), "hover shows the full label");
    hover(cx, "cmd/row-2");
    assert!(
        !tip(cx, "cmd/row-2/clip/tip"),
        "a label that fits has no tooltip"
    );
}

/// Short commands fit the palette. Hover and Down open no tooltip.
#[gpui::test]
fn an_untruncated_command_row_has_no_tooltip(cx: &mut TestAppContext) {
    let open_flag = Rc::new(Cell::new(false));
    let flag = open_flag.clone();
    let cx = open(cx, move |_, _| {
        let (flag, to_open) = (flag.clone(), flag.clone());
        Stack::new()
            .child(
                Button::new("open")
                    .label("Commands")
                    .on_click(move |_, window, _| {
                        to_open.set(true);
                        window.refresh();
                    }),
            )
            .child(
                Command::new("cmd", "Commands")
                    .open(flag.get())
                    .item(CommandItem::new("Open"))
                    .item(CommandItem::new("Save")),
            )
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    press(cx, "tab enter");
    redraw(cx);
    hover(cx, "cmd/row-0");
    assert!(!tip(cx, "cmd/row-0/clip/tip"));
    press(cx, "down");
    redraw(cx);
    assert!(!tip(cx, "cmd/row-1/clip/tip"));
}

/// Escape closes the palette while the highlighted row's tooltip is up. The field has focus,
/// so the key is the palette's, not the row's.
#[gpui::test]
fn escape_still_closes_a_palette_whose_row_shows_a_tooltip(cx: &mut TestAppContext) {
    let open_flag = Rc::new(Cell::new(true));
    let flag = open_flag.clone();
    let cx = open(cx, move |_, _| {
        let flag = flag.clone();
        Command::new("cmd", "Commands")
            .open(flag.get())
            .on_close({
                let flag = flag.clone();
                move |_, _| flag.set(false)
            })
            .item(CommandItem::new(LONG))
            .item(CommandItem::new("Open"))
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    redraw(cx);
    assert!(tip(cx, "cmd/row-0/clip/tip"));
    press(cx, "escape");
    assert!(!open_flag.get(), "Escape closed the palette");
}

/// The long crumb is a tab stop. Tab reaches it and the tooltip opens; it was closed before.
#[gpui::test]
fn a_truncated_breadcrumb_link_explains_itself_on_focus(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new().w(px(240.0)).child(
            Breadcrumb::new("b")
                .item("Go", |_, _| {})
                .item(LONG, |_, _| {})
                .current("Now"),
        )
    });
    redraw(cx);
    assert!(!tip(cx, "b/item-1/clip/tip"));
    press(cx, "tab tab");
    redraw(cx);
    assert!(
        tip(cx, "b/item-1/clip/tip"),
        "focus on the link shows the full crumb"
    );
}

/// Hover opens the same tooltip without moving focus onto the crumb.
#[gpui::test]
fn a_truncated_breadcrumb_link_explains_itself_on_hover(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new().w(px(240.0)).child(
            Breadcrumb::new("b")
                .item("Go", |_, _| {})
                .item(LONG, |_, _| {})
                .current("Now"),
        )
    });
    redraw(cx);
    hover(cx, "b/item-1");
    assert!(tip(cx, "b/item-1/clip/tip"), "hover shows the full crumb");
    assert!(!tip(cx, "b/item-0/clip/tip"));
}

/// The current page is not a tab stop. Tab skips it, and its tooltip opens on hover only.
#[gpui::test]
fn a_truncated_current_page_is_hover_only(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0u32));
    let seen = clicks.clone();
    let cx = open(cx, move |_, _| {
        let seen = seen.clone();
        Stack::new()
            .w(px(200.0))
            .child(Breadcrumb::new("b").item("Go", |_, _| {}).current(LONG))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| seen.set(seen.get() + 1)),
            )
    });
    redraw(cx);
    assert!(!tip(cx, "b/item-1/clip/tip"));
    press(cx, "tab tab enter");
    assert_eq!(clicks.get(), 1, "the current page is not a tab stop");
    assert!(!tip(cx, "b/item-1/clip/tip"), "Tab did not open it");
    hover(cx, "b/item-1");
    assert!(tip(cx, "b/item-1/clip/tip"), "hover shows the current page");
}

/// Crumbs that fit have no tooltip, on hover or on focus.
#[gpui::test]
fn an_untruncated_breadcrumb_has_no_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new()
            .w(px(600.0))
            .child(Breadcrumb::new("b").item("Home", |_, _| {}).current("Page"))
    });
    redraw(cx);
    hover(cx, "b/item-0");
    assert!(!tip(cx, "b/item-0/clip/tip"));
    hover(cx, "b/item-1");
    assert!(!tip(cx, "b/item-1/clip/tip"));
    press(cx, "tab");
    redraw(cx);
    assert!(!tip(cx, "b/item-0/clip/tip"));
}

/// A status is not a tab stop. Tab skips it; hover shows the full label only when it is cut.
#[gpui::test]
fn a_truncated_status_explains_itself_on_hover_only(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0u32));
    let seen = clicks.clone();
    let cx = open(cx, move |_, _| {
        let seen = seen.clone();
        Stack::new()
            .w(px(80.0))
            .child(StatusIndicator::new(Status::Active, LONG))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| seen.set(seen.get() + 1)),
            )
    });
    redraw(cx);
    assert!(!tip(
        cx,
        "odk-status-Pneumonoultramicroscopicsilicovolcanoconiosis und Bildschirmprojektoren/clip/tip"
    ));
    press(cx, "tab enter");
    assert_eq!(clicks.get(), 1, "a status is not a tab stop");
    assert!(!tip(
        cx,
        "odk-status-Pneumonoultramicroscopicsilicovolcanoconiosis und Bildschirmprojektoren/clip/tip"
    ));
    hover(cx, "odk-status");
    assert!(tip(
        cx,
        "odk-status-Pneumonoultramicroscopicsilicovolcanoconiosis und Bildschirmprojektoren/clip/tip"
    ));
}

#[gpui::test]
fn an_untruncated_status_has_no_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new()
            .w(px(400.0))
            .child(StatusIndicator::new(Status::Active, "Online"))
    });
    redraw(cx);
    hover(cx, "odk-status");
    assert!(!tip(cx, "odk-status-Online/clip/tip"));
}

/// Both segments ellipsize. Hover opens the one under the pointer; Tab opens the focused one.
#[gpui::test]
fn a_truncated_segment_explains_itself_on_hover_and_focus(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new().w(px(180.0)).child(
            ToggleGroup::new("seg", "View")
                .item("a", LONG)
                .item("b", LONG),
        )
    });
    redraw(cx);
    assert!(!tip(cx, "seg/item-0/clip/tip"));
    assert!(!tip(cx, "seg/item-1/clip/tip"));
    hover(cx, "seg/item-1");
    assert!(tip(cx, "seg/item-1/clip/tip"), "hover shows the segment");
    assert!(!tip(cx, "seg/item-0/clip/tip"));
    press(cx, "tab");
    redraw(cx);
    assert!(tip(cx, "seg/item-0/clip/tip"), "focus shows the segment");
}

/// Escape dismisses the segment's tooltip and leaves the segment focused, so the next Tab
/// leaves the group. The group does not close.
#[gpui::test]
fn escape_dismisses_a_segment_tooltip_and_the_segment_stays(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0u32));
    let seen = clicks.clone();
    let cx = open(cx, move |_, _| {
        let seen = seen.clone();
        Stack::new()
            .w(px(120.0))
            .child(
                ToggleGroup::new("seg", "View")
                    .item("a", LONG)
                    .item("b", "Ok"),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| seen.set(seen.get() + 1)),
            )
    });
    super::reduce_motion();
    press(cx, "tab");
    redraw(cx);
    assert!(tip(cx, "seg/item-0/clip/tip"));
    press(cx, "escape");
    redraw(cx);
    assert_eq!(
        crate::overlay::probe::last("item-0"),
        None,
        "Escape dismissed the tooltip"
    );
    press(cx, "tab enter");
    assert_eq!(
        clicks.get(),
        1,
        "one Tab left the group; the segment stayed focused"
    );
}

/// Segments that fit have no tooltip, on hover or on focus.
#[gpui::test]
fn an_untruncated_segment_has_no_tooltip(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new().w(px(400.0)).child(
            ToggleGroup::new("seg", "View")
                .item("a", "Left")
                .item("b", "Right"),
        )
    });
    redraw(cx);
    hover(cx, "seg/item-0");
    assert!(!tip(cx, "seg/item-0/clip/tip"));
    press(cx, "tab");
    redraw(cx);
    assert!(!tip(cx, "seg/item-0/clip/tip"));
    assert!(!tip(cx, "seg/item-1/clip/tip"));
}
