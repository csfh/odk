use std::cell::RefCell;
use std::rc::Rc;

use gpui::{ParentElement, TestAppContext};
use odk_a11y::Role;

use super::{Count, open, press, reduce_motion};
use crate::overlay::probe;
use crate::{Button, ContextMenu, DropdownMenu, Menu, MenuItem, Text, h_flex};

type Picks = Rc<RefCell<Vec<&'static str>>>;

fn menu(picks: &Picks) -> impl FnOnce(Menu) -> Menu + 'static {
    let picks = picks.clone();
    move |m| {
        let pick = |name: &'static str| {
            let picks = picks.clone();
            move |_: &mut gpui::Window, _: &mut gpui::App| picks.borrow_mut().push(name)
        };
        m.item(MenuItem::new("New").hint("Ctrl+N").on_select(pick("new")))
            .item(
                MenuItem::new("Print")
                    .disabled(true)
                    .on_select(pick("print")),
            )
            .separator()
            .item(MenuItem::new("Delete").danger().on_select(pick("delete")))
    }
}

/// Items 8, 10, 13: Enter opens the menu with focus on its first item, Down skips the
/// separator and the disabled item, Enter chooses and closes, and focus goes back to the
/// button, so Enter opens it again.
#[gpui::test]
fn dropdown_from_the_keyboard(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        DropdownMenu::new("file", "File").menu(menu(&p))
    });
    press(cx, "tab enter down enter");
    assert_eq!(*picks.borrow(), ["delete"]);
    press(cx, "enter enter");
    assert_eq!(
        *picks.borrow(),
        ["delete", "new"],
        "focus came back to the button"
    );
    press(cx, "enter up up enter");
    assert_eq!(
        *picks.borrow(),
        ["delete", "new", "new"],
        "Up wraps past the disabled item"
    );
}

/// Item 10, 13: Escape closes the menu without choosing, and focus goes back.
#[gpui::test]
fn escape_closes_the_menu(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        DropdownMenu::new("file", "File").menu(menu(&p))
    });
    press(cx, "tab enter escape");
    assert!(picks.borrow().is_empty());
    // Closed, with focus on the button: Down opens it on New. Still open, Down would move
    // to Delete.
    press(cx, "down enter");
    assert_eq!(*picks.borrow(), ["new"]);
}

/// Item 13: Tab closes an open menu instead of leaving it open behind.
#[gpui::test]
fn tab_closes_the_menu(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let after = Count::default();
    let (p, a) = (picks.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        h_flex()
            .child(DropdownMenu::new("file", "File").menu(menu(&p)))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab enter tab");
    assert!(picks.borrow().is_empty(), "Tab chose nothing");
    // Closed, with focus back on the button: Down opens it again on New.
    press(cx, "down enter");
    assert_eq!(*picks.borrow(), ["new"]);
    assert_eq!(after.get(), 0, "Tab only closed the menu");
}

/// Items 8, 9: the Menu key and Shift+F10 open a context menu on its focused trigger.
#[gpui::test]
fn context_menu_from_the_keyboard(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        ContextMenu::new("ctx", "Row actions", Text::new("Row")).menu(menu(&p))
    });
    press(cx, "tab menu enter");
    press(cx, "shift-f10 down enter");
    assert_eq!(*picks.borrow(), ["new", "delete"]);
}

/// Item 15: a context menu is named by its constructor, and a dropdown by its label.
#[test]
fn menus_are_named() {
    use odk_a11y::Describe;
    let ctx = ContextMenu::new("ctx", "Row actions", Text::new("Row")).accessible();
    assert_eq!((ctx.role, ctx.name.as_str()), (Role::Menu, "Row actions"));
    assert_eq!(DropdownMenu::new("file", "File").accessible().name, "File");
}

/// Items 14, 16, 17: the menu and its items report their role and state.
#[test]
fn reports_the_menu_and_its_items() {
    let picks = Picks::default();
    let a = menu(&picks)(Menu::new()).accessibles("File");
    assert_eq!((a[0].role, a[0].name.as_str()), (Role::Menu, "File"));
    assert_eq!((a[1].role, a[1].name.as_str()), (Role::MenuItem, "New"));
    assert!(a[2].disabled);
    assert_eq!(a[3].role, Role::Separator);
    let checkable = Menu::new()
        .item(MenuItem::new("Wrap").checked(true))
        .accessibles("File");
    assert_eq!(
        (checkable[1].role, checkable[1].selected),
        (Role::MenuItemCheckbox, Some(true))
    );
}

