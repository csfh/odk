use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Bounds, Modifiers, ParentElement, Pixels, Point, TestAppContext, VisualTestContext, px, size,
};
use odk_a11y::{Describe, Role};

use super::dialog::dialog_around;
use super::stability::{Step, assert_still};
use super::{Count, open, press, redraw, reduce_motion};
use crate::overlay::probe;
use crate::{Button, Input, Select, Size, h_flex, v_flex};

type Changes = Rc<RefCell<Vec<String>>>;

/// Five options: Cherry is disabled, and three start with A.
fn fruit(changes: &Changes) -> Select {
    let changes = changes.clone();
    Select::new("sl", "Fruit")
        .item("a", "Apple")
        .item("b", "Banana")
        .item("c", "Cherry")
        .item("d", "Apricot")
        .item("e", "Avocado")
        .disable("c")
        .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
}

fn centre(cx: &mut VisualTestContext, part: &'static str) -> Point<Pixels> {
    cx.debug_bounds(part)
        .unwrap_or_else(|| panic!("no part named {part}"))
        .center()
}

fn click(cx: &mut VisualTestContext, part: &'static str) {
    let at = centre(cx, part);
    cx.simulate_click(at, Modifiers::none());
    redraw(cx);
}

/// Items 8, 10, 13: Enter opens the list with focus on the chosen option, Down skips the
/// disabled one, Enter chooses and closes, and focus goes back to the box, so Enter opens it
/// again on the new choice.
#[gpui::test]
fn opens_on_the_chosen_option_and_chooses(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c).default_value("b"));
    press(cx, "tab enter down enter");
    assert_eq!(
        *changes.borrow(),
        ["d"],
        "it opened on Banana, and Down skipped Cherry"
    );
    // Back on the box, which now holds Apricot: choosing it again changes nothing.
    press(cx, "enter enter");
    assert_eq!(*changes.borrow(), ["d"]);
    // Up from Apricot skips Cherry too, and wraps past the first option.
    press(cx, "enter up enter");
    assert_eq!(*changes.borrow(), ["d", "b"]);
    press(cx, "enter up up enter");
    assert_eq!(*changes.borrow(), ["d", "b", "e"], "Up wraps");
}

/// Item 8: Down and Up open the list on the chosen option, Space chooses, and Home and End
/// jump.
#[gpui::test]
fn down_up_space_home_and_end(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c));
    press(cx, "tab down space");
    assert_eq!(*changes.borrow(), ["a"], "nothing chosen: the first option");
    press(cx, "up end space");
    assert_eq!(*changes.borrow(), ["a", "e"]);
    press(cx, "down home space");
    assert_eq!(*changes.borrow(), ["a", "e", "a"]);
}

/// Items 10, 13: Escape closes the list without choosing, and focus goes back to the box.
#[gpui::test]
fn escape_closes_the_list(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c).default_value("b"));
    reduce_motion();
    press(cx, "tab enter down");
    assert_eq!(probe::last("sl"), Some(1.0), "open");
    press(cx, "escape");
    assert_eq!(probe::last("sl"), None, "closed");
    assert!(changes.borrow().is_empty());
    // Closed, with focus on the box: Enter opens the list again, on Banana. Still open,
    // Enter would choose Apricot.
    press(cx, "enter enter");
    assert!(changes.borrow().is_empty(), "Banana was chosen already");
}

/// Item 13: Tab closes an open list instead of leaving it open behind, and chooses nothing.
#[gpui::test]
fn tab_closes_the_list(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let after = Count::default();
    let (c, a) = (changes.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        h_flex().child(fruit(&c)).child(
            Button::new("after")
                .label("After")
                .on_click(move |_, _, _| a.bump()),
        )
    });
    press(cx, "tab enter down tab");
    assert!(changes.borrow().is_empty(), "Tab chose nothing");
    assert_eq!(after.get(), 0);
    press(cx, "enter enter");
    assert_eq!(*changes.borrow(), ["a"], "focus was back on the box");
    assert_eq!(after.get(), 0, "Tab only closed the list");
}

/// Item 8: typing the start of a label opens the list on the first match; in the list it
/// moves to the next match, and pressing the same letter again steps through its options.
#[gpui::test]
fn typing_finds_an_option(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c));
    // On the closed box: "b" opens on Banana.
    press(cx, "tab b enter");
    assert_eq!(*changes.borrow(), ["b"]);
    // In the list: "a" from Banana goes to the next A after it, Apricot.
    press(cx, "enter a enter");
    assert_eq!(*changes.borrow(), ["b", "d"]);
    // Repeated, from Apricot: Avocado, then round to Apple.
    press(cx, "enter a a enter");
    assert_eq!(*changes.borrow(), ["b", "d", "a"]);
    // More letters narrow the search: "av" from Apple goes to Avocado.
    press(cx, "enter a v enter");
    assert_eq!(*changes.borrow(), ["b", "d", "a", "e"]);
}

