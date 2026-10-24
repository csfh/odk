use std::cell::RefCell;
use std::rc::Rc;

use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};

use odk_theme::component::{NavLook, States, TabStyle};

use super::{Count, open, press};
use crate::{Button, Density, Tabs, Text, v_flex};

/// Items 8, 9, 10: Tab lands on the selected tab, the arrows and Home and End select, and
/// one more Tab leaves the list. Only the selected tab's panel is built.
#[gpui::test]
fn tabs_from_the_keyboard(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let built: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let after = Count::default();
    let (s, b, a) = (seen.clone(), built.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (s, a) = (s.clone(), a.clone());
        let panel = |name: &'static str, b: &Rc<RefCell<Vec<&'static str>>>| {
            let b = b.clone();
            move |_: &mut gpui::Window, _: &mut gpui::App| {
                b.borrow_mut().push(name);
                Text::new(name)
            }
        };
        v_flex()
            .child(
                Tabs::new("t", "Sections")
                    .tab("a", "General", panel("a", &b))
                    .tab("b", "Display", panel("b", &b))
                    .tab("c", "About", panel("c", &b))
                    .on_change(move |v, _, _| s.borrow_mut().push(v.to_string())),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    assert!(
        built.borrow().iter().all(|p| *p == "a"),
        "only the first panel is built"
    );
    press(cx, "tab right end home left right");
    assert_eq!(*seen.borrow(), ["b", "c", "a", "c", "a"]);
    assert_eq!(built.borrow().last(), Some(&"a"));
    // On the first tab, so a Tab that stopped on the other tabs would not leave the list.
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "one Tab leaves the list");
    assert_eq!(seen.borrow().len(), 5);
}

/// Items 14, 16, 17.
#[test]
fn reports_each_tab() {
    let tabs = Tabs::new("t", "Sections")
        .tab("a", "General", |_, _| Text::new(""))
        .tab("b", "Display", |_, _| Text::new(""));
    assert_eq!(tabs.accessible().role, Role::TabList);
    let items = tabs.item_accessibles("b");
    assert_eq!((items[0].role, items[0].selected), (Role::Tab, Some(false)));
    assert_eq!(
        (items[1].name.as_str(), items[1].selected),
        ("Display", Some(true))
    );
}

/// Item 15: the tab list is named by its constructor, so it cannot be left without one.
#[test]
fn the_tab_list_is_named_by_its_constructor() {
    assert_eq!(
        Tabs::new("t", "Settings sections").accessible().name,
        "Settings sections"
    );
}

/// Item 6: the arrow keys skip a disabled tab, and a disabled list is not a tab stop.
#[gpui::test]
fn disabled_tabs_are_skipped(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let after = Count::default();
    let (s, a) = (seen.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (s, a) = (s.clone(), a.clone());
        v_flex()
            .child(
                Tabs::new("t", "Sections")
                    .tab("a", "General", |_, _| Text::new(""))
                    .tab("b", "Display", |_, _| Text::new(""))
                    .tab("c", "About", |_, _| Text::new(""))
                    .disable("b")
                    .on_change(move |v, _, _| s.borrow_mut().push(v.to_string())),
            )
            .child(
                Tabs::new("off", "Sections")
                    .tab("x", "X", |_, _| Text::new(""))
                    .disabled(true),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    press(cx, "tab right");
    assert_eq!(*seen.borrow(), ["c"], "Right skipped the disabled Display");
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab went past the disabled list");
    let tabs = Tabs::new("t", "Sections")
        .tab("a", "A", |_, _| Text::new(""))
        .tab("b", "B", |_, _| Text::new(""))
        .disable("b");
    let items = tabs.item_accessibles("a");
    assert!(!items[0].disabled && items[1].disabled);
}

/// Item 32: focusing, hovering, pressing, selecting and disabling tabs moves nothing.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    use std::cell::Cell;
    let off = Rc::new(Cell::new(false));
    let o = off.clone();
    let cx = open(cx, move |_, _| {
        let tabs = Tabs::new("t", "Sections")
            .tab("a", "General", |_, _| Text::new("a"))
            .tab("b", "Display", |_, _| Text::new("b"))
            .tab("c", "About", |_, _| Text::new("c"));
        let tabs = if o.get() { tabs.disable("c") } else { tabs };
        v_flex()
            .child(tabs)
            .child(Button::new("after").label("After"))
    });
    let disable = off.clone();
    assert_still(
        cx,
        &[
            "t/list",
            "t/tab-0",
            "t/tab-1",
            "t/tab-2",
            "t/label-0",
            "t/label-1",
            "t/label-2",
            "t/panel",
            "after/button",
        ],
        &[
            Step::Keys("tab"),
            Step::Keys("right"),
            Step::Keys("end"),
            Step::Hover("t/tab-0"),
            Step::Press("t/tab-1"),
            Step::Keys("tab"),
            Step::Change("disabling About", Box::new(move || disable.set(true))),
        ],
    );
}

/// Item 32: the selection mark is an indicator on the list's baseline that takes no space: it
/// spans the selected tab exactly, as thick as a focus ring, covers the baseline at the bottom
/// of the list, and follows the selection.
#[gpui::test]
fn the_indicator_sits_on_the_baseline(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        Tabs::new("t", "Sections")
            .tab("a", "General", |_, _| Text::new("a"))
            .tab("b", "Display", |_, _| Text::new("b"))
    });
    let c = crate::components();
    let width = gpui::px(
        c.tab(NavLook::Text, Density::Default, States::NONE.selected(true))
            .indicator_width,
    );
    let border = gpui::px(c.foundation().geometry.border_width);
    for (keys, selected) in [("", "t/tab-0"), ("tab right", "t/tab-1")] {
        if !keys.is_empty() {
            press(cx, keys);
            super::redraw(cx);
        }
        let list = cx.debug_bounds("t/list").unwrap();
        let tab = cx.debug_bounds(selected).unwrap();
        let line = cx.debug_bounds("t/underline").unwrap();
        let baseline = cx.debug_bounds("t/baseline").unwrap();
        assert_eq!(line.bottom(), list.bottom(), "covers the baseline");
        assert_eq!(baseline.bottom(), list.bottom());
        assert_eq!(baseline.size.height, border);
        assert_eq!(line.top(), tab.bottom() - width, "as thick as a ring");
        assert_eq!((line.left(), line.right()), (tab.left(), tab.right()));
    }
}

