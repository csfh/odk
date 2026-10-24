//! What every component on component tokens shares for interaction
//! (`docs/spec/odk-component-tokens.md`): its colours under the pointer, its focus ring, and
//! transitions that its own state drives. A component asks its style for `colours`, `responds`,
//! `focus` and `focus_visible` and hands them here, so none writes `.hover()`, `.active()` or a
//! ring of its own.
//!
//! Hover and press change colour at once. GPUI 0.2.2 applies `.hover()` and `.active()` styles
//! instantly and has no style transitions; tracking the pointer in ODK instead would give up
//! GPUI's own release of a press outside the control, and its hover listener reports no hover
//! while a button is held, so a fade built on it would flicker on every press (`docs/checks.md`).
//! What a component's own state drives, such as a button starting to load, moves with the
//! foundation's motion tokens through [`progress`], and a value such as a progress bar's through
//! [`tween`].

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, BorderStyle, Bounds, ContentMask, Corners, Div, ElementId, IntoElement, Pixels,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, TextStyleRefinement,
    UnderlineStyle, Window, canvas, div, px, quad, transparent_black,
};
use odk_theme::component::{FocusRing, Interactive, LinkStyle};
use odk_theme::foundation::{MotionDuration, MotionEasing};
use odk_theme::semantic::Treatment;

use crate::layout::{StyledExt, bounds_probe};
use crate::overlay::Presence;
use crate::theme::colour;

/// What the pointer has to be over for a control's hover and pressed colours to show.
pub(crate) enum Pointer {
    /// The control itself.
    Own,
    /// A larger element around it that takes the pointer for it, named with GPUI's `group`.
    Group(SharedString),
}

/// Puts `colours` on `el`: its resting treatment, and, when the control `responds`, its hover
/// and pressed treatments while `pointer` is hovered or pressed. A control that does not respond
/// (disabled or loading) keeps its resting look under the pointer: the style's three treatments
/// are already equal then, and no hover or press is wired at all. The cursor belongs to the
/// element that takes the click.
///
/// Call it after everything else that sets `el`'s text style (its type role, an underline).
/// GPUI 0.2.2 replaces an element's whole text style with a hover or press refinement's, so a
/// refinement that set only the text colour would drop the type role: the text would change size
/// while pressed, and be painted at the wrong size while hovered (`docs/checks.md`). Each
/// refinement here carries the element's own text style with its state's colour.
pub(crate) fn pointer_states<E>(
    el: E,
    colours: &Interactive<Treatment>,
    responds: bool,
    pointer: &Pointer,
) -> E
where
    E: StatefulInteractiveElement + Styled,
{
    let mut el = el.treatment(&colours.default);
    if !responds {
        return el;
    }
    let text = el.text_style().clone().unwrap_or_default();
    let state = move |t: Treatment| {
        let text = TextStyleRefinement {
            color: Some(colour(t.foreground).into()),
            ..text.clone()
        };
        move |mut s: StyleRefinement| {
            s = s.bg(colour(t.background)).border_color(colour(t.border));
            s.text = Some(text);
            s
        }
    };
    let (hover, pressed) = (state(colours.hover), state(colours.pressed));
    match pointer {
        Pointer::Own => el.hover(hover).active(pressed),
        Pointer::Group(group) => el
            .group_hover(group.clone(), hover)
            .group_active(group.clone(), pressed),
    }
}