/// Items 8, 31: a pause ends the search, timed on GPUI's clock, so the next key starts a new
/// one.
#[gpui::test]
fn a_pause_starts_a_new_search(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c));
    // "a" opens on Apple; "a" again goes to Apricot; "p" narrows to "ap", which Apricot
    // matches; "r" keeps it ("apr").
    press(cx, "tab enter a p r");
    cx.executor().advance_clock(Duration::from_millis(1100));
    // After the pause "a" is a new search: the next A after Apricot, which is Avocado. Had the
    // search gone on, "apra" would match nothing and Apricot would stay.
    press(cx, "a enter");
    assert_eq!(*changes.borrow(), ["e"]);
}

/// Item 8: a key that is not a character, or that holds Ctrl or Alt, is not a search.
#[gpui::test]
fn shortcuts_are_not_searches(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c).default_value("a"));
    press(cx, "tab enter ctrl-b alt-b enter");
    assert!(changes.borrow().is_empty(), "Apple stayed");
}

/// Items 6, 9: a disabled select is not a tab stop and ignores the keyboard and the pointer.
#[gpui::test]
fn a_disabled_select_is_inert(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let after = Count::default();
    let (c, a) = (changes.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex().child(fruit(&c).disabled(true)).child(
            Button::new("after")
                .label("After")
                .on_click(move |_, _, _| a.bump()),
        )
    });
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab skipped the select");
    click(cx, "sl/trigger");
    press(cx, "enter");
    assert!(changes.borrow().is_empty());
    assert_eq!(
        after.get(),
        2,
        "the click opened nothing; Enter pressed the button"
    );
}

/// Item 6: a disabled option cannot be reached by the pointer either.
#[gpui::test]
fn a_disabled_option_cannot_be_clicked(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c));
    reduce_motion();
    click(cx, "sl/trigger");
    click(cx, "sl/row-2");
    assert!(changes.borrow().is_empty(), "Cherry is disabled");
    click(cx, "sl/row-1");
    assert_eq!(*changes.borrow(), ["b"]);
}

/// Items 9, 23: a select with nothing to choose does not open, and Tab goes on past it.
#[gpui::test]
fn a_select_with_nothing_to_choose_does_not_open(cx: &mut TestAppContext) {
    let after = Count::default();
    let a = after.clone();
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex()
            .child(Select::new("none", "Nothing").placeholder("No options"))
            .child(
                Select::new("off", "All disabled")
                    .item("x", "X")
                    .disable("x"),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    // Enter on each does nothing, and Tab moves on instead of closing a list.
    press(cx, "tab enter tab enter tab enter");
    assert_eq!(after.get(), 1);
}

/// Item 23: pressing a select with nothing to choose leaves nothing behind that opens the list
/// once options arrive.
#[gpui::test]
fn options_that_arrive_later_do_not_open_the_list(cx: &mut TestAppContext) {
    let loaded = Rc::new(std::cell::Cell::new(false));
    let l = loaded.clone();
    let cx = open(cx, move |_, _| {
        let select = Select::new("sl", "Fruit");
        if l.get() {
            select.item("a", "Apple").item("b", "Banana")
        } else {
            select
        }
    });
    reduce_motion();
    press(cx, "tab enter down");
    loaded.set(true);
    redraw(cx);
    assert_eq!(probe::last("sl"), None, "it was never opened");
}

/// Item 24: controlled, the box shows the value the application gives it and `on_change`
/// reports a choice every time, until the application follows.
#[gpui::test]
fn a_controlled_select_shows_what_it_is_given(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c).value("a"));
    press(cx, "tab enter down enter");
    assert_eq!(*changes.borrow(), ["b"]);
    // Still Apple: the list opens on it, and Down and Enter choose Banana again.
    press(cx, "enter down enter");
    assert_eq!(*changes.borrow(), ["b", "b"]);
}

/// Item 24: following `on_change` with `value` shows the new choice.
#[gpui::test]
fn following_on_change_moves_the_choice(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let value = Rc::new(RefCell::new("a".to_string()));
    let (c, v) = (changes.clone(), value.clone());
    let cx = open(cx, move |_, _| {
        let (v, mirror) = (v.clone(), v.clone());
        fruit(&c).value(v.borrow().clone()).on_change({
            let c = c.clone();
            move |value, window, _| {
                c.borrow_mut().push(value.to_string());
                *mirror.borrow_mut() = value.to_string();
                window.refresh();
            }
        })
    });
    press(cx, "tab enter down enter");
    assert_eq!(*value.borrow(), "b");
    // The list opens on Banana now: one Down skips Cherry to Apricot.
    press(cx, "enter down enter");
    assert_eq!(*changes.borrow(), ["b", "d"]);
}

