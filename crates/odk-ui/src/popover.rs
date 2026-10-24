//! `Popover`: interactive content anchored to a trigger (`docs/spec/odk-overlays.md`, "Popover"),
//! on the popover's component token. Unlike a `HoverCard` it opens on a press or Enter or Space
//! and takes focus, so the content can hold controls; unlike a `Dialog` it is not modal: the
//! window behind it stays live, Tab leaves it, and focus is not trapped.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Div, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, MouseButton, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::foundation::MotionDuration;

use crate::components;
use crate::focus::{FocusNext, FocusPrevious, child_id, move_focus, part_name};
use crate::layout::{StyledExt, bounds_probe, inside_window, single_line};
use crate::overlay::{Action, Layer, Transition, dismiss_catcher, layer_over, lift};
use crate::placement::{Align, Placement, place};

type Build = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement + 'static>;
type OpenChange = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;

/// The widest a popover grows before its content wraps, in logical pixels. ODK's own (ADR 0035):
/// it has no width unless the content gives it one.
const POPOVER_MAX_WIDTH: f32 = 360.0;

/// Interactive content in a panel beside a trigger.
///
/// Opening it (a press on the trigger, or Enter or Space while the trigger has focus) moves focus
/// into the panel; Tab then walks its controls and may carry on past the last one into the rest
/// of the window, which stays live. Escape, with focus in the panel or on the trigger, and a press
/// outside close it, as far as [`Popover::close_on_escape`] and
/// [`Popover::close_on_outside_click`] allow, and focus goes back to the trigger. A press outside
/// that closes it is the popover's and presses nothing under it; with outside clicks off it
/// stays, and the window behind keeps working.
///
/// Uncontrolled by default, starting from [`Popover::default_open`]; [`Popover::open`] makes it
/// controlled, and the application follows [`Popover::on_open_change`]. It goes to the other
/// side of the trigger when it has no room on [`Popover::placement`]'s, appears over the
/// foundation's `normal` and goes over its `exit`, and keeps its content, built only while it is
/// shown, from the application.
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    name: SharedString,
    trigger: AnyElement,
    title: Option<SharedString>,
    content: Option<Build>,
    placement: Placement,
    align: Align,
    open: Option<bool>,
    default_open: bool,
    on_open_change: Option<OpenChange>,
    close_on_escape: bool,
    close_on_outside_click: bool,
}

impl Popover {
    /// A popover named `name` for assistive technology, such as "Filter", opened from `trigger`;
    /// the name is required because the panel may have no visible title.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        trigger: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            trigger: trigger.into_any_element(),
            title: None,
            content: None,
            placement: Placement::Bottom,
            align: Align::Start,
            open: None,
            default_open: false,
            on_open_change: None,
            close_on_escape: true,
            close_on_outside_click: true,
        }
    }

    /// A heading at the top of the panel.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Builds the panel's content; it runs only while the popover is shown.
    pub fn content<E: IntoElement>(
        mut self,
        build: impl FnOnce(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.content = Some(Box::new(move |w, cx| build(w, cx).into_any_element()));
        self
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

    /// Controls it: shown while `open`. The application follows [`Popover::on_open_change`].
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Whether an uncontrolled popover starts open; closed by default.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Called with the state the user asks for, once per request: on a press on the trigger, on
    /// Enter or Space, on Escape and on a press outside.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Whether Escape closes it; true by default.
    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    /// Whether a press outside closes it; true by default. Off, it stays until the trigger,
    /// Escape or the application closes it, and the window behind stays usable.
    pub fn close_on_outside_click(mut self, close: bool) -> Self {
        self.close_on_outside_click = close;
        self
    }
}

impl Describe for Popover {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.name.to_string())
    }
}

/// What a popover keeps between frames.
struct State {
    /// Whether it is open, when uncontrolled.
    open: bool,
    trigger: Rc<Cell<Bounds<Pixels>>>,
}

/// Tab in the panel: on to the next stop, and when that is outside the panel, to the one after
/// the trigger.
fn leave_forward(
    content: &FocusHandle,
    opener: Option<&FocusHandle>,
    window: &mut Window,
    cx: &mut App,
) {
    window.focus_next();
    if !content.contains_focused(window, cx)
        && let Some(opener) = opener
    {
        move_focus(opener, window);
        window.focus_next();
    }
}

/// Shift+Tab in the panel: back to the previous stop, and when that is outside the panel, to the
/// trigger.
fn leave_back(
    content: &FocusHandle,
    opener: Option<&FocusHandle>,
    window: &mut Window,
    cx: &mut App,
) {
    window.focus_prev();
    if !content.contains_focused(window, cx)
        && let Some(opener) = opener
    {
        move_focus(opener, window);
    }
}

