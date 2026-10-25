//! Decorations and truncation (ADR 0057, ADR 0058).

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, px, size};
use odk_a11y::Describe;

use super::{open, press, redraw};
use crate::input::drawn::ellipsis;
use crate::{
    Autosize, Button, Dialog, DropdownMenu, EndEllipsis, FixedWidth, Input, MenuItem, Select, Text,
    Wrap, v_flex,
};

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

/// A bounded button shows the full label in a tooltip on hover and on focus, and only when the
/// label does not fit. The tooltip adds no tab stop.
#[gpui::test]
fn a_truncated_button_explains_itself_on_hover_and_focus(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().w(px(80.0)).child(
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
        v_flex()
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
        v_flex()
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
        v_flex().w(px(120.0)).child(
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
    let cx = open(cx, |_, _| v_flex());
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
        v_flex()
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
