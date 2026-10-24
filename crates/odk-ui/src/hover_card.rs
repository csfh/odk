use gpui::{
    AnyElement, App, Bounds, Div, ElementId, Entity, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, RenderOnce, Stateful, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::foundation::MotionDuration;

use crate::focus::{child_id, focus_handle, part_name};
use crate::interaction::focus_ring;
use crate::layout::{StyledExt, bounds_probe, inside_window};
use crate::overlay::{Layer, Transition, layer_over, lift};
use crate::placement::{Align, Placement, place};
use crate::{components, foundation};

type Content = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement + 'static>;
pub(crate) type Panel = Box<dyn FnOnce(&mut Window, &mut App) -> Stateful<Div> + 'static>;

/// The widest a card grows before its content wraps, in logical pixels. ODK's own (ADR 0035).
const CARD_MAX_WIDTH: f32 = 336.0;

/// A panel below a trigger (or on another side, see [`HoverCard::placement`]) that opens after
/// the pointer rests on the trigger for the foundation's `hover_open`, and closes `hover_close`
/// after it leaves both the trigger and the panel, so the pointer can move into the panel and
/// use it. Focusing the trigger opens it at once, and Escape closes it. It is on the popover's
/// component token (`ComponentTokens::popover`), goes to the other side of the trigger when it
/// has no room on its own, and never takes focus; for content the user works in, with a focus of
/// its own, use a `Popover`.
#[derive(IntoElement)]
pub struct HoverCard {
    id: ElementId,
    trigger: AnyElement,
    content: Option<Content>,
    name: String,
    placement: Placement,
    align: Align,
}

impl HoverCard {
    /// A card named `name` for assistive technology, such as "About Omarchy", shown for
    /// `trigger`; the name is required because the card has no visible label of its own.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<String>,
        trigger: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into_any_element(),
            content: None,
            name: name.into(),
            placement: Placement::Bottom,
            align: Align::Start,
        }
    }

    /// The side of the trigger it prefers; below by default.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which edge of the trigger it lines up with; the start by default.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Builds the panel's content; it runs only while the card is shown.
    pub fn content<E: IntoElement>(
        mut self,
        build: impl FnOnce(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.content = Some(Box::new(move |w, cx| build(w, cx).into_any_element()));
        self
    }
}

impl Describe for HoverCard {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Tooltip, self.name.clone())
    }
}

/// A hover panel, as a hover card or a tooltip draws one.
pub(crate) struct Hover {
    pub id: ElementId,
    pub trigger: AnyElement,
    /// Whether the trigger is a tab stop of its own. A tooltip's trigger is usually focusable
    /// itself, so its wrapper is not.
    pub tab_stop: bool,
    pub placement: Placement,
    pub align: Align,
    /// How long it takes to appear and to go.
    pub transition: Transition,
    /// The panel, styled and filled, built while it is shown.
    pub panel: Panel,
}

#[derive(Default)]
struct HoverState {
    over_trigger: bool,
    over_panel: bool,
    /// Shown because of the pointer, after the delays.
    open: bool,
    /// Escape closes the panel until focus or the pointer leaves.
    dismissed: bool,
    /// Bumped on every pointer change, so a stale timer does nothing.
    generation: u64,
    trigger: Bounds<Pixels>,
}

/// Whether the panel should be open once the pointer has settled.
fn wanted(s: &HoverState) -> bool {
    s.over_trigger || s.over_panel
}

/// Records a pointer change and, after the matching delay, opens or closes the panel.
fn pointer_changed(
    state: &Entity<HoverState>,
    update: impl FnOnce(&mut HoverState),
    window: &mut Window,
    cx: &mut App,
) {
    let (generation, want) = state.update(cx, |s, _| {
        update(s);
        if !wanted(s) {
            s.dismissed = false;
        }
        s.generation += 1;
        (s.generation, wanted(s))
    });
    let motion = &foundation().motion;
    let delay = if want {
        motion.hover_open
    } else {
        motion.hover_close
    };
    let state = state.clone();
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(delay).await;
            let _ = state.update(cx, |s, cx| {
                if s.generation == generation && s.open != want {
                    s.open = want;
                    cx.notify();
                }
            });
        })
        .detach();
}

