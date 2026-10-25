//! Keeping the focused control on screen (WCAG 2.4.11).
//!
//! A dialog or sheet body scrolls the focused control into view. A non-modal sheet and a toast
//! are author content that can sit over what is focused: if one would hide that control
//! completely, it shifts by the smallest move that leaves a sliver of the control visible.
//! An overlay reads the focused bounds before its own content is prepainted, so it moves for
//! a control behind it and not for one inside it.

use std::cell::Cell;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Point, ScrollHandle,
    Stateful, StatefulInteractiveElement, Styled, Window, div, point, px,
};

use crate::components;

/// A move smaller than this is already where it should be. ODK's own, half a pixel.
const SCROLL_SLOP: f32 = 0.5;

thread_local! {
    /// How many reveal-scroll bodies the element being prepainted is inside.
    static REVEAL: Cell<usize> = const { Cell::new(0) };
    /// The focused control's bounds on this frame. Later overlays read what was noted before them.
    static FOCUSED: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    /// The scroll offset last asked for, so a clamp that cannot move any further does not ask again.
    static TRIED: Cell<Option<Point<Pixels>>> = const { Cell::new(None) };
}

/// Drops the focused bounds recorded on the previous frame. Called once, at the start of the
/// window's render, before anything is prepainted.
pub(crate) fn begin_frame() {
    FOCUSED.with(|c| c.set(None));
}

/// Notes `bounds` as the focused control, and asks a reveal-scroll body around it to bring it
/// into view. A later overlay reads the bounds noted before its own content is prepainted.
pub(crate) fn see_focus(bounds: Bounds<Pixels>, window: &mut Window) {
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }
    if REVEAL.with(|c| c.get()) > 0 {
        window.request_autoscroll(bounds);
    }
    FOCUSED.with(|c| c.set(Some(bounds)));
}

struct Entered {
    slot: &'static std::thread::LocalKey<Cell<usize>>,
    previous: usize,
}

impl Drop for Entered {
    fn drop(&mut self) {
        self.slot.with(|c| c.set(self.previous));
    }
}

fn enter(slot: &'static std::thread::LocalKey<Cell<usize>>) -> Entered {
    let previous = slot.with(|c| c.replace(c.get() + 1));
    Entered { slot, previous }
}

/// Counts as inside a reveal-scroll body while `child` is prepainted, so the focused control
/// there asks to be scrolled into view.
struct RevealScope(AnyElement);

impl Element for RevealScope {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.0.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let _entered = enter(&REVEAL);
        self.0.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(window, cx);
    }
}

impl IntoElement for RevealScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// A column that scrolls vertically and brings the focused control inside it into view.
/// The scroll offset is applied on the frame after the focus moves: the focused bounds are
/// known only once the children have been prepainted, and GPUI has already placed them.
pub(crate) fn scrolling_body(
    id: impl Into<ElementId>,
    scroll: ScrollHandle,
    child: impl IntoElement,
) -> Stateful<gpui::Div> {
    let handle = scroll.clone();
    div()
        .on_children_prepainted(move |_, window, cx| follow_focus(&handle, window, cx))
        .id(id.into())
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .overflow_y_scroll()
        .track_scroll(&scroll)
        .child(RevealScope(child.into_any_element()))
}

fn follow_focus(handle: &ScrollHandle, window: &mut Window, cx: &mut App) {
    let Some(focus) = window.take_autoscroll() else {
        return;
    };
    let viewport = handle.bounds();
    if viewport.size.height <= px(0.0) {
        return;
    }
    let offset = handle.offset();
    let next = reveal_offset(viewport, focus, offset);
    if (next.y - offset.y).abs() <= px(SCROLL_SLOP) {
        TRIED.with(|t| t.set(None));
        return;
    }
    if TRIED.with(|t| t.get()) == Some(next) {
        return;
    }
    TRIED.with(|t| t.set(Some(next)));
    let handle = handle.clone();
    window.defer(cx, move |window, _| {
        handle.set_offset(next);
        window.refresh();
    });
}

/// The scroll offset that brings `focus` inside `viewport`. `focus` and `viewport` are where
/// they are painted on this frame, which already includes `offset`.
fn reveal_offset(
    viewport: Bounds<Pixels>,
    focus: Bounds<Pixels>,
    offset: Point<Pixels>,
) -> Point<Pixels> {
    let above = focus.top() < viewport.top();
    let below = focus.bottom() > viewport.bottom();
    let dy = if above {
        viewport.top() - focus.top()
    } else if below {
        viewport.bottom() - focus.bottom()
    } else {
        px(0.0)
    };
    point(offset.x, (offset.y + dy).min(px(0.0)))
}

