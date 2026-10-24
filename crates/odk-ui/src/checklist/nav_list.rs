use std::cell::RefCell;
use std::rc::Rc;

use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};
use odk_theme::component::{NavItemStyle, States};

use super::{Count, open, press, redraw};
use crate::{Button, Density, NavList, v_flex};

type Seen = Rc<RefCell<Vec<String>>>;

fn settings(seen: &Seen) -> NavList {
    let s = seen.clone();
    NavList::new("n", "Settings")
        .group("Workspace")
        .item("general", "General")
        .icon("general", "⚙")
        .item("members", "Members")
        .counter("members", "4")
        .item("billing", "Billing")
        .disable("billing")
        .group("Account")
        .item("profile", "Profile")
        .default_value("general")
        .on_change(move |v, _, _| s.borrow_mut().push(v.to_string()))
}

fn item(i: usize) -> NavItemStyle {
    crate::drawn::last::<NavItemStyle>(&format!("n/item-{i}"))
}

/// Items 8, 9, 10, 6: the list is one tab stop, on the current page; Up and Down move focus,
/// wrapping and skipping a disabled row, without opening anything; Enter or Space opens the
/// focused row, once; one Tab leaves the list.
#[gpui::test]
fn rows_open_from_the_keyboard(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let after = Count::default();
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let a = a.clone();
        v_flex().child(settings(&s)).child(
            Button::new("after")
                .label("After")
                .on_click(move |_, _, _| a.bump()),
        )
    });
    press(cx, "tab down");
    assert!(seen.borrow().is_empty(), "moving focus opens nothing");
    press(cx, "enter");
    assert_eq!(*seen.borrow(), ["members"]);
    // Up to General; then down past the disabled Billing to Profile.
    press(cx, "up enter down down space");
    assert_eq!(*seen.borrow(), ["members", "general", "profile"]);
    press(cx, "enter");
    assert_eq!(
        seen.borrow().len(),
        3,
        "opening the current page is no change"
    );
    // Home goes to the first row, and Up wraps from it to the last.
    press(cx, "home enter");
    assert_eq!(seen.borrow().last().map(String::as_str), Some("general"));
    press(cx, "up enter");
    assert_eq!(seen.borrow().last().map(String::as_str), Some("profile"));
    press(cx, "end home space");
    assert_eq!(seen.borrow().last().map(String::as_str), Some("general"));
    press(cx, "down tab enter");
    assert_eq!(after.get(), 1, "one Tab leaves the list");
}

/// A click opens a row; a click on a disabled one does nothing.
#[gpui::test]
fn a_click_opens_a_row(cx: &mut TestAppContext) {
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| settings(&s));
    for (part, n) in [("n/item-1", 1), ("n/item-2", 1)] {
        let at = cx.debug_bounds(part).unwrap().center();
        cx.simulate_click(at, gpui::Modifiers::none());
        redraw(cx);
        assert_eq!(seen.borrow().len(), n, "{part}");
    }
    assert!(
        item(1).indicator.is_some(),
        "Members is the current page now"
    );
}

/// Items 5, 11, 16, 18: the current page has its fill and indicator; focus is a ring of its own,
/// drawn apart from the current page and painted over the rows after it; a disabled row draws
/// its disabled treatment.
#[gpui::test]
fn current_focused_and_disabled_are_drawn_apart(cx: &mut TestAppContext) {
    use crate::interaction::painted;
    let seen = Seen::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| settings(&s));
    let c = crate::components();
    let d = Density::Default;
    press(cx, "tab down");
    redraw(cx);
    assert_eq!(item(0), c.nav_item(d, States::NONE.selected(true)));
    assert_eq!(item(1), c.nav_item(d, States::NONE.focused(true)));
    assert_eq!(item(2), c.nav_item(d, States::NONE.disabled(true)));
    assert!(item(0).indicator.is_some() && !item(0).focus_visible);
    assert!(item(1).indicator.is_none() && item(1).focus_visible);
    let ring = item(1).focus;
    let members = cx.debug_bounds("n/item-1").unwrap();
    assert_eq!(
        painted::take("n"),
        Some(members.dilate(gpui::px(ring.offset + ring.width)))
    );
}

/// Rows are a control's height at each density, their text centred, the indicator along the
/// leading edge; group headings take no focus.
#[gpui::test]
fn rows_follow_their_density(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().children(Density::ALL.map(|d| {
            NavList::new(gpui::SharedString::from(d.name()), "Settings")
                .density(d)
                .group("Workspace")
                .item("general", "General")
                .default_value("general")
        }))
    });
    for d in Density::ALL {
        let n = d.name();
        let row = cx
            .debug_bounds(Box::leak(format!("{n}/item-0").into_boxed_str()))
            .unwrap();
        assert_eq!(row.size.height, gpui::px(d.metrics().height), "{n}");
        let mark = cx
            .debug_bounds(Box::leak(format!("{n}/indicator").into_boxed_str()))
            .unwrap();
        assert_eq!(
            (mark.left(), mark.size.height),
            (row.left(), row.size.height)
        );
        let header = cx
            .debug_bounds(Box::leak(format!("{n}/group-0").into_boxed_str()))
            .unwrap();
        assert!(header.bottom() <= row.top());
    }
}

/// Item 32: focusing, hovering, pressing, opening and disabling rows moves nothing.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    use std::cell::Cell;
    let off = Rc::new(Cell::new(false));
    let o = off.clone();
    let cx = open(cx, move |_, _| {
        let nav = NavList::new("n", "Settings")
            .item("a", "General")
            .item("b", "Members")
            .counter("b", "4")
            .item("c", "Billing");
        let nav = if o.get() { nav.disable("c") } else { nav };
        v_flex()
            .child(nav)
            .child(Button::new("after").label("After"))
    });
    let disable = off.clone();
    assert_still(
        cx,
        &[
            "n/item-0",
            "n/item-1",
            "n/item-2",
            "n/label-0",
            "n/label-1",
            "n/counter-1",
            "after/button",
        ],
        &[
            Step::Keys("tab"),
            Step::Keys("enter"),
            Step::Keys("down"),
            Step::Keys("space"),
            Step::Hover("n/item-0"),
            Step::Press("n/item-2"),
            Step::Change("disabling Billing", Box::new(move || disable.set(true))),
        ],
    );
}

/// Items 14, 15, 17.
#[test]
fn reports_the_navigation_and_each_row() {
    let nav = settings(&Seen::default());
    let a = nav.accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Navigation, "Settings"));
    let rows = nav.item_accessibles(Some("general"));
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].selected, Some(true));
    assert_eq!(rows[1].name, "Members (4)");
    assert!(rows[2].disabled);
}