/// Draws a trigger with its hover panel. The panel opens after the pointer rests on the
/// trigger for `motion.hover_open` and closes `motion.hover_close` after it leaves both the
/// trigger and the panel, so the pointer can move into the panel (WCAG 1.4.13). Focus on or
/// inside the trigger opens it at once, and Escape closes it. It does not take focus.
pub(crate) fn render_hover(hover: Hover, window: &mut Window, cx: &mut App) -> AnyElement {
    let c = components();
    let f = c.foundation().clone();
    let Hover {
        id,
        trigger,
        tab_stop,
        placement,
        align,
        transition,
        panel,
    } = hover;
    let state = window.use_keyed_state(child_id(&id, "state"), cx, |_, _| HoverState::default());
    let focus = focus_handle(child_id(&id, "trigger"), tab_stop, window, cx);
    let focused = focus.contains_focused(window, cx);
    let (open, at) = {
        let s = state.read(cx);
        ((s.open || focused) && !s.dismissed, s.trigger)
    };

    // Not modal, and it does not take focus: the pointer or the focused trigger opened it.
    let frame = layer_over(child_id(&id, "layer"), open, false, transition, window, cx);
    let overlay = frame.amount.map(|amount| {
        let hover_state = state.clone();
        let panel = panel(window, cx)
            .opacity(f.motion.easing.apply(amount))
            .occlude()
            .on_hover(move |over, window, cx| {
                let over = *over;
                pointer_changed(&hover_state, |s| s.over_panel = over, window, cx)
            });
        let placed = place(placement, align, at).wrap(px(f.spacing.s1), panel);
        lift(&id, Layer::Popover, placed, window, cx)
    });

    let (hover_state, key_state) = (state.clone(), state.clone());
    let probe_state = state;
    let mut wrapper = div()
        .id(id.clone())
        .debug_selector(|| part_name(&id, "trigger"))
        .relative()
        .flex()
        .track_focus(&focus)
        .on_hover(move |over, window, cx| {
            let over = *over;
            pointer_changed(&hover_state, |s| s.over_trigger = over, window, cx)
        })
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                key_state.update(cx, |s, cx| {
                    s.dismissed = true;
                    s.open = false;
                    cx.notify();
                });
            }
        });
    if tab_stop && focus.is_focused(window) {
        // The trigger is the tab stop: a ring around it, outside it, taking no room.
        wrapper = wrapper.child(focus_ring(
            &c.trigger_focus(),
            f.geometry.radius.default,
            0.0,
        ));
    }
    // As wide as its trigger, so only the trigger itself is hovered.
    wrapper.style().align_self = Some(gpui::AlignSelf::Start);
    wrapper
        .child(trigger)
        .child(bounds_probe(px(0.0), move |bounds, cx| {
            probe_state.update(cx, |s, _| s.trigger = bounds)
        }))
        .children(overlay)
        .into_any_element()
}

impl RenderOnce for HoverCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let content = self.content;
        render_hover(
            Hover {
                id: self.id,
                trigger: self.trigger,
                tab_stop: true,
                placement: self.placement,
                align: self.align,
                transition: Transition::of(MotionDuration::Normal, MotionDuration::Exit),
                panel: Box::new(move |window, cx| {
                    let style = components().popover();
                    let g = style.geometry;
                    let viewport = window.viewport_size().width;
                    let content = content.map(|c| c(window, cx));
                    div()
                        .id(child_id(&id, "card"))
                        .debug_selector(|| part_name(&id, "card"))
                        .px(px(g.padding_x))
                        .py(px(g.padding_y))
                        // Never wider than the window, less the margin it is kept from the edges.
                        .max_w(
                            px(CARD_MAX_WIDTH)
                                .min(inside_window(viewport, px(foundation().spacing.s2))),
                        )
                        .treatment(&style.surface)
                        .border(px(g.border_width))
                        .rounded(px(g.radius))
                        .text_role(g.body)
                        .children(content)
                }),
            },
            window,
            cx,
        )
    }
}