/// Item 8: the pointer opens the list, chooses an option, and a click on the box while it
/// is open closes it again.
#[gpui::test]
fn the_pointer_opens_chooses_and_closes(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = open(cx, move |_, _| fruit(&c));
    reduce_motion();
    click(cx, "sl/trigger");
    assert_eq!(probe::last("sl"), Some(1.0), "a click opens the list");
    click(cx, "sl/row-3");
    assert_eq!(*changes.borrow(), ["d"]);
    assert_eq!(probe::last("sl"), None, "and choosing closes it");
    // A second click on the open box closes it. Closed, with focus on the box, Down opens the
    // list on Apricot and the next Down moves on to Avocado.
    click(cx, "sl/trigger");
    click(cx, "sl/trigger");
    assert_eq!(probe::last("sl"), None);
    press(cx, "down down enter");
    assert_eq!(*changes.borrow(), ["d", "e"]);
    // A press elsewhere closes an open list.
    click(cx, "sl/trigger");
    assert_eq!(probe::last("sl"), Some(1.0));
    cx.simulate_click(gpui::point(px(500.0), px(500.0)), Modifiers::none());
    redraw(cx);
    assert_eq!(probe::last("sl"), None);
    assert_eq!(*changes.borrow(), ["d", "e"]);
}

/// Item 33: inside a dialog, the list opens above the dialog, a click on an option reaches it,
/// and Escape closes the list before it closes the dialog.
#[gpui::test]
fn a_select_in_a_dialog(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let closed = Count::default();
    let c = changes.clone();
    let cx = dialog_around(cx, closed.clone(), move || fruit(&c));
    click(cx, "sl/trigger");
    click(cx, "sl/row-1");
    assert_eq!(*changes.borrow(), ["b"], "the click reached the option");
    assert_eq!(closed.get(), 0, "and not the scrim behind it");

    click(cx, "sl/trigger");
    press(cx, "escape");
    assert_eq!(closed.get(), 0, "Escape closed only the list");
    press(cx, "escape");
    assert_eq!(closed.get(), 1, "the next Escape closed the dialog");
}

/// Item 33: the list is not cut off by the dialog's panel or its scrolling body. A list that
/// reaches below the panel takes the click on an option there.
#[gpui::test]
fn a_list_in_a_dialog_reaches_past_the_panel(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let c = changes.clone();
    let cx = dialog_around(cx, Count::default(), move || {
        let c = c.clone();
        (0..12).fold(
            Select::new("sl", "Number")
                .on_change(move |v, _, _| c.borrow_mut().push(v.to_string())),
            |s, i| s.item(format!("n{i}"), format!("Number {i}")),
        )
    });
    click(cx, "sl/trigger");
    let panel = cx.debug_bounds("dlg/panel").unwrap();
    let last = cx.debug_bounds("sl/row-11").unwrap();
    assert!(
        last.bottom() > panel.bottom(),
        "the list reaches past the panel: {last:?} {panel:?}"
    );
    click(cx, "sl/row-11");
    assert_eq!(*changes.borrow(), ["n11"], "and takes the click there");
}

/// Item 33: a list opened in a dialog that is itself inside a sheet.
#[gpui::test]
fn a_select_in_a_dialog_in_a_sheet(cx: &mut TestAppContext) {
    let changes = Changes::default();
    let closed = Count::default();
    let (c, d) = (changes.clone(), closed.clone());
    let cx = open(cx, move |_, _| {
        let (c, d) = (c.clone(), d.clone());
        crate::Sheet::new("sh", "Settings")
            .open(true)
            .content(move |_, _| {
                let (c, d) = (c.clone(), d.clone());
                crate::Dialog::new("dlg", "Choose")
                    .open(true)
                    .on_close(move |_, _| d.bump())
                    .content(move |_, _| fruit(&c))
            })
    });
    reduce_motion();
    redraw(cx);
    click(cx, "sl/trigger");
    click(cx, "sl/row-1");
    assert_eq!(*changes.borrow(), ["b"]);
    assert_eq!(closed.get(), 0, "the click did not reach a scrim");
    click(cx, "sl/trigger");
    press(cx, "escape");
    assert_eq!(closed.get(), 0, "Escape closed only the list");
}