/// A link's text colour and underline in each pointer state, as [`pointer_states`] puts a
/// control's colours: the resting ones, and when the link responds, those under the pointer and
/// while pressed, each refinement carrying the element's own text style. The underline is one
/// border width thick. Call it after everything else that sets `el`'s text style.
pub(crate) fn link_states<E>(el: E, s: &LinkStyle) -> E
where
    E: StatefulInteractiveElement + Styled,
{
    let thickness = px(crate::foundation().geometry.border_width);
    let underline = move |on: bool| {
        on.then_some(UnderlineStyle {
            thickness,
            ..Default::default()
        })
    };
    let mut el = el.text_color(colour(s.colours.default.foreground));
    el.text_style()
        .get_or_insert_with(Default::default)
        .underline = underline(s.underline.default);
    if !s.responds {
        return el;
    }
    let text = el.text_style().clone().unwrap_or_default();
    let state = move |foreground: odk_theme::Color, on: bool| {
        let text = TextStyleRefinement {
            color: Some(colour(foreground).into()),
            underline: underline(on),
            ..text.clone()
        };
        move |mut s: StyleRefinement| {
            s.text = Some(text);
            s
        }
    };
    el.hover(state(s.colours.hover.foreground, s.underline.hover))
        .active(state(s.colours.pressed.foreground, s.underline.pressed))
}

/// The focus ring of a control with `border` wide borders and corners of `radius`: `ring.width`
/// wide in `ring.colour`, `ring.offset` outside the control, so a gap of the surface lies between
/// it and the control's own fill. It keeps the control's corner radius rather than growing one
/// of its own, since no rectangle is rounder than the application's radius. Absolutely
/// positioned, so it takes no space and moves nothing (checklist item 32), and it has no
/// listeners, so it takes no clicks. Add it to a `relative` control that does not clip.
pub(crate) fn focus_ring(ring: &FocusRing, radius: f32, border: f32) -> Div {
    let outside = px(-(border + ring.offset + ring.width));
    div()
        .absolute()
        .top(outside)
        .left(outside)
        .right(outside)
        .bottom(outside)
        .border(px(ring.width))
        .border_color(colour(ring.colour))
        .rounded(px(radius))
}

