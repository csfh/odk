use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, MouseButton, ParentElement, Pixels, Point, Styled, TestAppContext,
    point, px,
};
use odk_a11y::{Describe, Role};
use odk_theme::component::ResizeHandleStyle;

use super::{Count, FIXTURES, open, press, publish_fixture, redraw};
use crate::drawn;
use crate::{Button, Resizable, ResizablePanel, Text, v_flex};

type Seen = Rc<RefCell<Vec<Vec<f32>>>>;

/// Two panels, the first at 40% between 20% and 70%, and a button after the group.
fn split(seen: &Seen, after: &Count, disabled: bool) -> impl Fn() -> gpui::AnyElement + 'static {
    let (seen, after) = (seen.clone(), after.clone());
    move || {
        let (seen, after) = (seen.clone(), after.clone());
        v_flex()
            .size_full()
            .child(
                v_flex().h(px(300.0)).w_full().child(
                    Resizable::horizontal("split", "List and detail")
                        .disabled(disabled)
                        .panel(
                            ResizablePanel::new()
                                .default_size(0.4)
                                .min_size(0.2)
                                .max_size(0.7)
                                .child(Text::new("List")),
                        )
                        .panel(ResizablePanel::new().child(Text::new("Detail")))
                        .on_layout(move |sizes, _, _| seen.borrow_mut().push(sizes.to_vec())),
                ),
            )
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| after.bump()),
            )
            .into_any_element()
    }
}

fn sizes() -> Vec<f32> {
    drawn::last::<Vec<f32>>("split")
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

fn handle_centre(cx: &mut gpui::VisualTestContext) -> Point<Pixels> {
    cx.debug_bounds("split/handle-0")
        .expect("a handle")
        .center()
}

/// Items 8, 9, 12; ADR 0069: the handle is a tab stop, the arrows move it by a twentieth of the
/// group, Home and End take the panel before it to its limits, each key is reported, and the
/// next Tab leaves the group.
#[gpui::test]
fn a_handle_moves_from_the_keyboard(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, false);
    let cx = open(cx, move |_, _| build());
    assert!(close(&sizes(), &[0.4, 0.6]));
    press(cx, "tab right right");
    assert!(close(&sizes(), &[0.5, 0.5]), "{:?}", sizes());
    press(cx, "left");
    assert!(close(&sizes(), &[0.45, 0.55]));
    press(cx, "home");
    assert!(close(&sizes(), &[0.2, 0.8]), "its smallest");
    press(cx, "end");
    assert!(close(&sizes(), &[0.7, 0.3]), "its largest");
    press(cx, "end");
    assert_eq!(
        seen.borrow().len(),
        5,
        "each move reported, a key that moves nothing not"
    );
    assert!(close(seen.borrow().last().unwrap(), &[0.7, 0.3]));
    press(cx, "up down");
    assert!(
        close(&sizes(), &[0.7, 0.3]),
        "the other axis's keys do nothing"
    );
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "one Tab leaves the group");
}

/// ADR 0069: a drag follows the pointer past the handle until the button is released, stops at
/// the limits, and is reported once, at its end.
#[gpui::test]
fn a_drag_follows_the_pointer_and_reports_once(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, false);
    let cx = open(cx, move |_, _| build());
    let group = cx.debug_bounds("split/group").unwrap();
    let width = f32::from(group.size.width);
    let start = handle_centre(cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    let to = point(start.x + px(width * 0.1), start.y);
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    assert!(close(&sizes(), &[0.5, 0.5]), "{:?}", sizes());
    assert!(seen.borrow().is_empty(), "nothing reported during the drag");
    // Far past the window's edge: the panel stops at its largest.
    let far = point(start.x + px(width * 3.0), start.y + px(400.0));
    cx.simulate_mouse_move(far, Some(MouseButton::Left), Modifiers::none());
    assert!(close(&sizes(), &[0.7, 0.3]));
    cx.simulate_mouse_up(far, MouseButton::Left, Modifiers::none());
    assert_eq!(seen.borrow().len(), 1);
    assert!(close(&seen.borrow()[0], &[0.7, 0.3]));
    // After the release, moving the pointer moves nothing.
    cx.simulate_mouse_move(start, None, Modifiers::none());
    assert!(close(&sizes(), &[0.7, 0.3]));
}

