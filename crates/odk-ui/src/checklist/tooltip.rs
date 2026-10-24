use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};

use super::{Count, open, press, redraw, reduce_motion};
use crate::overlay::probe;
use crate::{Button, Tooltip, h_flex, v_flex};

fn centre(cx: &mut VisualTestContext, part: &'static str) -> gpui::Point<gpui::Pixels> {
    let b = cx.debug_bounds(part).unwrap();
    gpui::point(
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    )
}

/// How open the tooltip is now, drawn at the current time.
fn shown(cx: &mut VisualTestContext) -> Option<f32> {
    redraw(cx);
    probe::last("tt")
}

/// Items 13, 14, 15: the tooltip is named by its text, adds no tab stop, opens when its
/// button has focus, closes on Escape, and the button still takes Enter.
#[gpui::test]
fn focus_opens_it_and_escape_closes_it(cx: &mut TestAppContext) {
    let (clicks, after) = (Count::default(), Count::default());
    let (c, a) = (clicks.clone(), after.clone());
    let cx = open(cx, move |_, _| {
        let (c, a) = (c.clone(), a.clone());
        h_flex()
            .child(Tooltip::new(
                "tt",
                "Save the file",
                Button::new("save")
                    .label("Save")
                    .on_click(move |_, _, _| c.bump()),
            ))
            .child(
                Button::new("after")
                    .label("After")
                    .on_click(move |_, _, _| a.bump()),
            )
    });
    reduce_motion();
    assert_eq!(shown(cx), None, "closed until the button has focus");
    press(cx, "tab");
    assert_eq!(shown(cx), Some(1.0), "focus on the button opened it");
    press(cx, "enter");
    assert_eq!(clicks.get(), 1, "the button still takes Enter");
    press(cx, "escape");
    assert_eq!(shown(cx), None, "Escape closed it");
    press(cx, "tab enter");
    assert_eq!(
        after.get(),
        1,
        "no extra tab stop: one Tab reached the next button"
    );
    let a = Tooltip::new("tt", "Save the file", Button::new("s")).accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Tooltip, "Save the file"));
}

/// The pointer resting on the trigger opens it after `motion.hover_open`; moving into the
/// tooltip keeps it open; leaving both closes it after `motion.hover_close`.
#[gpui::test]
fn the_pointer_opens_it_after_a_delay_and_can_rest_on_it(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().pt(px(120.0)).child(Tooltip::new(
            "tt",
            "Save the file",
            Button::new("save").label("Save"),
        ))
    });
    reduce_motion();
    let motion = crate::foundation().motion.clone();
    let at = centre(cx, "save/button");
    cx.simulate_mouse_move(at, None, Modifiers::default());
    assert_eq!(shown(cx), None, "not yet");
    cx.executor().advance_clock(motion.hover_open);
    assert_eq!(shown(cx), Some(1.0), "open after the delay");

    let at = centre(cx, "tt/tip");
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.executor().advance_clock(motion.hover_close * 2);
    assert_eq!(shown(cx), Some(1.0), "the pointer can rest on the tooltip");

    cx.simulate_mouse_move(
        gpui::point(px(900.0), px(900.0)),
        None,
        Modifiers::default(),
    );
    cx.executor().advance_clock(motion.hover_close);
    assert_eq!(shown(cx), None, "closed after the pointer left both");
}

/// It opens above its trigger, and below when there is no room above.
#[gpui::test]
fn it_opens_above_and_flips_below_at_the_top(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(Tooltip::new(
                "top",
                "At the top",
                Button::new("a").label("A"),
            ))
            .child(v_flex().pt(px(200.0)).child(Tooltip::new(
                "low",
                "Further down",
                Button::new("b").label("B"),
            )))
    });
    reduce_motion();
    press(cx, "tab");
    redraw(cx);
    let (button, tip) = (
        cx.debug_bounds("a/button").unwrap(),
        cx.debug_bounds("top/tip").unwrap(),
    );
    assert!(
        tip.top() >= button.bottom(),
        "no room above: below the button: {button:?} {tip:?}"
    );
    press(cx, "tab");
    redraw(cx);
    let (button, tip) = (
        cx.debug_bounds("b/button").unwrap(),
        cx.debug_bounds("low/tip").unwrap(),
    );
    assert!(tip.bottom() <= button.top(), "above the button");
}

/// Item 23: a long text wraps at the tooltip's width and stays inside a small window.
#[gpui::test]
fn a_long_text_wraps_inside_a_small_window(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().pt(px(200.0)).child(Tooltip::new(
            "tt",
            "Speichert alle geöffneten Dokumente und synchronisiert sie mit https://cloud.example.com/aleksandra.wisniewska-kowalczyk",
            Button::new("save").label("Save"),
        ))
    });
    reduce_motion();
    cx.simulate_resize(gpui::size(px(220.0), px(640.0)));
    press(cx, "tab");
    redraw(cx);
    let tip = cx.debug_bounds("tt/tip").unwrap();
    assert!(
        tip.left() >= px(0.0) && tip.right() <= px(220.0),
        "inside the window: {tip:?}"
    );
    assert!(
        tip.size.height
            > px(crate::foundation()
                .typography
                .style(crate::components().tooltip().geometry.body)
                .line_height
                * 2.0),
        "the text wrapped"
    );
}

/// The panel sits right against its trigger: a tooltip just above its button, with the gap of
/// its margin, and the wrapper is exactly as wide as the button, so nothing beside the button
/// opens it.
#[gpui::test]
fn it_sits_right_above_its_button(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().pt(px(200.0)).child(Tooltip::new(
            "tt",
            "Save the file",
            Button::new("save").label("Save"),
        ))
    });
    reduce_motion();
    press(cx, "tab");
    redraw(cx);
    let (button, tip, wrapper) = (
        cx.debug_bounds("save/button").unwrap(),
        cx.debug_bounds("tt/tip").unwrap(),
        cx.debug_bounds("tt/trigger").unwrap(),
    );
    let gap = px(crate::foundation().spacing.s1);
    assert_eq!(tip.bottom() + gap, button.top(), "{tip:?} above {button:?}");
    assert_eq!(tip.left(), button.left());
    assert_eq!(
        wrapper.size, button.size,
        "the wrapper is as wide as the button"
    );
}

/// Item 33: a tooltip whose button has focus inside a dialog is drawn above the dialog.
#[gpui::test]
fn a_tooltip_in_a_dialog(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let cx = dialog_around(cx, Count::default(), || {
        Tooltip::new("tt", "Save the file", Button::new("save").label("Save"))
    });
    let at = centre(cx, "save/button");
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(shown(cx), Some(1.0), "focus on the button opened it");
}
