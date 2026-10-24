//! Item 23 with the worst realistic content, in small windows: nothing runs out of its box or
//! out of the window, and no button is pushed out of reach. The gallery's "Worst case" switch
//! shows the same content (`examples/gallery/src/content.rs`).

use gpui::{Bounds, ParentElement, Pixels, Styled, TestAppContext, VisualTestContext, px, size};

use super::{open, press, redraw, reduce_motion};
use crate::{
    Alert, Button, Dialog, DropdownMenu, HoverCard, MenuItem, Select, Sheet, Size, Text, Toast,
    Tone, toast, v_flex,
};

const URL: &str =
    "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed";

fn within(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.left() >= outer.left()
        && inner.right() <= outer.right()
        && inner.top() >= outer.top()
        && inner.bottom() <= outer.bottom()
}

fn bounds(cx: &mut VisualTestContext, part: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("no part named {part}"))
}

/// A window `width` wide, as the whole screen.
fn shrink_window(cx: &mut VisualTestContext, width: f32) -> Bounds<Pixels> {
    cx.simulate_resize(size(px(width), px(640.0)));
    redraw(cx);
    let viewport = cx.update(|window, _| window.viewport_size());
    Bounds::new(gpui::point(px(0.0), px(0.0)), viewport)
}

#[gpui::test]
fn an_alert_wraps_a_long_url_and_keeps_its_action(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().child(
            gpui::div().w(px(300.0)).child(
                Alert::new("a", "Datensicherungsüberprüfungsprotokoll wurde erstellt")
                    .variant(Tone::Warning)
                    .description(URL)
                    .action(
                        Button::new("act")
                            .label("Speicherplatz freigeben")
                            .size(Size::Sm),
                    )
                    .on_dismiss(|_, _, _| {}),
            ),
        )
    });
    let alert = bounds(cx, "a/alert");
    assert!(
        alert.size.width <= px(300.0),
        "the alert fits: {:?}",
        alert.size
    );
    assert!(
        within(bounds(cx, "a/text"), alert),
        "the text stays in the alert"
    );
    assert!(
        within(bounds(cx, "act/button"), alert),
        "the action stays in reach"
    );
    assert!(
        within(bounds(cx, "a/dismiss/button"), alert),
        "so does dismiss"
    );
}

#[gpui::test]
fn a_sheet_fits_a_small_window_and_keeps_its_close_button(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Sheet::new(
            "s",
            "Anzeigeeinstellungen für externe Bildschirme und Projektoren",
        )
        .open(true)
        .content(|_, _| Text::new(URL))
    });
    reduce_motion();
    let screen = shrink_window(cx, 320.0);
    let panel = bounds(cx, "s/panel");
    assert!(
        within(panel, screen),
        "the panel fits the window: {panel:?}"
    );
    let close = bounds(cx, "s/close/button");
    assert!(within(close, panel), "the close button stays in the panel");
    assert!(
        bounds(cx, "s/title").right() <= close.left(),
        "the title wraps beside it"
    );
}

#[gpui::test]
fn a_dialog_fits_a_small_window_and_keeps_its_actions_in_reach(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Dialog::new(
            "d",
            "Anzeigeeinstellungen für externe Bildschirme und Projektoren",
        )
        .open(true)
        .content(|_, _| Text::new(URL))
        .action(|_, _| Button::new("cancel").label("Abbrechen"))
        .action(|_, _| Button::new("save").label("Übernehmen und schließen"))
    });
    reduce_motion();
    let screen = shrink_window(cx, 320.0);
    let panel = bounds(cx, "d/panel");
    assert!(
        within(panel, screen),
        "the panel fits the window: {panel:?}"
    );
    let close = bounds(cx, "d/close/button");
    assert!(within(close, panel), "the close button stays in the panel");
    assert!(
        bounds(cx, "d/title").right() <= close.left(),
        "the title wraps beside it"
    );
    for action in ["cancel/button", "save/button"] {
        let b = bounds(cx, action);
        assert!(within(b, panel), "{action} is out of reach: {b:?}");
    }
}

