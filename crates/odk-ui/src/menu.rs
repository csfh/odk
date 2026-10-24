//! `ContextMenu` and `DropdownMenu`, which open the list surface of `menu_list.rs`. A dropdown's
//! link trigger is the link of `link.rs`, which a breadcrumb shares.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Pixels, Point, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, div, point, px,
};
use odk_a11y::{Accessible, Describe, Role};

use crate::focus::{child_id, focus_handle, part_name};
use crate::interaction::{RingTarget, focus_ring};
use crate::layout::bounds_probe;
use crate::link::link;
use crate::menu_list::{Fit, Surface, surface};
pub use crate::menu_list::{Menu, MenuItem};
use crate::{Button, components};

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// Where a menu is open, kept across frames so it stays put while it closes.
struct MenuState {
    open: bool,
    at: Point<Pixels>,
    trigger: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            open: false,
            at: Point::default(),
            trigger: Rc::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

type MenuBuilder = Box<dyn FnOnce(Menu) -> Menu + 'static>;

/// A menu that opens at the pointer when `trigger` is right-clicked, or below the trigger
/// with the Menu key or Shift+F10 while it has focus.
#[derive(IntoElement)]
pub struct ContextMenu {
    id: ElementId,
    name: SharedString,
    trigger: AnyElement,
    menu: Option<MenuBuilder>,
    focusable: bool,
}

impl ContextMenu {
    /// A menu named `name` for assistive technology, such as "Row actions", for `trigger`;
    /// the name is required because the menu has no visible label of its own.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        trigger: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            trigger: trigger.into_any_element(),
            menu: None,
            focusable: true,
        }
    }

    /// Builds the menu; it runs only while the menu is shown.
    pub fn menu(mut self, build: impl FnOnce(Menu) -> Menu + 'static) -> Self {
        self.menu = Some(Box::new(build));
        self
    }

    /// Whether the wrapper around the trigger is a tab stop of its own; true by default.
    /// Turn it off when the trigger is focusable itself, so Tab does not stop twice.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }
}

impl Describe for ContextMenu {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Menu, self.name.to_string())
    }
}

/// True for the keys that open a context menu from the keyboard.
fn opens_context_menu(key: &str, shift: bool) -> bool {
    key == "menu" || (key == "f10" && shift)
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state =
            window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| MenuState::default());
        let (open, at, trigger, scroll) = {
            let s = state.read(cx);
            (s.open, s.at, s.trigger.clone(), s.scroll.clone())
        };
        let focus = focus_handle(child_id(&self.id, "trigger"), self.focusable, window, cx);
        let c = components();
        let ring = (self.focusable && focus.is_focused(window)).then(|| {
            focus_ring(
                &c.trigger_focus(),
                c.foundation().geometry.radius.default,
                0.0,
            )
        });

        let close: Action = {
            let state = state.clone();
            Rc::new(move |_, cx| {
                state.update(cx, |s, cx| {
                    s.open = false;
                    cx.notify();
                })
            })
        };
        let menu = self
            .menu
            .map(|build| build(Menu::new()))
            .unwrap_or_default();
        let how = Surface {
            fit: Fit::Flip,
            ..Surface::menu(at, open, close, scroll)
        };
        let overlay = surface(&self.id, menu, how, window, cx);

        let (mouse_state, key_state) = (state.clone(), state);
        div()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "trigger"))
            .relative()
            .track_focus(&focus)
            .on_mouse_down(MouseButton::Right, move |event, _, cx| {
                mouse_state.update(cx, |s, cx| {
                    s.open = true;
                    s.at = event.position;
                    cx.notify();
                });
            })
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                let ks = &event.keystroke;
                if opens_context_menu(&ks.key, ks.modifiers.shift) {
                    key_state.update(cx, |s, cx| {
                        let b = s.trigger.get();
                        s.open = true;
                        s.at = point(b.origin.x, b.origin.y + b.size.height);
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            })
            .children(ring)
            .child(self.trigger)
            .child(bounds_probe(px(0.0), move |b, _| trigger.set(b)))
            .children(overlay)
    }
}

