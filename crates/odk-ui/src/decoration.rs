//! Decorations (ADR 0057): typed values attached to a component that change one behaviour
//! and leave the rest at that component's default. Truncation and overlay sizing are the first
//! use (ADR 0058).
//!
//! A component accepts only the decorations that apply to it, so a button cannot be given an
//! overlay size and a dialog can take both a text flow and a size. Two decorations compose when
//! they name different concerns. Two that name the same concern do not: the later one wins.

use gpui::{
    AnyElement, App, ElementId, IntoElement, Pixels, SharedString, Styled, Window, canvas, px,
};

use crate::Tooltip;
use crate::focus::child_id;
use crate::layout::{text_width, truncate_end, truncate_lines};

/// Applies `flow` to `el`. Middle ellipsis is drawn by the caller (the string is already
/// shortened); the element still stays on one line so a short measure cannot overflow.
pub fn apply_flow<E: Styled>(el: E, flow: FlowKind) -> E {
    match flow {
        FlowKind::Wrap => el,
        FlowKind::End | FlowKind::Middle => truncate_end(el),
        FlowKind::Clamp(n) => truncate_lines(el, n),
    }
}

/// How a component's text flows. The component's own default applies when none of these is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// Wrap onto as many lines as the text needs.
    Wrap,
    /// One line, ending in `…`.
    End,
    /// One line, with `…` in the middle. The caller opts in; nothing infers it.
    Middle,
    /// Wrap for at most this many lines, then end in `…`.
    Clamp(u8),
}

/// How an overlay takes its size. Capped at the window either way (ADR 0058).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeMode {
    /// This many logical pixels, still capped at the window.
    Fixed(f32),
    /// Grow with the text size and, where the component says so, with its content, capped at
    /// the window.
    Autosize,
}

/// What has been attached so far. Absent means the component's default.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Applied {
    flow: Option<FlowKind>,
    size: Option<SizeMode>,
}

impl Applied {
    /// The text flow to draw: the last text decoration, or `default`.
    pub fn flow(self, default: FlowKind) -> FlowKind {
        self.flow.unwrap_or(default)
    }

    /// The overlay size to draw: the last size decoration, or `default`.
    pub fn size(self, default: SizeMode) -> SizeMode {
        self.size.unwrap_or(default)
    }

    pub(crate) fn set_flow(&mut self, flow: FlowKind) {
        self.flow = Some(flow);
    }

    pub(crate) fn set_size(&mut self, size: SizeMode) {
        self.size = Some(size);
    }
}

/// A decoration that changes text flow. A component that only truncates accepts this and nothing
/// that sizes an overlay.
pub trait TextDecoration {
    fn apply_text(self, slot: &mut Applied);
}

/// A decoration that changes an overlay's size.
pub trait SizeDecoration {
    fn apply_size(self, slot: &mut Applied);
}

/// A decoration a dialog, a sheet or another overlay accepts: text flow or size. The two concerns
/// compose; a second decoration of the same concern replaces the first.
pub trait OverlayDecoration {
    fn apply_overlay(self, slot: &mut Applied);
}

impl OverlayDecoration for Wrap {
    fn apply_overlay(self, slot: &mut Applied) {
        self.apply_text(slot);
    }
}

impl OverlayDecoration for EndEllipsis {
    fn apply_overlay(self, slot: &mut Applied) {
        self.apply_text(slot);
    }
}

impl OverlayDecoration for MiddleEllipsis {
    fn apply_overlay(self, slot: &mut Applied) {
        self.apply_text(slot);
    }
}

impl OverlayDecoration for Clamp {
    fn apply_overlay(self, slot: &mut Applied) {
        self.apply_text(slot);
    }
}

impl OverlayDecoration for Autosize {
    fn apply_overlay(self, slot: &mut Applied) {
        self.apply_size(slot);
    }
}

impl OverlayDecoration for FixedWidth {
    fn apply_overlay(self, slot: &mut Applied) {
        self.apply_size(slot);
    }
}

/// Wrap, with no line cap. On a dialog or sheet title this lifts the default three-line cap.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wrap;

/// One line, ending in `…`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EndEllipsis;

/// One line, with `…` in the middle. Opt-in only (ADR 0058).
#[derive(Debug, Clone, Copy, Default)]
pub struct MiddleEllipsis;

/// Wrap for at most `n` lines, then end in `…`. `n` of 0 is one line.
#[derive(Debug, Clone, Copy)]
pub struct Clamp(pub u8);

/// Grow with the text size and the component's content, capped at the window.
#[derive(Debug, Clone, Copy, Default)]
pub struct Autosize;

/// An explicit size in logical pixels, still capped at the window.
#[derive(Debug, Clone, Copy)]
pub struct FixedWidth(pub f32);

impl TextDecoration for Wrap {
    fn apply_text(self, slot: &mut Applied) {
        slot.set_flow(FlowKind::Wrap);
    }
}

impl TextDecoration for EndEllipsis {
    fn apply_text(self, slot: &mut Applied) {
        slot.set_flow(FlowKind::End);
    }
}