#[gpui::test]
fn a_menu_is_never_wider_than_the_window(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        DropdownMenu::new("file", "File").menu(|m| {
            m.item(
                MenuItem::new(
                    "Neues Dokument aus einer gespeicherten Vorlage erstellen und öffnen",
                )
                .hint("Ctrl+Alt+Shift+N"),
            )
            .item(MenuItem::new("Öffnen…").hint("Strg+O"))
        })
    });
    reduce_motion();
    let screen = shrink_window(cx, 320.0);
    press(cx, "tab enter");
    redraw(cx);
    let menu = bounds(cx, "file/menu");
    assert!(within(menu, screen), "the menu fits the window: {menu:?}");
    let row = bounds(cx, "file/row-0");
    assert!(within(row, menu));
    assert!(
        bounds(cx, "file/label-0").right() <= row.right(),
        "the label gives way to the hint"
    );
}

/// A select with long, unbroken and non-Latin labels in a small window: the box and the list
/// stay inside the window, and a label gives way to the box with `…`.
#[gpui::test]
fn a_select_is_never_wider_than_the_window(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().child(
            Select::new("sl", "Datei")
                .default_value("long")
                .item(
                    "long",
                    "Datensicherungsüberprüfungsprotokoll-Archivierungsverzeichnis",
                )
                .item("url", URL)
                .item("ja", "日本語の長いラベルは折り返さずに省略記号で終わります")
                .item("emoji", "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉"),
        )
    });
    reduce_motion();
    let screen = shrink_window(cx, 320.0);
    let trigger = bounds(cx, "sl/trigger");
    assert!(
        within(trigger, screen),
        "the box fits the window: {trigger:?}"
    );
    assert!(
        bounds(cx, "sl/value").right() <= trigger.right(),
        "the label gives way to the arrow"
    );
    press(cx, "tab enter");
    redraw(cx);
    let list = bounds(cx, "sl/menu");
    assert!(within(list, screen), "the list fits the window: {list:?}");
    for row in ["sl/row-0", "sl/row-1", "sl/row-2", "sl/row-3"] {
        assert!(within(bounds(cx, row), list), "{row} stays in the list");
    }
    assert!(bounds(cx, "sl/label-1").right() <= bounds(cx, "sl/row-1").right());
}

#[gpui::test]
fn toasts_fit_a_small_window(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| Text::new(""));
    reduce_motion();
    let screen = shrink_window(cx, 300.0);
    cx.update(|window, cx| {
        toast(
            window,
            cx,
            Toast::new("Die Datensicherung wurde erfolgreich abgeschlossen 🎉")
                .variant(Tone::Danger)
                .description(URL)
                .action("Erneut versuchen", |_, _| {}),
        )
    });
    redraw(cx);
    let card = bounds(cx, "odk-toast/card-1");
    assert!(within(card, screen), "the toast fits the window: {card:?}");
}

#[gpui::test]
fn a_hover_card_wraps_a_long_address(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        HoverCard::new("h", "About the account", Text::new("@aleksandra")).content(|_, _| {
            Text::new("bartholomew.fitzgerald@northwind-industries-holdings.example.com")
        })
    });
    reduce_motion();
    let screen = shrink_window(cx, 320.0);
    press(cx, "tab");
    redraw(cx);
    let card = bounds(cx, "h/card");
    assert!(within(card, screen), "the card fits the window: {card:?}");
}

/// A button never gives up width, so its label is never cut: in a row too narrow for its
/// buttons, each keeps the width it has when there is room.
#[gpui::test]
fn a_button_keeps_its_label_in_a_narrow_row(cx: &mut TestAppContext) {
    use std::cell::Cell;
    use std::rc::Rc;
    let width = Rc::new(Cell::new(2000.0));
    let w = width.clone();
    let cx = open(cx, move |_, _| {
        v_flex().child(
            crate::h_flex()
                .w(px(w.get()))
                .child(Button::new("a").label("Änderungen speichern und fortfahren"))
                .child(Button::new("b").label("Auf Werkseinstellungen zurücksetzen")),
        )
    });
    redraw(cx);
    let natural = (bounds(cx, "a/button").size, bounds(cx, "b/button").size);
    width.set(200.0);
    redraw(cx);
    assert_eq!(
        (bounds(cx, "a/button").size, bounds(cx, "b/button").size),
        natural
    );
}