/// Item 32: moving through an open menu, by keyboard and pointer, moves nothing in it, and
/// opening it moves nothing around it.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::reduce_motion;
    use super::stability::{Step, assert_still};
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        h_flex()
            .child(DropdownMenu::new("file", "File").menu(menu(&p)))
            .child(Button::new("after").label("After"))
    });
    reduce_motion();
    assert_still(
        cx,
        &["file/button/button", "after/button"],
        &[Step::Keys("tab enter")],
    );
    assert_still(
        cx,
        &[
            "file/menu",
            "file/row-0",
            "file/row-1",
            "file/row-3",
            "file/label-0",
            "file/label-1",
            "file/label-3",
            "file/button/button",
            "after/button",
        ],
        &[
            Step::Keys("down"),
            Step::Keys("down"),
            Step::Hover("file/row-0"),
            Step::Hover("file/row-1"),
            Step::Hover("file/row-3"),
        ],
    );
}

/// From the keyboard, a context menu opens right below its trigger.
#[gpui::test]
fn a_context_menu_opens_right_below_its_trigger(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        crate::v_flex()
            .child(Text::new("Above"))
            .child(ContextMenu::new("ctx", "Row actions", Text::new("Row")).menu(menu(&p)))
    });
    super::reduce_motion();
    press(cx, "tab menu");
    super::redraw(cx);
    let (trigger, list) = (
        cx.debug_bounds("ctx/trigger").unwrap(),
        cx.debug_bounds("ctx/menu").unwrap(),
    );
    assert_eq!(list.top(), trigger.bottom(), "{list:?} below {trigger:?}");
    assert_eq!(list.left(), trigger.left());
}

/// A second click on the button of an open menu closes it. A frame is drawn between the press
/// and the release, so the button must toggle on the press, not on the click.
#[gpui::test]
fn a_second_click_on_the_button_closes_the_menu(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        DropdownMenu::new("dd", "File").menu(menu(&p))
    });
    reduce_motion();
    click(cx, "dd/button/button");
    assert_eq!(probe::last("dd"), Some(1.0), "a click opens the menu");
    click(cx, "dd/button/button");
    assert_eq!(probe::last("dd"), None, "the next click closes it");
    click(cx, "dd/button/button");
    assert_eq!(
        probe::last("dd"),
        Some(1.0),
        "and the one after opens it again"
    );
    press(cx, "escape enter");
    assert_eq!(
        probe::last("dd"),
        Some(1.0),
        "Enter on the button opens it by keyboard"
    );
    assert!(picks.borrow().is_empty());
}

/// Layer order outside a modal is unchanged: a menu still closing when its item opens a dialog
/// is under the dialog's scrim, so a press where it was takes no item.
#[gpui::test]
fn a_menu_closing_under_a_dialog_takes_no_input(cx: &mut TestAppContext) {
    use std::cell::Cell;
    let picks = Picks::default();
    let shown = Rc::new(Cell::new(false));
    let (p, s) = (picks.clone(), shown.clone());
    let cx = open(cx, move |_, _| {
        let (p, s) = (p.clone(), s.clone());
        let open_dialog = s.clone();
        gpui::div()
            .child(DropdownMenu::new("dd", "File").menu(move |m| {
                let (p, open_dialog) = (p.clone(), open_dialog.clone());
                m.item(MenuItem::new("New").on_select(move |_, _| {
                    p.borrow_mut().push("new");
                    open_dialog.set(true);
                }))
            }))
            .child(crate::Dialog::new("dlg", "Created").open(s.get()))
    });
    // Full motion: the menu is fully shown, then takes a moment to fade out.
    click(cx, "dd/button/button");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    super::redraw(cx);
    assert_eq!(probe::last("dd"), Some(1.0));
    click(cx, "dd/row-0");
    assert_eq!(*picks.borrow(), ["new"]);
    assert!(
        probe::last("dd").is_some(),
        "the menu is still drawn, fading out"
    );
    cx.update(|window, _| window.refresh());
    let row = cx.debug_bounds("dd/row-0").unwrap().center();
    cx.simulate_click(row, gpui::Modifiers::none());
    assert_eq!(
        *picks.borrow(),
        ["new"],
        "the closing menu took no second pick"
    );
}