impl TextDecoration for MiddleEllipsis {
    fn apply_text(self, slot: &mut Applied) {
        slot.set_flow(FlowKind::Middle);
    }
}

impl TextDecoration for Clamp {
    fn apply_text(self, slot: &mut Applied) {
        slot.set_flow(FlowKind::Clamp(self.0.max(1)));
    }
}

impl SizeDecoration for Autosize {
    fn apply_size(self, slot: &mut Applied) {
        slot.set_size(SizeMode::Autosize);
    }
}

impl SizeDecoration for FixedWidth {
    fn apply_size(self, slot: &mut Applied) {
        slot.set_size(SizeMode::Fixed(self.0));
    }
}

/// The width an overlay draws for `mode`: a fixed measure as given, or `base` multiplied by the
/// text scale, either one capped at `cap`.
pub(crate) fn sized_width(base: f32, mode: SizeMode, factor: f32, cap: Pixels) -> Pixels {
    let width = match mode {
        SizeMode::Fixed(px) => px,
        SizeMode::Autosize => base * factor,
    };
    px(width).min(cap)
}

/// Whether `text` at `size` did not fit `lines` times the box on the previous frame, and a probe
/// that measures it again this frame. The tooltip that follows is one frame behind the measure,
/// and a window is asked to draw again only when the answer changes (ADR 0058).
pub(crate) fn truncation_probe(
    id: &ElementId,
    text: SharedString,
    size: Pixels,
    lines: u8,
    window: &mut Window,
    cx: &mut App,
) -> (bool, impl IntoElement + 'static) {
    let state = window.use_keyed_state(child_id(id, "truncated"), cx, |_, _| false);
    let was = *state.read(cx);
    let flag = state.clone();
    let probe = canvas(
        move |bounds, window, cx| {
            let room = bounds.size.width * f32::from(lines.max(1));
            let over = !text.is_empty()
                && text_width(window, &text, size) > room + px(crate::layout::MEASURE_SLACK);
            let changed = flag.update(cx, |current, cx| {
                if *current == over {
                    false
                } else {
                    *current = over;
                    cx.notify();
                    true
                }
            });
            if changed {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top(px(0.0))
    .left(px(0.0))
    .right(px(0.0))
    .bottom(px(0.0));
    (was, probe)
}

/// The full `text` in a tooltip around `trigger` when `show` is set. The tooltip adds no tab
/// stop. `fill` makes the wrapper take the parent's width, so a control that ellipsizes because
/// it is width-bounded still is.
pub(crate) fn overflow_tooltip(
    id: &ElementId,
    text: impl Into<SharedString>,
    show: bool,
    fill: bool,
    trigger: impl IntoElement,
) -> AnyElement {
    let text = text.into();
    if !show || text.is_empty() {
        trigger.into_any_element()
    } else {
        let mut tip = Tooltip::new(child_id(id, "clip"), text, trigger);
        if fill {
            tip = tip.fill_width();
        }
        tip.into_any_element()
    }
}

/// How many lines a flow allows when measuring overflow. Middle ellipsis is one line; wrap is
/// never overflow.
pub(crate) fn flow_lines(flow: FlowKind) -> Option<u8> {
    match flow {
        FlowKind::Wrap => None,
        FlowKind::End | FlowKind::Middle => Some(1),
        FlowKind::Clamp(n) => Some(n.max(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_later_decoration_of_one_concern_wins_and_the_other_concern_stays() {
        let mut slot = Applied::default();
        Wrap.apply_overlay(&mut slot);
        EndEllipsis.apply_overlay(&mut slot);
        assert_eq!(slot.flow(FlowKind::Wrap), FlowKind::End);
        Clamp(3).apply_text(&mut slot);
        Wrap.apply_text(&mut slot);
        assert_eq!(slot.flow(FlowKind::End), FlowKind::Wrap);
        Autosize.apply_overlay(&mut slot);
        assert_eq!(
            slot.flow(FlowKind::End),
            FlowKind::Wrap,
            "size leaves the flow"
        );
        assert_eq!(slot.size(SizeMode::Fixed(1.0)), SizeMode::Autosize);
        FixedWidth(320.0).apply_size(&mut slot);
        Clamp(5).apply_overlay(&mut slot);
        assert_eq!(slot.size(SizeMode::Autosize), SizeMode::Fixed(320.0));
        assert_eq!(slot.flow(FlowKind::Wrap), FlowKind::Clamp(5));
        Clamp(0).apply_text(&mut slot);
        assert_eq!(slot.flow(FlowKind::Wrap), FlowKind::Clamp(1));
    }

    #[test]
    fn a_missing_decoration_is_the_component_default() {
        let slot = Applied::default();
        assert_eq!(slot.flow(FlowKind::Clamp(3)), FlowKind::Clamp(3));
        assert_eq!(slot.size(SizeMode::Fixed(480.0)), SizeMode::Fixed(480.0));
        assert_eq!(slot.size(SizeMode::Autosize), SizeMode::Autosize);
    }
}