/// A sidebar in a narrow column: a long label ends in `…` before its counter, the counter stays
/// in its row, and no row leaves the list.
#[gpui::test]
fn a_nav_list_truncates_long_rows_and_keeps_counters(cx: &mut TestAppContext) {
    use crate::NavList;
    let cx = open(cx, |_, _| {
        v_flex().child(
            gpui::div().w(px(160.0)).child(
                NavList::new("n", "Einstellungen")
                    .group("Arbeitsbereich und gemeinsame Einstellungen")
                    .item("m", "Mitglieder und Berechtigungen der Organisation")
                    .icon("m", "☺")
                    .counter("m", "1.284")
                    .item("p", "P"),
            ),
        )
    });
    let list = bounds(cx, "n/list");
    assert!(list.size.width <= px(160.0), "{:?}", list.size);
    let row = bounds(cx, "n/item-0");
    assert!(within(row, list), "the row stays in the list");
    let (label, counter) = (bounds(cx, "n/label-0"), bounds(cx, "n/counter-0"));
    assert!(within(counter, row), "the counter stays in its row");
    assert!(label.right() <= counter.left(), "the label gives way to it");
    assert!(within(bounds(cx, "n/group-0"), list), "so does the heading");
}

/// An empty navigation list, tab list or segmented control draws and takes no tab stop; one
/// with a single item works.
#[gpui::test]
fn empty_and_single_navigation_works(cx: &mut TestAppContext) {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::Count;
    use crate::{NavList, Tabs, ToggleGroup};
    let (after, opened) = (
        Count::default(),
        Rc::new(RefCell::new(Vec::<String>::new())),
    );
    let (a, o) = (after.clone(), opened.clone());
    let cx = open(cx, move |_, _| {
        let (a, o) = (a.clone(), o.clone());
        v_flex()
            .child(NavList::new("empty", "Nothing"))
            .child(Tabs::new("no-tabs", "Nothing"))
            .child(ToggleGroup::new("no-options", "Nothing"))
            .child(
                NavList::new("one", "One")
                    .item("only", "Only")
                    .on_change(move |v, _, _| o.borrow_mut().push(v.to_string())),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab down enter");
    assert_eq!(*opened.borrow(), ["only"], "Tab went past the empty ones");
    press(cx, "tab enter");
    assert_eq!(after.get(), 1);
}

/// A progress bar in a narrow column with a long name and its value: the name ends in `…`, the
/// value keeps its place, and the bar fits.
#[gpui::test]
fn a_progress_bar_keeps_its_value_in_a_narrow_column(cx: &mut TestAppContext) {
    use crate::Progress;
    let cx = open(cx, |_, _| {
        v_flex().child(
            gpui::div().w(px(160.0)).child(
                Progress::new("p", URL)
                    .value(3.0)
                    .max(7.0)
                    .value_text("3 von 7 Dateien")
                    .show_label(true)
                    .show_value(true),
            ),
        )
    });
    let all = bounds(cx, "p/progress");
    assert!(all.size.width <= px(160.0), "{:?}", all.size);
    for part in ["p/line", "p/value", "p/track"] {
        assert!(within(bounds(cx, part), all), "{part} stays inside");
    }
}

/// A status indicator with a long label in a narrow column truncates and stays inside it.
#[gpui::test]
fn a_status_indicator_truncates_a_long_label(cx: &mut TestAppContext) {
    use crate::{Status, StatusIndicator};
    let cx = open(cx, |_, _| {
        v_flex().child(gpui::div().w(px(140.0)).child(StatusIndicator::new(
            Status::Warning,
            "Eingeschränkt verfügbar wegen Wartungsarbeiten 🔥",
        )))
    });
    let row = bounds(cx, "odk-status");
    assert!(row.size.width <= px(140.0), "{:?}", row.size);
    assert!(within(bounds(cx, "odk-status/dot"), row));
}
