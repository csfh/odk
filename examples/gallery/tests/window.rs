//! The gallery in a window of any size: its pages scroll inside it, an overlay stays inside it,
//! and what was open survives a resize.

mod common;

use common::{click, open, serial, settle};
use gallery::Page;
use odk::gpui::{
    Modifiers, Pixels, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase,
    VisualTestContext, point, px, size,
};

fn wheel(cx: &mut VisualTestContext, over: odk::gpui::Point<Pixels>, pixels: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: over,
        delta: ScrollDelta::Pixels(point(px(0.0), px(pixels))),
        modifiers: Modifiers::default(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
}

/// A page scrolls inside its own region, the sidebar and toolbar stay where they are, and the
/// scroll is kept across a change of density (which redraws the kept page).
#[test]
fn a_page_scrolls_by_itself() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, Page::Buttons);
    let page = cx.debug_bounds("buttons-page").expect("the page");
    let button = |cx: &mut VisualTestContext| cx.debug_bounds("variant-0/button").unwrap().origin.y;
    let sidebar = cx.debug_bounds("pages/list").unwrap().origin.y;
    let toolbar = cx.debug_bounds("density/group").unwrap().origin.y;
    let before = button(cx);

    wheel(cx, page.center(), -300.0);
    let after = button(cx);
    assert!(
        after < before - px(100.0),
        "the button moved from {before:?} to {after:?}"
    );
    assert_eq!(cx.debug_bounds("pages/list").unwrap().origin.y, sidebar);
    assert_eq!(cx.debug_bounds("density/group").unwrap().origin.y, toolbar);

    click(cx, "density/item-0");
    assert!(
        button(cx) < before - px(100.0),
        "the page lost its scroll when the density changed"
    );
}

/// A popover placed against the end of a row stays inside the window, in a wide window and in a
/// narrow one: it is shifted back, not clipped.
#[test]
fn an_overlay_stays_inside_the_window() {
    let _serial = serial();
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, Page::Overlays);
    for width in [1400.0, 640.0] {
        cx.simulate_resize(size(px(width), px(900.0)));
        settle(cx);
        // Scroll the edge popover's trigger into view.
        let page = cx.debug_bounds("overlays-page").expect("the page");
        for _ in 0..30 {
            let trigger = cx.debug_bounds("pop-edge-button/button").unwrap();
            if trigger.bottom() < page.bottom() - px(80.0) && trigger.origin.y > page.origin.y {
                break;
            }
            wheel(cx, page.center(), -200.0);
        }
        click(cx, "pop-edge-button/button");
        settle(cx);
        let window = cx.update(|window, _| window.bounds());
        let popover = cx
            .debug_bounds("pop-edge/popover")
            .expect("the popover opened");
        assert!(
            popover.left() >= px(0.0) && popover.right() <= window.size.width,
            "at {width} px the popover spans {:?} to {:?}",
            popover.left(),
            popover.right()
        );
        assert!(popover.top() >= px(0.0) && popover.bottom() <= window.size.height);
        cx.simulate_keystrokes("escape");
        settle(cx);
    }
}
