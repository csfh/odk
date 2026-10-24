use std::cell::Cell;
use std::rc::Rc;

use gpui::{Modifiers, ParentElement, Styled, TestAppContext, VisualTestContext, px};
use odk_a11y::{Describe, Role};

use super::{Count, open, press, redraw, reduce_motion};
use crate::overlay::probe;
use crate::{Align, Button, Placement, Popover, Text, h_flex, v_flex};

fn click(cx: &mut VisualTestContext, part: &'static str) {
    let at = cx.debug_bounds(part).unwrap().center();
    cx.simulate_click(at, Modifiers::none());
    redraw(cx);
}

/// A popover on a button, with a button inside, and a button beside it to click away on.
fn window(
    cx: &mut TestAppContext,
    inside: Count,
    beside: Count,
    tweak: impl Fn(Popover) -> Popover + 'static,
) -> &mut VisualTestContext {
    open(cx, move |_, _| {
        let (inside, beside) = (inside.clone(), beside.clone());
        h_flex()
            .child(tweak(
                Popover::new("pop", "Filter", Button::new("trigger").label("Filter"))
                    .title("Filter results")
                    .content(move |_, _| {
                        let (a, b) = (inside.clone(), inside.clone());
                        v_flex()
                            .child(
                                Button::new("one")
                                    .label("One")
                                    .on_click(move |_, _, _| a.bump()),
                            )
                            .child(
                                Button::new("two")
                                    .label("Two")
                                    .on_click(move |_, _, _| b.bump()),
                            )
                    }),
            ))
            .child(
                Button::new("beside")
                    .label("Beside")
                    .on_click(move |_, _, _| beside.bump()),
            )
    })
}

/// Items 14, 15.
#[test]
fn is_named_by_its_constructor() {
    let p = Popover::new("p", "Filter", Text::new("Filter"));
    let a = p.accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Dialog, "Filter"));
}

/// Items 8, 10, 13: Enter on the trigger opens it and moves focus into the panel; Tab walks its
/// controls and, being nonmodal, carries on past the last one out of the panel; Escape closes it
/// and focus goes back to the trigger, so Enter opens it again.
#[gpui::test]
fn opens_from_the_keyboard_and_gives_focus_back(cx: &mut TestAppContext) {
    let (inside, beside) = (Count::default(), Count::default());
    let cx = window(cx, inside.clone(), beside.clone(), |p| p);
    reduce_motion();
    press(cx, "tab enter");
    redraw(cx);
    assert_eq!(probe::last("pop"), Some(1.0), "Enter opened it");
    // Focus is in the panel: the first Tab reaches its first control.
    press(cx, "tab enter");
    assert_eq!(
        inside.get(),
        1,
        "Tab went to the first control in the panel"
    );
    press(cx, "tab enter");
    assert_eq!(inside.get(), 2, "and on to the second");
    press(cx, "tab enter");
    assert_eq!(beside.get(), 1, "Tab is not trapped: it left the panel");
    assert_eq!(
        probe::last("pop"),
        Some(1.0),
        "and the popover is still open"
    );
    press(cx, "shift-tab shift-tab shift-tab escape");
    redraw(cx);
    assert_eq!(probe::last("pop"), None, "Escape closed it");
    press(cx, "enter");
    redraw(cx);
    assert_eq!(
        probe::last("pop"),
        Some(1.0),
        "focus went back to the trigger"
    );
}

/// A press on the trigger opens it and a second press closes it.
#[gpui::test]
fn the_trigger_toggles_it(cx: &mut TestAppContext) {
    let cx = window(cx, Count::default(), Count::default(), |p| p);
    reduce_motion();
    click(cx, "trigger/button");
    assert_eq!(probe::last("pop"), Some(1.0));
    click(cx, "trigger/button");
    assert_eq!(probe::last("pop"), None);
}

/// The content is interactive: a click on a control in the panel reaches it and leaves the
/// popover open.
#[gpui::test]
fn its_content_takes_clicks(cx: &mut TestAppContext) {
    let inside = Count::default();
    let cx = window(cx, inside.clone(), Count::default(), |p| p);
    reduce_motion();
    click(cx, "trigger/button");
    click(cx, "one/button");
    assert_eq!(inside.get(), 1);
    assert_eq!(probe::last("pop"), Some(1.0), "it stayed open");
}

/// Dismissal: a press outside closes it and presses nothing under it.
#[gpui::test]
fn an_outside_press_closes_it_and_goes_no_further(cx: &mut TestAppContext) {
    let beside = Count::default();
    let cx = window(cx, Count::default(), beside.clone(), |p| p);
    reduce_motion();
    click(cx, "trigger/button");
    click(cx, "beside/button");
    assert_eq!(probe::last("pop"), None, "closed");
    assert_eq!(beside.get(), 0, "the press was the popover's alone");
}

/// With outside clicks off the popover stays and the window behind it keeps working.
#[gpui::test]
fn with_outside_clicks_off_the_window_behind_stays_live(cx: &mut TestAppContext) {
    let beside = Count::default();
    let cx = window(cx, Count::default(), beside.clone(), |p| {
        p.close_on_outside_click(false)
    });
    reduce_motion();
    click(cx, "trigger/button");
    click(cx, "beside/button");
    assert_eq!(beside.get(), 1, "the button behind was pressed");
    assert_eq!(probe::last("pop"), Some(1.0), "and the popover stayed");
}

