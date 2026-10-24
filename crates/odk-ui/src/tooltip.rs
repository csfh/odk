use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};

use crate::focus::{child_id, part_name};
use crate::hover_card::{Hover, render_hover};
use crate::layout::{StyledExt, inside_window};
use crate::overlay::Transition;
use crate::placement::{Align, Placement};
use crate::{components, foundation};
use odk_theme::foundation::MotionDuration;

/// The widest a tooltip grows before its text wraps, in logical pixels. ODK's own (ADR 0035).
const TOOLTIP_MAX_WIDTH: f32 = 240.0;

/// A short text that explains its trigger, shown above it (below when there is no room), on the
/// tooltip's component token (`ComponentTokens::tooltip`).
///
/// It opens after the pointer rests on the trigger for the foundation's `hover_open`, and at once
/// when the trigger, or anything in it, has keyboard focus; Escape closes it. It stays open while
/// the pointer is over it, and closes `hover_close` after the pointer leaves both (WCAG 1.4.13).
/// It never takes focus, adds no tab stop and catches no click meant for the trigger: the
/// trigger, usually a button, is the one that gets focus. It appears over the foundation's
/// `fast` and goes over its `exit`, at once with reduced motion. A tooltip says what a control
/// does; anything the user needs to finish a task belongs in visible text or a popover.
#[derive(IntoElement)]
pub struct Tooltip {
    id: ElementId,
    text: SharedString,
    trigger: AnyElement,
    placement: Placement,
    align: Align,
}

impl Tooltip {
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        trigger: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            trigger: trigger.into_any_element(),
            placement: Placement::Top,
            align: Align::Start,
        }
    }

    /// The side of the trigger it prefers; above by default.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which edge of the trigger it lines up with; the start by default.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

impl Describe for Tooltip {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Tooltip, self.text.to_string())
    }
}

impl RenderOnce for Tooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let text = self.text;
        render_hover(
            Hover {
                id: self.id,
                trigger: self.trigger,
                tab_stop: false,
                placement: self.placement,
                align: self.align,
                transition: Transition::of(MotionDuration::Fast, MotionDuration::Exit),
                panel: Box::new(move |window, _| {
                    let style = components().tooltip();
                    let g = style.geometry;
                    let viewport = window.viewport_size().width;
                    div()
                        .id(child_id(&id, "tip"))
                        .debug_selector(|| part_name(&id, "tip"))
                        .px(px(g.padding_x))
                        .py(px(g.padding_y))
                        // Never wider than the window, less the margin it is kept from the edges.
                        .max_w(
                            px(TOOLTIP_MAX_WIDTH)
                                .min(inside_window(viewport, px(foundation().spacing.s1))),
                        )
                        .treatment(&style.surface)
                        .border(px(g.border_width))
                        .rounded(px(g.radius))
                        .text_role(g.body)
                        .child(text)
                }),
            },
            window,
            cx,
        )
    }
}