/// Item 33: a dropdown opened inside a dialog is drawn above it, takes the click on an item, and
/// closes on Escape before the dialog does.
#[gpui::test]
fn a_dropdown_in_a_dialog(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let picks = Picks::default();
    let closed = Count::default();
    let p = picks.clone();
    let cx = dialog_around(cx, closed.clone(), move || {
        DropdownMenu::new("dd", "File").menu(menu(&p))
    });
    click(cx, "dd/button/button");
    click(cx, "dd/row-0");
    assert_eq!(*picks.borrow(), ["new"], "the click reached the item");
    assert_eq!(closed.get(), 0, "and not the scrim behind it");
    click(cx, "dd/button/button");
    press(cx, "escape");
    assert_eq!(closed.get(), 0, "Escape closed only the menu");
}

/// Item 33: the same for a context menu opened from the keyboard.
#[gpui::test]
fn a_context_menu_in_a_dialog(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let picks = Picks::default();
    let closed = Count::default();
    let p = picks.clone();
    let cx = dialog_around(cx, closed.clone(), move || {
        ContextMenu::new("ctx", "Row actions", Text::new("Row")).menu(menu(&p))
    });
    click(cx, "ctx/trigger");
    press(cx, "menu enter");
    assert_eq!(*picks.borrow(), ["new"]);
    assert_eq!(closed.get(), 0);
}

fn click(cx: &mut gpui::VisualTestContext, part: &'static str) {
    let at = cx.debug_bounds(part).unwrap().center();
    cx.simulate_click(at, gpui::Modifiers::none());
    super::redraw(cx);
}

/// The press that dismisses a menu is the menu's: it closes it and goes no further, so a click
/// away from an open menu does not also press the control it landed on (docs/spec/odk-overlays.md,
/// "Dismissal").
#[gpui::test]
fn the_click_that_dismisses_a_menu_presses_nothing_under_it(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let (p, other) = (picks.clone(), Count::default());
    let o = other.clone();
    let cx = open(cx, move |_, _| {
        let o = o.clone();
        h_flex()
            .child(DropdownMenu::new("dd", "File").menu(menu(&p)))
            .child(
                Button::new("other")
                    .label("Other")
                    .on_click(move |_, _, _| o.bump()),
            )
    });
    reduce_motion();
    click(cx, "dd/button/button");
    assert_eq!(probe::last("dd"), Some(1.0));
    click(cx, "other/button");
    assert_eq!(probe::last("dd"), None, "the click closed the menu");
    assert_eq!(other.get(), 0, "and went no further");
    click(cx, "other/button");
    assert_eq!(
        other.get(),
        1,
        "with the menu closed the button works again"
    );
}

/// A right click away from an open context menu closes it and opens nothing.
#[gpui::test]
fn a_right_click_away_from_a_context_menu_closes_it(cx: &mut TestAppContext) {
    use gpui::{Modifiers, MouseButton, point, px};
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        h_flex().child(
            ContextMenu::new("ctx", "Row actions", Text::new("Right click me")).menu(menu(&p)),
        )
    });
    reduce_motion();
    let trigger = cx.debug_bounds("ctx/trigger").unwrap();
    cx.simulate_mouse_down(trigger.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(trigger.center(), MouseButton::Right, Modifiers::default());
    super::redraw(cx);
    assert_eq!(probe::last("ctx"), Some(1.0));
    let away = point(px(700.0), px(500.0));
    cx.simulate_mouse_down(away, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(away, MouseButton::Right, Modifiers::default());
    super::redraw(cx);
    assert_eq!(probe::last("ctx"), None);
}

/// Typing finds an item by the first letters of its label and moves the highlight there; a
/// heading and a disabled item are never found.
#[gpui::test]
fn typing_finds_an_item(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        let p = p.clone();
        DropdownMenu::new("file", "File").menu(move |m| {
            let pick = |name: &'static str| {
                let p = p.clone();
                move |_: &mut gpui::Window, _: &mut gpui::App| p.borrow_mut().push(name)
            };
            m.heading("Recent")
                .item(MenuItem::new("Alpha").on_select(pick("alpha")))
                .item(MenuItem::new("Beta").disabled(true).on_select(pick("beta")))
                .item(MenuItem::new("Bravo").on_select(pick("bravo")))
        })
    });
    reduce_motion();
    press(cx, "tab enter b enter");
    assert_eq!(*picks.borrow(), ["bravo"], "B skipped the disabled Beta");
}