/// A button that opens a menu below itself. Enter, Space or Down opens it.
#[derive(IntoElement)]
pub struct DropdownMenu {
    id: ElementId,
    label: SharedString,
    menu: Option<MenuBuilder>,
    trigger: Trigger,
}

/// How a dropdown's trigger looks.
enum Trigger {
    /// A `Button` with the label and a `▾`.
    Button,
    /// Text drawn like a link, as for the `…` of a folded breadcrumb, with its own accessible
    /// name because the label alone does not say what it does. Its focus ring is painted by the
    /// path around it, from `ring`.
    Link {
        name: SharedString,
        ring: RingTarget,
    },
}

impl DropdownMenu {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            menu: None,
            trigger: Trigger::Button,
        }
    }

    /// A dropdown whose trigger is drawn like a link, labelled `label` and named `name`, whose
    /// ring the path around it paints from `ring`.
    pub(crate) fn link(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        name: impl Into<SharedString>,
        ring: RingTarget,
    ) -> Self {
        Self {
            trigger: Trigger::Link {
                name: name.into(),
                ring,
            },
            ..Self::new(id, label)
        }
    }

    pub fn menu(mut self, build: impl FnOnce(Menu) -> Menu + 'static) -> Self {
        self.menu = Some(Box::new(build));
        self
    }
}

impl Describe for DropdownMenu {
    fn accessible(&self) -> Accessible {
        let name = match &self.trigger {
            Trigger::Button => &self.label,
            Trigger::Link { name, .. } => name,
        };
        Accessible::new(Role::Button, name.to_string())
    }
}

impl RenderOnce for DropdownMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state =
            window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| MenuState::default());
        let (open, trigger, scroll) = {
            let s = state.read(cx);
            (s.open, s.trigger.clone(), s.scroll.clone())
        };
        let b = trigger.get();
        let at = point(
            b.origin.x,
            b.origin.y + b.size.height + px(components().foundation().spacing.s1),
        );
        let close: Action = {
            let state = state.clone();
            Rc::new(move |_, cx| {
                state.update(cx, |s, cx| {
                    s.open = false;
                    cx.notify();
                })
            })
        };
        let menu = self
            .menu
            .map(|build| build(Menu::new()))
            .unwrap_or_default();
        let how = Surface {
            fit: Fit::Below(b),
            ..Surface::menu(at, open, close, scroll)
        };
        let overlay = surface(&self.id, menu, how, window, cx);

        let (click_state, key_state) = (state.clone(), state);
        div()
            .relative()
            .flex()
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "down" {
                    key_state.update(cx, |s, cx| {
                        s.open = true;
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            })
            .child({
                // The pointer toggles on press, like a native menu button. A press outside the
                // open menu has closed it already, so toggle from how it was drawn, not from its
                // state now. A frame can be drawn between press and release, which is why this
                // is not the click. Enter and Space arrive as keyboard clicks.
                let toggle = Rc::new(move |cx: &mut App| {
                    click_state.update(cx, |s, cx| {
                        s.open = !open;
                        cx.notify();
                    });
                });
                let on_press = toggle.clone();
                let on_click = move |event: &ClickEvent, _: &mut Window, cx: &mut App| {
                    if matches!(event, ClickEvent::Keyboard(_)) {
                        toggle(cx);
                    }
                };
                match self.trigger {
                    Trigger::Button => div()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_press(cx))
                        .child(
                            Button::new(child_id(&self.id, "button"))
                                .label(format!("{} ▾", self.label))
                                .on_click(on_click),
                        )
                        .into_any_element(),
                    Trigger::Link { ring, .. } => link(&self.id, self.label, &ring, window, cx)
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_press(cx))
                        .on_click(on_click)
                        .into_any_element(),
                }
            })
            .child(bounds_probe(px(0.0), move |b, _| trigger.set(b)))
            .children(overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_from_the_menu_key_or_shift_f10() {
        assert!(opens_context_menu("menu", false));
        assert!(opens_context_menu("f10", true));
        assert!(!opens_context_menu("f10", false));
    }
}