/// Whether a key that came up opens the popover from its trigger. On the release, not the press,
/// since a button in the panel takes an Enter that comes up after focus moved there.
fn toggles(key: &str) -> bool {
    key == "enter" || key == "space"
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let f = c.foundation().clone();
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| State {
            open: self.default_open,
            trigger: Rc::default(),
        });
        let (kept_open, trigger) = {
            let s = state.read(cx);
            (s.open, s.trigger.clone())
        };
        let controlled = self.open.is_some();
        let open = self.open.unwrap_or(kept_open);

        // Every request goes through here: an uncontrolled popover follows it, and the
        // application hears it either way.
        let request: OpenChange = {
            let (state, on_change) = (state.clone(), self.on_open_change.clone());
            Rc::new(move |want, window, cx| {
                if !controlled {
                    state.update(cx, |s, cx| {
                        s.open = want;
                        cx.notify();
                    });
                }
                if let Some(f) = &on_change {
                    f(want, window, cx);
                }
            })
        };
        let close: Action = {
            let request = request.clone();
            Rc::new(move |window, cx| request(false, window, cx))
        };

        let frame = layer_over(
            child_id(&self.id, "layer"),
            open,
            true,
            Transition::of(MotionDuration::Normal, MotionDuration::Exit),
            window,
            cx,
        );
        let overlay = frame.amount.map(|amount| {
            let style = c.popover();
            let g = style.geometry;
            let viewport = window.viewport_size();
            let key_close = close.clone();
            let escape = self.close_on_escape;
            let (content_next, content_prev) = (frame.content.clone(), frame.content.clone());
            let (opener_next, opener_prev) = (
                crate::overlay::opener(&child_id(&self.id, "layer"), window, cx),
                crate::overlay::opener(&child_id(&self.id, "layer"), window, cx),
            );
            let mut panel: Div = div()
                .debug_selector(|| part_name(&self.id, "popover"))
                .flex()
                .flex_col()
                .gap(px(g.gap))
                .px(px(g.padding_x))
                .py(px(g.padding_y))
                // Content-driven, never wider than a named length or than the window.
                .max_w(px(POPOVER_MAX_WIDTH).min(inside_window(viewport.width, px(f.spacing.s2))))
                .max_h(inside_window(viewport.height, px(f.spacing.s2)))
                .treatment(&style.surface)
                .border(px(g.border_width))
                .rounded(px(g.radius))
                .text_role(g.body)
                .opacity(f.motion.easing.apply(amount));
            if let Some(title) = self.title.clone() {
                panel = panel.child(single_line(
                    div()
                        .debug_selector(|| part_name(&self.id, "title"))
                        .text_role(g.title)
                        .child(title),
                ));
            }
            if let Some(build) = self.content {
                panel = panel.child(build(window, cx));
            }
            let panel = panel
                .id(child_id(&self.id, "panel"))
                .overflow_y_scroll()
                .track_focus(&frame.content)
                .occlude()
                // Not modal: Tab leaves the panel, to the control after the trigger and Shift+Tab
                // to the trigger itself, instead of wrapping round the window.
                .on_action(move |_: &FocusNext, window, cx| {
                    leave_forward(&content_next, opener_next.as_ref(), window, cx)
                })
                .on_action(move |_: &FocusPrevious, window, cx| {
                    leave_back(&content_prev, opener_prev.as_ref(), window, cx)
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        if escape {
                            key_close(window, cx);
                        }
                        // The innermost overlay has heard it: an outer one does not close too.
                        cx.stop_propagation();
                    }
                });
            let catcher = self
                .close_on_outside_click
                .then(|| dismiss_catcher(&self.id, close.clone(), window, cx));
            let placed =
                place(self.placement, self.align, trigger.get()).wrap(px(f.spacing.s1), panel);
            let popover = lift(&self.id, Layer::Popover, placed, window, cx);
            // Out of flow, so a flex parent leaves no gap for them.
            div().absolute().children(catcher).child(popover)
        });

        // The panel is a child of the trigger's wrapper in the tree, so its keys reach the
        // wrapper's handlers: they leave what happens inside the panel to the panel.
        let (inside_up, inside_down) = (frame.content.clone(), frame.content.clone());
        let (press, release, escape_key) = (request.clone(), request.clone(), close);
        let toggle_from = open;
        let escape_on = self.close_on_escape;
        let trigger_probe = trigger;
        div()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "trigger"))
            .relative()
            .flex()
            // The pointer toggles on the press, like a native menu button; a press while it is
            // open and outside-click dismissal is on reaches the dismissal first and closes it.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                press(!toggle_from, window, cx)
            })
            .on_key_up(move |event: &KeyUpEvent, window, cx| {
                if toggles(&event.keystroke.key) && !inside_up.contains_focused(window, cx) {
                    release(!toggle_from, window, cx);
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if toggle_from
                    && escape_on
                    && event.keystroke.key == "escape"
                    && !inside_down.contains_focused(window, cx)
                {
                    escape_key(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(self.trigger)
            .child(bounds_probe(px(0.0), move |b, _| trigger_probe.set(b)))
            .children(overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_space_open_it() {
        assert!(toggles("enter") && toggles("space"));
        assert!(!toggles("down") && !toggles("escape"));
    }
}