/// A heading is a label, not an item: the keyboard skips it and it reports as a group; a radio
/// item reports as one and shows its dot only when chosen.
#[gpui::test]
fn headings_are_skipped_and_radios_say_which_is_chosen(cx: &mut TestAppContext) {
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        let p = p.clone();
        DropdownMenu::new("view", "View").menu(move |m| {
            let pick = |name: &'static str| {
                let p = p.clone();
                move |_: &mut gpui::Window, _: &mut gpui::App| p.borrow_mut().push(name)
            };
            m.heading("Density")
                .item(
                    MenuItem::new("Compact")
                        .radio(false)
                        .on_select(pick("compact")),
                )
                .item(
                    MenuItem::new("Default")
                        .radio(true)
                        .on_select(pick("default")),
                )
                .separator()
                .heading("Panels")
                .item(
                    MenuItem::new("Sidebar")
                        .icon("▤")
                        .checked(true)
                        .on_select(pick("sidebar")),
                )
        })
    });
    reduce_motion();
    press(cx, "tab enter");
    // Focus is on the first item, not the heading above it.
    press(cx, "enter");
    assert_eq!(*picks.borrow(), ["compact"]);
    press(cx, "enter down down enter");
    assert_eq!(
        *picks.borrow(),
        ["compact", "sidebar"],
        "Down went from Default over the separator and the heading to Sidebar"
    );
    press(cx, "enter");
    let chosen = crate::menu_list::drawn::last("view/row-2");
    assert!(
        chosen.indicator.is_some(),
        "the chosen radio shows its mark"
    );
    let other = crate::menu_list::drawn::last("view/row-1");
    assert!(other.indicator.is_none(), "the other does not");
    let a = Menu::new()
        .heading("Density")
        .item(MenuItem::new("Compact").radio(true))
        .accessibles("View");
    assert_eq!(a[1].role, Role::Group);
    assert_eq!(
        (a[2].role, a[2].selected),
        (Role::MenuItemRadio, Some(true))
    );
}

/// A dropdown near the bottom of the window opens above its trigger, never over it.
#[gpui::test]
fn a_dropdown_flips_above_its_trigger_when_there_is_no_room_below(cx: &mut TestAppContext) {
    use gpui::{Styled, px};
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        gpui::div()
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .child(DropdownMenu::new("dd", "File").menu(menu(&p)))
    });
    reduce_motion();
    cx.simulate_resize(gpui::size(px(400.0), px(300.0)));
    press(cx, "tab enter");
    super::redraw(cx);
    let (trigger, list) = (
        cx.debug_bounds("dd/button/button").unwrap(),
        cx.debug_bounds("dd/menu").unwrap(),
    );
    assert!(
        list.bottom() <= trigger.top(),
        "above: {trigger:?} {list:?}"
    );
    assert!(list.top() >= px(0.0), "and inside the window");
}

/// A context menu near the bottom right of the window opens up and to the left of the pointer,
/// inside the window.
#[gpui::test]
fn a_context_menu_stays_inside_the_window(cx: &mut TestAppContext) {
    use gpui::{Modifiers, MouseButton, Styled, point, px};
    let picks = Picks::default();
    let p = picks.clone();
    let cx = open(cx, move |_, _| {
        ContextMenu::new(
            "ctx",
            "Row",
            gpui::div()
                .w(px(400.0))
                .h(px(300.0))
                .child(Text::new("Right click me")),
        )
        .menu(menu(&p))
    });
    reduce_motion();
    cx.simulate_resize(gpui::size(px(400.0), px(300.0)));
    let at = point(px(390.0), px(290.0));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    super::redraw(cx);
    let list = cx.debug_bounds("ctx/menu").unwrap();
    assert!(
        list.right() <= px(400.0) && list.bottom() <= px(300.0),
        "inside the window: {list:?}"
    );
    assert!(
        list.left() < at.x && list.top() < at.y,
        "opened up and left"
    );
}

/// A right click on a button inside a `ContextMenu` opens the menu and does not press the
/// button (`docs/checks.md`).
#[gpui::test]
fn a_right_click_does_not_press_the_button_under_it(cx: &mut TestAppContext) {
    use gpui::{Modifiers, MouseButton};
    let (picks, pressed) = (Picks::default(), Count::default());
    let (p, c) = (picks.clone(), pressed.clone());
    let cx = open(cx, move |_, _| {
        let c = c.clone();
        ContextMenu::new(
            "ctx",
            "Row",
            Button::new("row")
                .label("Row")
                .on_click(move |_, _, _| c.bump()),
        )
        .menu(menu(&p))
    });
    reduce_motion();
    let at = cx.debug_bounds("row/button").unwrap().center();
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    super::redraw(cx);
    assert_eq!(probe::last("ctx"), Some(1.0), "the menu opened");
    assert_eq!(pressed.get(), 0, "the button was not pressed");
}