/// Where the focused item of a composite control was laid out on this frame, so that its focus
/// ring can be painted from outside the item: over the items after it, whose fills would cover a
/// ring drawn as the item's own child, and past a row that clips its items, such as a scrolling
/// tab list or a breadcrumb whose labels truncate. One per render; the focused item takes its
/// [`RingTarget::probe`] and the control's outermost element, which must not clip, its
/// [`RingTarget::ring`] as its last child.
#[derive(Clone, Default)]
pub(crate) struct RingTarget {
    item: Rc<Cell<Option<Bounds<Pixels>>>>,
    clip: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl RingTarget {
    /// Records the border box of the element it is put in, a `relative` item, when it is laid
    /// out. GPUI lays out and prepaints the whole window before it paints any of it, so the ring
    /// painted later in the same frame is around where the item is now.
    pub(crate) fn probe(&self) -> impl IntoElement {
        let item = self.item.clone();
        bounds_probe(px(0.0), move |b, _| item.set(Some(b)))
    }

    /// Records the bounds of a scrolling list the item is in, so the ring of an item scrolled
    /// partly out of view is cut where the list is, with room for the ring around it.
    pub(crate) fn clip_probe(&self) -> impl IntoElement {
        let clip = self.clip.clone();
        bounds_probe(px(0.0), move |b, _| clip.set(Some(b)))
    }

    /// The ring around the recorded item, as [`focus_ring`] draws one: `ring.width` wide,
    /// `ring.offset` outside the item, with the item's corners of `radius`. A layer of no size
    /// that paints after everything before it in its parent, takes no space, moves nothing and
    /// takes no clicks. Nothing is painted when no item was recorded.
    pub(crate) fn ring(&self, ring: FocusRing, radius: f32, part: &ElementId) -> impl IntoElement {
        let (item, clip) = (self.item.clone(), self.clip.clone());
        #[cfg(test)]
        let part = crate::focus::id_path(part);
        #[cfg(not(test))]
        let _ = part;
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let Some(bounds) = item.get() else {
                    return;
                };
                let reach = px(ring.offset + ring.width);
                let around = bounds.dilate(reach);
                #[cfg(test)]
                painted::record(&part, around);
                let paint = |window: &mut Window| {
                    window.paint_quad(quad(
                        around,
                        Corners::all(px(radius)),
                        transparent_black(),
                        px(ring.width),
                        colour(ring.colour),
                        BorderStyle::Solid,
                    ))
                };
                match clip.get() {
                    Some(list) => window.with_content_mask(
                        Some(ContentMask {
                            bounds: list.dilate(reach),
                        }),
                        paint,
                    ),
                    None => paint(window),
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_0()
    }
}

/// The rings each composite control painted on its last frame, for tests, by the control's id.
#[cfg(test)]
pub(crate) mod painted {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use gpui::{Bounds, Pixels};

    thread_local! {
        static PAINTED: RefCell<HashMap<String, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(part: &str, ring: Bounds<Pixels>) {
        PAINTED.with(|p| p.borrow_mut().insert(part.to_owned(), ring));
    }

    /// The outer edge of the ring last painted for the control with id `id`, and forgets it, so
    /// a test can tell a frame that painted none.
    pub(crate) fn take(id: &str) -> Option<Bounds<Pixels>> {
        PAINTED.with(|p| p.borrow_mut().remove(id))
    }
}

/// A value moving toward a target, eased, from wherever it was when the target changed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Tween {
    from: f32,
    to: f32,
    since: Option<Instant>,
}

impl Tween {
    fn settled(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            since: None,
        }
    }

    /// The value at `now`.
    fn value(&self, now: Instant, length: Duration, easing: MotionEasing) -> f32 {
        let Some(since) = self.since else {
            return self.to;
        };
        if length.is_zero() {
            return self.to;
        }
        let t = now.saturating_duration_since(since).as_secs_f32() / length.as_secs_f32();
        if t >= 1.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * easing.apply(t)
    }

    /// Heads for `target` from where it is at `now`, when that is a new target.
    fn set(&mut self, target: f32, now: Instant, length: Duration, easing: MotionEasing) {
        if target == self.to {
            if self.since.is_some() && self.value(now, length, easing) == self.to {
                self.since = None;
            }
            return;
        }
        self.from = self.value(now, length, easing);
        self.to = target;
        self.since = Some(now);
    }
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// How many frames have been asked for on this thread, for the checklist and the benchmarks.
    static FRAME_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Asks for another frame on behalf of the view being drawn. Every request goes through here and
/// nothing else asks (a source scan, `checklist/source.rs`), so that tests can count them: GPUI's
/// test platform never runs the callbacks of the next frame, and a component that asks for one
/// forever is the bug it cannot show otherwise (checklist item 35).
///
/// A frame request re-renders every view from the window down to the one asking, at every frame
/// of the display, so ask only while something is moving, and say when it stops. Something that
/// changes on a schedule, as a spinner's glyph does, wakes its view with a timer instead.
pub(crate) fn request_frame(window: &Window) {
    #[cfg(any(test, feature = "test-support"))]
    FRAME_REQUESTS.with(|n| n.set(n.get() + 1));
    window.request_animation_frame();
}

/// The frames asked for on this thread since the last [`take_frame_requests`].
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn take_frame_requests() -> usize {
    FRAME_REQUESTS.with(|n| n.replace(0))
}

/// A value that follows `target` over the foundation's `duration` on its curve, for a component
/// whose own value moves, such as a progress bar's. It starts at its first target without moving;
/// a new target partway carries on from where it had got to (checklist item 22); time comes from
/// GPUI's clock (item 31); with reduced motion it is the target at once (item 20). Keeps frames
/// coming while it moves. `id` keys the state.
pub(crate) fn tween(
    id: ElementId,
    target: f32,
    duration: MotionDuration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let f = crate::foundation();
    let (length, easing) = (f.motion.transition(duration), f.motion.easing);
    let state = window.use_keyed_state(id, cx, |_, _| Tween::settled(target));
    let now = cx.background_executor().now();
    let value = state.update(cx, |t, _| {
        t.set(target, now, length, easing);
        t.value(now, length, easing)
    });
    if value != target {
        request_frame(window);
    }
    value
}

/// How far a component's own state has moved toward `on`, eased, from 0 (off) to 1 (on), over
/// the foundation's `duration`. Starts where the state is, without animating there; a change
/// reversed partway carries on from where it had got to (checklist item 22); time comes from
/// GPUI's clock (item 31); with reduced motion it is 0 or 1 at once (item 20). Keeps frames
/// coming while it moves. `id` keys the state, so give each transition of a component its own.
pub(crate) fn progress(
    id: ElementId,
    on: bool,
    duration: MotionDuration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let f = crate::foundation();
    let length = f.motion.transition(duration);
    let presence = window.use_keyed_state(id, cx, |_, _| Presence::settled(on));
    let now = cx.background_executor().now();
    let amount = presence.update(cx, |p, _| {
        p.set(on, now, length);
        p.amount(now, length).unwrap_or(0.0)
    });
    if (on && amount < 1.0) || (!on && amount > 0.0) {
        request_frame(window);
    }
    f.motion.easing.apply(amount)
}

#[cfg(test)]
mod tests {
    use gpui::{Length, Position, Styled, px};
    use odk_theme::component::{ButtonLayout, ButtonVariant, States};
    use odk_theme::foundation::Density;

    use crate::theme::colour;

    /// The ring lies `offset` outside the control's border, `width` wide, and keeps the
    /// control's own corner radius instead of growing a rounder one.
    #[test]
    fn the_ring_lies_outside_the_control_with_its_corners() {
        crate::theme::install_test_theme();
        let ring = crate::components()
            .button(
                ButtonVariant::Primary,
                Density::Default,
                ButtonLayout::Text,
                States::NONE,
            )
            .focus;
        let mut el = super::focus_ring(&ring, 2.0, 1.0);
        let s = el.style().clone();
        assert_eq!(s.position, Some(Position::Absolute));
        let outside = Some(Length::from(px(-(1.0 + ring.offset + ring.width))));
        assert_eq!(
            (s.inset.top, s.inset.right, s.inset.bottom, s.inset.left),
            (outside, outside, outside, outside)
        );
        assert_eq!(s.border_widths.left, Some(px(ring.width).into()));
        assert_eq!(s.border_color, Some(colour(ring.colour).into()));
        assert_eq!(s.corner_radii.top_left, Some(px(2.0).into()));
        assert_eq!(s.background, None, "a ring has no fill");
    }

    /// A tween starts where it is told without moving, eases to a new target over its length,
    /// carries on from where it is when the target changes partway, and is at its target at once
    /// with no length (reduced motion).
    #[test]
    fn a_tween_moves_from_where_it_is() {
        use std::time::{Duration, Instant};

        use odk_theme::foundation::MotionEasing;

        use super::Tween;

        let (easing, length) = (MotionEasing::Linear, Duration::from_millis(100));
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut t = Tween::settled(0.2);
        assert_eq!(
            t.value(start, length, easing),
            0.2,
            "no move to the first value"
        );
        t.set(1.0, at(0), length, easing);
        assert!((t.value(at(50), length, easing) - 0.6).abs() < 1e-5);
        // Reversed halfway: from 0.6, not from 1.0 or 0.2.
        t.set(0.0, at(50), length, easing);
        assert!(
            (t.value(at(50), length, easing) - 0.6).abs() < 1e-5,
            "no jump"
        );
        assert!((t.value(at(100), length, easing) - 0.3).abs() < 1e-5);
        assert_eq!(t.value(at(150), length, easing), 0.0);
        t.set(0.5, at(200), Duration::ZERO, easing);
        assert_eq!(
            t.value(at(200), Duration::ZERO, easing),
            0.5,
            "reduced: at once"
        );
    }
}