/// Shifts `child` so it does not completely cover the focused control in the page.
pub(crate) struct Uncover(AnyElement);

impl Uncover {
    pub(crate) fn new(child: AnyElement) -> Self {
        Self(child)
    }
}

impl Element for Uncover {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.0.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let sliver = px(components().foundation().geometry.border_width);
        let shift = FOCUSED.with(|c| {
            c.get()
                .map(|focus| shift_to_reveal(bounds, focus, sliver))
                .unwrap_or_default()
        });
        window.with_element_offset(shift, |window| {
            self.0.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // The shift was applied while prepainting, which is when the child's bounds were taken.
        self.0.paint(window, cx);
    }
}

impl IntoElement for Uncover {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn covers(overlay: Bounds<Pixels>, focus: Bounds<Pixels>) -> bool {
    focus.top() >= overlay.top()
        && focus.bottom() <= overlay.bottom()
        && focus.left() >= overlay.left()
        && focus.right() <= overlay.right()
}

/// The smallest move of `overlay` that leaves `sliver` of `focus` outside it, or none when
/// `focus` is not completely inside `overlay`.
fn shift_to_reveal(
    overlay: Bounds<Pixels>,
    focus: Bounds<Pixels>,
    sliver: Pixels,
) -> Point<Pixels> {
    if focus.size.width <= px(0.0) || focus.size.height <= px(0.0) || !covers(overlay, focus) {
        return point(px(0.0), px(0.0));
    }
    let candidates = [
        point(focus.left() - overlay.left() + sliver, px(0.0)),
        point(focus.right() - overlay.right() - sliver, px(0.0)),
        point(px(0.0), focus.top() - overlay.top() + sliver),
        point(px(0.0), focus.bottom() - overlay.bottom() - sliver),
    ];
    candidates
        .into_iter()
        .min_by(|a, b| {
            let da = a.x.abs() + a.y.abs();
            let db = b.x.abs() + b.y.abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, point, px, size};

    #[test]
    fn a_control_already_in_view_is_not_scrolled() {
        let viewport = Bounds::new(point(px(0.0), px(40.0)), size(px(200.0), px(100.0)));
        let focus = Bounds::new(point(px(8.0), px(50.0)), size(px(40.0), px(24.0)));
        let offset = point(px(0.0), px(-10.0));
        assert_eq!(reveal_offset(viewport, focus, offset), offset);
    }

    #[test]
    fn a_control_below_the_body_is_scrolled_up_into_it() {
        let viewport = Bounds::new(point(px(0.0), px(40.0)), size(px(200.0), px(100.0)));
        let focus = Bounds::new(point(px(8.0), px(200.0)), size(px(40.0), px(34.0)));
        let next = reveal_offset(viewport, focus, point(px(0.0), px(0.0)));
        assert!(next.y < px(0.0));
        let moved = focus.origin.y + next.y;
        assert_eq!(moved + focus.size.height, viewport.bottom());
    }

    #[test]
    fn a_covered_control_is_shifted_clear_by_the_smallest_move() {
        let overlay = Bounds::new(point(px(400.0), px(500.0)), size(px(336.0), px(80.0)));
        let focus = Bounds::new(point(px(420.0), px(520.0)), size(px(80.0), px(34.0)));
        let shift = shift_to_reveal(overlay, focus, px(1.0));
        let moved = Bounds::new(
            point(overlay.origin.x + shift.x, overlay.origin.y + shift.y),
            overlay.size,
        );
        assert!(!covers(moved, focus));
        assert!(shift.x.abs() + shift.y.abs() <= px(22.0));
    }

    #[test]
    fn a_control_that_already_peeks_out_is_left_where_it_is() {
        let overlay = Bounds::new(point(px(400.0), px(500.0)), size(px(336.0), px(80.0)));
        let focus = Bounds::new(point(px(380.0), px(520.0)), size(px(80.0), px(34.0)));
        assert_eq!(
            shift_to_reveal(overlay, focus, px(1.0)),
            point(px(0.0), px(0.0))
        );
    }
}