/// Items 8, 23: a list taller than the window scrolls, and opening it scrolls the chosen
/// option into view. Arrow keys keep the focused option in view.
#[gpui::test]
fn a_long_list_scrolls(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        (0..60).fold(Select::new("sl", "Number").default_value("n40"), |s, i| {
            s.item(format!("n{i}"), format!("Number {i}"))
        })
    });
    reduce_motion();
    cx.simulate_resize(size(px(400.0), px(300.0)));
    press(cx, "tab enter");
    redraw(cx);
    let window = Bounds::new(
        gpui::point(px(0.0), px(0.0)),
        cx.update(|window, _| window.viewport_size()),
    );
    let list = cx.debug_bounds("sl/menu").unwrap();
    assert!(
        list.top() >= window.top() && list.bottom() <= window.bottom(),
        "the list fits the window: {list:?}"
    );
    let shown = |cx: &mut VisualTestContext, row: &'static str| {
        let r = cx.debug_bounds(row).unwrap();
        r.top() >= list.top() && r.bottom() <= list.bottom()
    };
    assert!(shown(cx, "sl/row-40"), "the chosen option is in view");
    press(cx, "end");
    redraw(cx);
    assert!(shown(cx, "sl/row-59"), "End scrolls to the last option");
    press(cx, "home");
    redraw(cx);
    assert!(shown(cx, "sl/row-0"), "Home scrolls back");
    press(cx, "up");
    redraw(cx);
    assert!(shown(cx, "sl/row-59"), "Up wraps and scrolls");
}

/// Items 14, 15, 16, 17: the box is a combobox named by its constructor and reports the
/// option it shows; the list reports each option with its state.
#[test]
fn reports_the_box_and_its_options() {
    let s = fruit(&Changes::default()).default_value("d");
    let a = s.accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Combobox, "Fruit"));
    assert_eq!(a.value.as_deref(), Some("Apricot"));
    let items = s.item_accessibles(Some("d"));
    assert_eq!(items[0].role, Role::Listbox);
    assert_eq!(items.len(), 6);
    assert_eq!(
        (items[4].name.as_str(), items[4].selected),
        ("Apricot", Some(true))
    );
    assert!(items[3].disabled, "Cherry");
    assert!(!items[1].disabled);
}

/// Item 32: opening the list, moving through it, hovering it and choosing an option of a
/// different length move nothing in the box or around it.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                Select::new("sl", "Greeting")
                    .item("s", "Hi")
                    .item(
                        "l",
                        "A much longer greeting than the box has room for at this width",
                    )
                    .item("m", "Hello")
                    .placeholder("Pick one"),
            )
            .child(Button::new("after").label("After"))
    });
    reduce_motion();
    let parts = ["sl/trigger", "sl/value", "after/button"];
    assert_still(
        cx,
        &parts,
        &[
            Step::Hover("sl/trigger"),
            Step::Keys("tab"),
            Step::Keys("enter"),
        ],
    );
    let in_list = [
        "sl/trigger",
        "sl/value",
        "after/button",
        "sl/menu",
        "sl/row-0",
        "sl/row-1",
        "sl/row-2",
        "sl/label-0",
        "sl/label-1",
        "sl/label-2",
    ];
    // Choosing the long label, then the short one, keeps the box the size it was.
    assert_still(
        cx,
        &in_list,
        &[
            Step::Hover("sl/row-0"),
            Step::Hover("sl/row-1"),
            Step::Keys("down"),
            Step::Keys("down"),
            Step::Press("sl/row-1"),
            Step::Keys("enter"),
            Step::Keys("down enter"),
        ],
    );
}

/// A select and an input of the same size are as tall to within a pixel, so they line up in a
/// form.
#[gpui::test]
fn a_select_is_as_tall_as_an_input(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(Select::new("ssm", "S").size(Size::Sm).item("a", "A"))
            .child(Input::new("ism", "I").size(Size::Sm))
            .child(Select::new("smd", "S").item("a", "A"))
            .child(Input::new("imd", "I"))
            .child(Select::new("slg", "S").size(Size::Lg).item("a", "A"))
            .child(Input::new("ilg", "I").size(Size::Lg))
    });
    redraw(cx);
    for (select, input) in [
        ("ssm/trigger", "ism/box"),
        ("smd/trigger", "imd/box"),
        ("slg/trigger", "ilg/box"),
    ] {
        let (s, i) = (
            cx.debug_bounds(select).unwrap(),
            cx.debug_bounds(input).unwrap(),
        );
        // Layout rounds a fractional line height differently in the two, by half a pixel.
        let apart = (s.size.height - i.size.height).abs();
        assert!(apart <= px(1.0), "{select}: {s:?} against {i:?}");
    }
}

/// Item 15: every select is named by its constructor.
#[test]
fn selects_are_named() {
    assert_eq!(Select::new("s", "Country").accessible().name, "Country");
}