/// Items 5, 11, 16, 18: selected, focused, hovered and disabled stay apart. With manual activation
/// the arrows move focus without selecting, Enter and Space select the focused tab, and the
/// focused tab and the selected one are drawn each as itself: the ring on one, the indicator and
/// primary text on the other.
#[gpui::test]
fn manual_activation_moves_focus_without_selecting(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let s = seen.clone();
    let cx = open(cx, move |_, _| {
        let s = s.clone();
        Tabs::new("t", "Sections")
            .manual_activation(true)
            .tab("a", "General", |_, _| Text::new("a"))
            .tab("b", "Display", |_, _| Text::new("b"))
            .tab("c", "About", |_, _| Text::new("c"))
            .on_change(move |v, _, _| s.borrow_mut().push(v.to_string()))
    });
    press(cx, "tab right right");
    assert!(seen.borrow().is_empty(), "the arrows only move focus");
    let drawn = |i: usize| crate::drawn::last::<(TabStyle, f32)>(&format!("t/tab-{i}")).0;
    let c = crate::components();
    let d = Density::Default;
    assert_eq!(
        drawn(2),
        c.tab(NavLook::Text, d, States::NONE.focused(true)),
        "focused, not selected"
    );
    assert_eq!(
        drawn(0),
        c.tab(NavLook::Text, d, States::NONE.selected(true)),
        "selected, not focused"
    );
    assert!(drawn(2).focus_visible && drawn(2).indicator.is_none());
    assert!(!drawn(0).focus_visible && drawn(0).indicator.is_some());
    press(cx, "enter");
    assert_eq!(*seen.borrow(), ["c"]);
    press(cx, "left space");
    assert_eq!(*seen.borrow(), ["c", "b"]);
    press(cx, "enter");
    assert_eq!(
        seen.borrow().len(),
        2,
        "selecting the selected tab is no change"
    );
    super::redraw(cx);
    assert_eq!(
        drawn(1),
        c.tab(NavLook::Text, d, States::NONE.selected(true).focused(true))
    );
}

/// Items 11, 32: the focused tab's ring is painted around it, `focus_offset` outside and
/// `focus_width` wide, from outside the list, so neither the list's clipping nor the next tab
/// covers it; it follows focus and goes with it.
#[gpui::test]
fn the_ring_is_painted_around_the_focused_tab(cx: &mut TestAppContext) {
    use crate::interaction::painted;
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(
                Tabs::new("t", "Sections")
                    .manual_activation(true)
                    .tab("a", "General", |_, _| Text::new("a"))
                    .tab("b", "Display", |_, _| Text::new("b")),
            )
            .child(Button::new("after").label("After"))
    });
    super::redraw(cx);
    assert_eq!(painted::take("t"), None, "no ring before focus");
    let ring = crate::components()
        .tab(NavLook::Text, Density::Default, States::NONE)
        .focus;
    let reach = gpui::px(ring.offset + ring.width);
    for (keys, tab) in [("tab", "t/tab-0"), ("right", "t/tab-1")] {
        press(cx, keys);
        super::redraw(cx);
        let tab = cx.debug_bounds(tab).unwrap();
        assert_eq!(painted::take("t"), Some(tab.dilate(reach)), "{keys}");
    }
    press(cx, "tab");
    super::redraw(cx);
    assert_eq!(painted::take("t"), None, "the ring left with focus");
}

