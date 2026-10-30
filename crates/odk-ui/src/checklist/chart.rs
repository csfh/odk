use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, MouseButton, ParentElement, ScrollDelta, ScrollWheelEvent, Styled,
    TestAppContext, TouchPhase, point, px,
};
use odk_a11y::{Describe, Role};
use odk_theme::component::SERIES_ROLES;
use odk_theme::semantic;

use super::{FIXTURES, open, press, publish_fixture, redraw, reduce_motion};
use crate::interaction::take_frame_requests;
use crate::{Button, Chart, ChartHeight, ChartPath, Ink, Stack, StrokeWidth, components, drawn};

type Events = Rc<RefCell<Vec<String>>>;

/// A chart 200 by 100 that records what it hears and the size it was painted at, and a button
/// after it.
fn chart(
    generation: Rc<Cell<u64>>,
    events: &Events,
    painted: &Rc<Cell<(f32, f32)>>,
) -> impl Fn() -> gpui::AnyElement + 'static {
    let (events, painted) = (events.clone(), painted.clone());
    move || {
        let (press, pan, zoom) = (events.clone(), events.clone(), events.clone());
        let painted = painted.clone();
        Stack::new()
            .child(
                Stack::new().w(px(200.0)).h(px(100.0)).child(
                    Chart::new("c", "CPU")
                        .transition(generation.get())
                        .paint(move |scene, _| {
                            painted.set((scene.width(), scene.height()));
                            let path = ChartPath::new()
                                .move_to(0.0, 0.0)
                                .line_to(scene.width(), scene.height())
                                .arc(100.0, 50.0, 20.0, 0.0, std::f32::consts::TAU);
                            scene.stroke(&path, Ink::Series(0), StrokeWidth::Thin);
                        })
                        .on_press(move |x, y, _, _| {
                            press.borrow_mut().push(format!("press {x} {y}"))
                        })
                        .on_pan(move |dx, dy, _, _| pan.borrow_mut().push(format!("pan {dx} {dy}")))
                        .on_zoom(move |f, x, y, _, _| {
                            zoom.borrow_mut().push(format!("zoom {f:.2} {x} {y}"))
                        }),
                ),
            )
            .child(Button::new("after").label("After"))
            .into_any_element()
    }
}

fn progress() -> f32 {
    drawn::last("c")
}

/// Items 19, 20, 22, 31, 35, 39; ADR 0071: a new generation starts a transition from 0 over the
/// foundation's `normal` on GPUI's clock, which asks for frames while it moves and none once it
/// has arrived; with reduced motion it is at 1 at once.
#[gpui::test]
fn a_transition_runs_over_normal_and_then_rests(cx: &mut TestAppContext) {
    let generation = Rc::new(Cell::new(1));
    let (events, painted) = (Events::default(), Rc::default());
    let build = chart(generation.clone(), &events, &painted);
    let cx = open(cx, move |_, _| build());
    assert_eq!(progress(), 1.0, "a first generation does not move");
    take_frame_requests();
    redraw(cx);
    assert_eq!(take_frame_requests(), 0, "at rest it asks for nothing");
    generation.set(2);
    redraw(cx);
    assert_eq!(progress(), 0.0);
    assert!(take_frame_requests() > 0, "it moves");
    let normal = crate::foundation()
        .motion
        .transition(odk_theme::foundation::MotionDuration::Normal);
    cx.executor().advance_clock(normal / 2);
    redraw(cx);
    let half = progress();
    assert!(half > 0.0 && half < 1.0, "{half}");
    cx.executor().advance_clock(normal);
    redraw(cx);
    assert_eq!(progress(), 1.0);
    take_frame_requests();
    redraw(cx);
    assert_eq!(
        take_frame_requests(),
        0,
        "it asks for nothing once it has arrived"
    );
    reduce_motion();
    generation.set(3);
    redraw(cx);
    assert_eq!(progress(), 1.0, "reduced motion: at once");
}

/// Item 35: a chart scrolled out of view asks for no frame even while its transition runs.
#[gpui::test]
fn a_chart_out_of_view_asks_for_nothing(cx: &mut TestAppContext) {
    let generation = Rc::new(Cell::new(1));
    let g = generation.clone();
    let cx = open(cx, move |_, _| {
        Stack::new()
            .w(px(100.0))
            .h(px(10.0))
            .overflow_hidden()
            .child(gpui::div().h(px(50.0)).flex_shrink_0())
            .child(
                Stack::new()
                    .h(px(40.0))
                    .w_full()
                    .flex_shrink_0()
                    .child(Chart::new("c", "CPU").transition(g.get()).paint(|_, _| {})),
            )
    });
    generation.set(2);
    take_frame_requests();
    redraw(cx);
    assert!(progress() < 1.0, "it is moving");
    assert_eq!(take_frame_requests(), 0);
}

/// ADR 0071: the scene is the chart's box, in its own coordinates.
#[gpui::test]
fn the_scene_is_the_box(cx: &mut TestAppContext) {
    let (events, painted) = (Events::default(), Rc::new(Cell::new((0.0, 0.0))));
    let build = chart(Rc::new(Cell::new(1)), &events, &painted);
    let _cx = open(cx, move |_, _| build());
    assert_eq!(painted.get(), (200.0, 100.0));
}