/// Dismissal: Escape can be turned off.
#[gpui::test]
fn escape_can_be_turned_off(cx: &mut TestAppContext) {
    let cx = window(cx, Count::default(), Count::default(), |p| {
        p.close_on_escape(false)
    });
    reduce_motion();
    press(cx, "tab enter");
    redraw(cx);
    press(cx, "escape");
    redraw(cx);
    assert_eq!(probe::last("pop"), Some(1.0));
}

/// Controlled: the application owns `open`; the popover only asks, once per request.
#[gpui::test]
fn a_controlled_popover_asks_and_the_application_decides(cx: &mut TestAppContext) {
    let wanted = Rc::new(Cell::new(None::<bool>));
    let asks = Count::default();
    let (w, a) = (wanted.clone(), asks.clone());
    let cx = open(cx, move |_, _| {
        let (w, a) = (w.clone(), a.clone());
        Popover::new("pop", "Filter", Button::new("trigger").label("Filter"))
            .open(false)
            .on_open_change(move |want, _, _| {
                w.set(Some(want));
                a.bump();
            })
            .content(|_, _| Text::new("Body"))
    });
    reduce_motion();
    click(cx, "trigger/button");
    assert_eq!(wanted.get(), Some(true), "it asked to open");
    assert_eq!(asks.get(), 1, "once");
    assert_eq!(probe::last("pop"), None, "and stayed closed, as told");
}

/// The preferred side, flipped when there is no room: below a trigger at the top, above one at
/// the bottom of the window; and it never covers the trigger.
#[gpui::test]
fn it_flips_to_the_side_that_has_room(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex()
            .child(
                Popover::new("top", "Top", Button::new("a").label("A"))
                    .default_open(true)
                    .content(|_, _| Text::new("Under the button")),
            )
            .child(
                v_flex().pt(px(700.0)).child(
                    Popover::new("low", "Low", Button::new("b").label("B"))
                        .default_open(true)
                        .content(|_, _| Text::new("Over the button")),
                ),
            )
    });
    reduce_motion();
    redraw(cx);
    let (a, pa) = (
        cx.debug_bounds("a/button").unwrap(),
        cx.debug_bounds("top/popover").unwrap(),
    );
    assert!(pa.top() >= a.bottom(), "below the first: {a:?} {pa:?}");
    let (b, pb) = (
        cx.debug_bounds("b/button").unwrap(),
        cx.debug_bounds("low/popover").unwrap(),
    );
    assert!(
        pb.bottom() <= b.top() || pb.top() >= b.bottom(),
        "never over the second: {b:?} {pb:?}"
    );
}

/// Placement beside the trigger and the end alignment.
#[gpui::test]
fn it_can_sit_beside_its_trigger_or_end_where_it_ends(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        v_flex().pt(px(200.0)).pl(px(300.0)).child(
            Popover::new("p", "Side", Button::new("t").label("Trigger"))
                .default_open(true)
                .placement(Placement::Right)
                .align(Align::End)
                .content(|_, _| Text::new("Beside")),
        )
    });
    reduce_motion();
    redraw(cx);
    let (t, p) = (
        cx.debug_bounds("t/button").unwrap(),
        cx.debug_bounds("p/popover").unwrap(),
    );
    assert!(p.left() >= t.right(), "to the right: {t:?} {p:?}");
    assert_eq!(p.bottom(), t.bottom(), "ending where the trigger ends");
}

/// Item 33: inside a dialog it is drawn above it, and Escape closes the popover alone.
#[gpui::test]
fn a_popover_in_a_dialog_closes_alone_on_escape(cx: &mut TestAppContext) {
    use super::dialog::dialog_around;
    let closed = Count::default();
    let cx = dialog_around(cx, closed.clone(), || {
        Popover::new("p", "Details", Button::new("t").label("Details"))
            .title("Details")
            .content(|_, _| Text::new("More about it."))
    });
    click(cx, "t/button");
    assert!(
        cx.debug_bounds("p/popover").is_some(),
        "drawn above the dialog"
    );
    press(cx, "escape");
    redraw(cx);
    assert_eq!(probe::last("p"), None, "the popover closed");
    assert_eq!(closed.get(), 0, "the dialog did not");
}

/// Item 34: padding and gaps are the popover token's.
#[gpui::test]
fn it_is_spaced_by_the_popover_token(cx: &mut TestAppContext) {
    let cx = window(cx, Count::default(), Count::default(), |p| {
        p.default_open(true)
    });
    reduce_motion();
    redraw(cx);
    let g = crate::components().popover().geometry;
    let (panel, title) = (
        cx.debug_bounds("pop/popover").unwrap(),
        cx.debug_bounds("pop/title").unwrap(),
    );
    assert_eq!(
        title.left() - panel.left(),
        px(g.border_width + g.padding_x)
    );
    assert_eq!(title.top() - panel.top(), px(g.border_width + g.padding_y));
}

/// Item 32: moving through an open popover, by keyboard and pointer, moves nothing in it, and
/// opening it moves nothing around it.
#[gpui::test]
fn no_state_moves_anything(cx: &mut TestAppContext) {
    use super::stability::{Step, assert_still};
    let cx = window(cx, Count::default(), Count::default(), |p| p);
    reduce_motion();
    assert_still(
        cx,
        &["trigger/button", "beside/button"],
        &[Step::Keys("tab enter")],
    );
    assert_still(
        cx,
        &[
            "pop/popover",
            "pop/title",
            "one/button",
            "two/button",
            "trigger/button",
            "beside/button",
        ],
        &[
            Step::Keys("tab"),
            Step::Keys("tab"),
            Step::Hover("one/button"),
            Step::Hover("two/button"),
            Step::Press("two/button"),
            Step::Keys("shift-tab"),
        ],
    );
}