/// Items 19, 20, 22: the indicator fades from the old tab to the new over the foundation's
/// `normal` on its curve, and is there at once with reduced motion.
#[gpui::test]
fn the_indicator_fades_between_tabs(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        Tabs::new("t", "Sections")
            .tab("a", "General", |_, _| Text::new("a"))
            .tab("b", "Display", |_, _| Text::new("b"))
    });
    let shown = |i: usize| crate::drawn::last::<(TabStyle, f32)>(&format!("t/tab-{i}")).1;
    assert_eq!(
        (shown(0), shown(1)),
        (1.0, 0.0),
        "no fade on the first frame"
    );
    let normal = crate::foundation().motion.normal;
    press(cx, "tab right");
    cx.executor().advance_clock(normal / 2);
    super::redraw(cx);
    // Each is half way along its own fade, on the curve: the new one coming in fast, the old one
    // going out slow, so the line never disappears between the two.
    let half = crate::foundation().motion.easing.apply(0.5);
    assert!((shown(1) - half).abs() < 0.02, "{}", shown(1));
    assert!((shown(0) - half).abs() < 0.02, "{}", shown(0));
    cx.executor().advance_clock(normal);
    super::redraw(cx);
    assert_eq!((shown(0), shown(1)), (0.0, 1.0));
    super::reduce_motion();
    press(cx, "left");
    assert_eq!((shown(0), shown(1)), (1.0, 0.0), "at once");
}

/// Tabs are a control's height at each density, with no box; an icon is the density's icon size
/// before the label, and a counter follows it.
#[gpui::test]
fn tabs_follow_their_density_and_hold_icons_and_counters(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().children(Density::ALL.map(|d| {
            Tabs::new(gpui::SharedString::from(d.name()), "Sections")
                .density(d)
                .tab("in", "Inbox", |_, _| Text::new(""))
                .icon("in", "✉")
                .counter("in", "12")
                .tab("out", "Sent", |_, _| Text::new(""))
        }))
    });
    for d in Density::ALL {
        let n = d.name();
        let tab = cx.debug_bounds(leak(format!("{n}/tab-0"))).unwrap();
        assert_eq!(tab.size.height, gpui::px(d.metrics().height), "{n}");
        let g = crate::drawn::last::<(TabStyle, f32)>(&format!("{n}/tab-0"))
            .0
            .geometry;
        assert_eq!((g.border_width, g.radius), (0.0, 0.0), "no box");
        let label = cx.debug_bounds(leak(format!("{n}/label-0"))).unwrap();
        let counter = cx.debug_bounds(leak(format!("{n}/counter-0"))).unwrap();
        assert!(counter.left() >= label.right() + gpui::px(g.gap), "{n}");
        assert_eq!(
            label.left(),
            tab.left() + gpui::px(g.padding_x + g.icon_size + g.gap)
        );
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Item 23: tabs that do not fit scroll sideways, never shrink or wrap, and the list brings
/// the selected tab fully into view when the selection changes.
#[gpui::test]
fn the_selected_tab_scrolls_into_view(cx: &mut TestAppContext) {
    use gpui::{Styled, px};
    let cx = open(cx, move |_, _| {
        let mut tabs = Tabs::new("t", "Settings sections");
        for n in 0..10 {
            tabs = tabs.tab(
                gpui::SharedString::from(format!("{n}")),
                "Anzeige und Bildschirmskalierung",
                |_, _| Text::new(""),
            );
        }
        v_flex().child(gpui::div().w(px(300.0)).child(tabs))
    });
    let list = cx.debug_bounds("t/list").unwrap();
    let inside =
        |b: gpui::Bounds<gpui::Pixels>| b.left() >= list.left() && b.right() <= list.right();
    let first = cx.debug_bounds("t/tab-0").unwrap();
    assert!(
        first.size.width > px(200.0),
        "a tab keeps its full label: {:?}",
        first.size
    );
    assert!(
        !inside(cx.debug_bounds("t/tab-9").unwrap()),
        "the last tab starts out of view"
    );

    press(cx, "tab end");
    super::redraw(cx);
    assert!(
        inside(cx.debug_bounds("t/tab-9").unwrap()),
        "End brought it into view"
    );
    assert_eq!(
        cx.debug_bounds("t/tab-9").unwrap().size,
        first.size,
        "and it did not shrink"
    );

    press(cx, "home");
    super::redraw(cx);
    assert!(
        inside(cx.debug_bounds("t/tab-0").unwrap()),
        "Home brought the first back"
    );
}