/// Item 10; ADR 0069: Escape during a drag puts back the sizes it began with, and nothing is
/// reported.
#[gpui::test]
fn escape_puts_a_drag_back(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, false);
    let cx = open(cx, move |_, _| build());
    let width = f32::from(cx.debug_bounds("split/group").unwrap().size.width);
    let start = handle_centre(cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    let to = point(start.x - px(width * 0.15), start.y);
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    assert!(close(&sizes(), &[0.25, 0.75]));
    press(cx, "escape");
    assert!(close(&sizes(), &[0.4, 0.6]), "{:?}", sizes());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert!(seen.borrow().is_empty());
}

/// Items 5, 11, 32: the handle lights up under the pointer and shows a ring when focused, and
/// neither moves a panel.
#[gpui::test]
fn hover_and_focus_show_and_move_nothing(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, false);
    let cx = open(cx, move |_, _| build());
    let at_rest: ResizeHandleStyle = drawn::last("split/handle-0");
    let panels = |cx: &mut gpui::VisualTestContext| {
        (
            cx.debug_bounds("split/panel-0").unwrap(),
            cx.debug_bounds("split/panel-1").unwrap(),
        )
    };
    let before = panels(cx);
    let centre = handle_centre(cx);
    cx.simulate_mouse_move(centre, None, Modifiers::none());
    let hovered: ResizeHandleStyle = drawn::last("split/handle-0");
    assert_ne!(hovered.line, at_rest.line);
    assert_eq!(hovered.drawn, crate::foundation().geometry.focus_width);
    assert_eq!(panels(cx), before);
    press(cx, "tab");
    let focused: ResizeHandleStyle = drawn::last("split/handle-0");
    assert!(focused.focus_visible);
    assert_eq!(panels(cx), before);
}

/// Item 6: a disabled group's handles take neither the pointer nor the keyboard.
#[gpui::test]
fn a_disabled_group_does_not_move(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, true);
    let cx = open(cx, move |_, _| build());
    press(cx, "tab enter");
    assert_eq!(after.get(), 1, "Tab goes past the handle");
    let start = handle_centre(cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    let to = point(start.x + px(100.0), start.y);
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert!(close(&sizes(), &[0.4, 0.6]));
    assert!(seen.borrow().is_empty());
}

/// Items 3 and 39: a theme change keeps the sizes.
#[gpui::test]
fn a_theme_change_keeps_the_sizes(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, false);
    let cx = open(cx, move |_, _| build());
    press(cx, "tab right");
    publish_fixture(FIXTURES[1].1);
    redraw(cx);
    assert!(close(&sizes(), &[0.45, 0.55]));
}

/// Items 14, 15, 16, 17: the group and each handle report their role, name and value.
#[test]
fn reports_the_group_and_each_handle() {
    let group = Resizable::vertical("g", "Editor and terminal")
        .panel(ResizablePanel::new().default_size(0.7))
        .panel(ResizablePanel::new())
        .panel(ResizablePanel::new().default_size(0.1));
    let a = group.accessible();
    assert_eq!(
        (a.role, a.name.as_str()),
        (Role::Group, "Editor and terminal")
    );
    let handles = group.item_accessibles();
    let values: Vec<_> = handles.iter().map(|h| h.value.clone().unwrap()).collect();
    assert_eq!(values, ["70%", "20%"]);
    assert!(handles.iter().all(|h| h.role == Role::Separator));
}

/// ADR 0069: the panels take their fractions of what the handle's room leaves, and the handle's
/// target is centred on that room, after a key moves it too.
#[gpui::test]
fn the_panels_take_their_fractions_and_the_handle_sits_between(cx: &mut TestAppContext) {
    let (seen, after) = (Seen::default(), Count::default());
    let build = split(&seen, &after, false);
    let cx = open(cx, move |_, _| build());
    let check = |cx: &mut gpui::VisualTestContext, first: f32| {
        let group = cx.debug_bounds("split/group").unwrap();
        let a = cx.debug_bounds("split/panel-0").unwrap();
        let b = cx.debug_bounds("split/panel-1").unwrap();
        let handle = cx.debug_bounds("split/handle-0").unwrap();
        let room = f32::from(b.origin.x - a.right());
        let width = f32::from(group.size.width) - room;
        assert!(room > 0.0, "the handle takes room");
        assert!(
            (f32::from(a.size.width) - first * width).abs() < 1.0,
            "{a:?}"
        );
        assert!(
            (f32::from(b.size.width) - (1.0 - first) * width).abs() < 1.0,
            "{b:?}"
        );
        let middle = f32::from(a.right()) + room / 2.0;
        assert!(
            (f32::from(handle.center().x) - middle).abs() < 1.0,
            "{handle:?}"
        );
    };
    check(cx, 0.4);
    press(cx, "tab right right");
    check(cx, 0.5);
}