/// ADR 0075: a chart that asks for a height is that tall and as wide as its parent, in a parent
/// that gives it no height; one that asks for none fills its parent.
#[gpui::test]
fn a_height_is_the_charts_and_the_width_its_parents(cx: &mut TestAppContext) {
    let painted: Rc<RefCell<Vec<(f32, f32)>>> = Rc::default();
    let seen = painted.clone();
    let _cx = open(cx, move |_, _| {
        let row = |h: ChartHeight, seen: Rc<RefCell<Vec<(f32, f32)>>>| {
            Chart::new(gpui::SharedString::from(format!("h{h:?}")), "Sized")
                .height(h)
                .paint(move |scene, _| {
                    seen.borrow_mut().push((scene.width(), scene.height()));
                })
        };
        Stack::new()
            .w(px(300.0))
            .child(row(ChartHeight::Compact, seen.clone()))
            .child(row(ChartHeight::Regular, seen.clone()))
            .child(row(ChartHeight::Tall, seen.clone()))
            .into_any_element()
    });
    let h = components().chart().heights;
    let drawn: Vec<(f32, f32)> = painted.borrow().iter().take(3).copied().collect();
    assert_eq!(drawn, vec![(300.0, h[0]), (300.0, h[1]), (300.0, h[2])]);
    assert!(h[0] < h[1] && h[1] < h[2]);
}

/// Items 8, 9, 10; ADR 0071: a chart that hears the pointer is a tab stop; the arrows pan by a
/// twentieth of the box, `+` and `-` zoom about its middle, Enter presses its middle, and the
/// next Tab leaves.
#[gpui::test]
fn the_keys_pan_zoom_and_press(cx: &mut TestAppContext) {
    let (events, painted) = (Events::default(), Rc::default());
    let build = chart(Rc::new(Cell::new(1)), &events, &painted);
    let cx = open(cx, move |_, _| build());
    press(cx, "tab right up + - enter");
    assert_eq!(
        *events.borrow(),
        [
            "pan 10 0",
            "pan 0 -5",
            "zoom 1.25 100 50",
            "zoom 0.80 100 50",
            "press 100 50"
        ]
    );
}

/// ADR 0071: a press reports where in the box, a drag pans by the pointer's moves until the
/// release, and the wheel zooms about the pointer.
#[gpui::test]
fn the_pointer_presses_pans_and_zooms(cx: &mut TestAppContext) {
    let (events, painted) = (Events::default(), Rc::default());
    let build = chart(Rc::new(Cell::new(1)), &events, &painted);
    let cx = open(cx, move |_, _| build());
    let b = cx.debug_bounds("c/chart").unwrap();
    let at = point(b.origin.x + px(30.0), b.origin.y + px(20.0));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    let to = point(at.x + px(15.0), at.y + px(5.0));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(to.x + px(40.0), to.y), None, Modifiers::none());
    cx.simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    let seen = events.borrow().clone();
    assert_eq!(&seen[..2], ["press 30 20", "pan 15 5"], "{seen:?}");
    assert_eq!(seen.len(), 3, "no pan after the release: {seen:?}");
    assert!(seen[2].starts_with("zoom 1.12 30 20"), "{seen:?}");
}

/// Item 3: inks are the theme's; a seventh series takes the first's colour and the ramp stops
/// at its top; a Role's ink is that Role's series colour. A theme change recolours on the next
/// frame.
#[gpui::test]
fn inks_are_the_themes(cx: &mut TestAppContext) {
    crate::theme::install_test_theme();
    let s = crate::components().chart();
    let resolve = |ink: Ink| ink.resolve(&s);
    assert_eq!(resolve(Ink::Series(6)), resolve(Ink::Series(0)));
    assert_eq!(resolve(Ink::Heat(9)), resolve(Ink::Heat(4)));
    let danger = SERIES_ROLES
        .iter()
        .position(|r| *r == semantic::Role::Danger)
        .unwrap();
    assert_eq!(
        resolve(Ink::Role(semantic::Role::Danger)),
        resolve(Ink::Series(danger))
    );
    let (events, painted) = (Events::default(), Rc::default());
    let build = chart(Rc::new(Cell::new(1)), &events, &painted);
    let cx = open(cx, move |_, _| build());
    let before = crate::components().chart().series[0];
    publish_fixture(FIXTURES[4].1);
    redraw(cx);
    assert_ne!(crate::components().chart().series[0], before);
}

/// Items 14, 15: a chart reports `Image` with its name and description.
#[test]
fn reports_an_image() {
    let a = Chart::new("c", "Memory")
        .description("8 of 16 GB")
        .accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Image, "Memory"));
    assert_eq!(a.description.as_deref(), Some("8 of 16 GB"));
}

/// Item 10; ADR 0071: Escape during a drag pans back by what the drag moved, and ends it.
#[gpui::test]
fn escape_pans_a_drag_back(cx: &mut TestAppContext) {
    let (events, painted) = (Events::default(), Rc::default());
    let build = chart(Rc::new(Cell::new(1)), &events, &painted);
    let cx = open(cx, move |_, _| build());
    let b = cx.debug_bounds("c/chart").unwrap();
    let at = point(b.origin.x + px(30.0), b.origin.y + px(20.0));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    let mid = point(at.x + px(10.0), at.y);
    cx.simulate_mouse_move(mid, Some(MouseButton::Left), Modifiers::none());
    let to = point(mid.x + px(5.0), mid.y + px(4.0));
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    press(cx, "escape");
    let further = point(to.x + px(20.0), to.y);
    cx.simulate_mouse_move(further, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(further, MouseButton::Left, Modifiers::none());
    assert_eq!(
        *events.borrow(),
        ["press 30 20", "pan 10 0", "pan 5 4", "pan -15 -4"],
        "back by the whole drag, and nothing after it"
    );
}
