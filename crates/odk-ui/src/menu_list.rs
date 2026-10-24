//! The list surface that `ContextMenu`, `DropdownMenu` and a `Select`'s list share, on component
//! tokens (`docs/spec/odk-component-tokens.md`, "Menus"): items, separators, checkable,
//! disabled and danger items, and key hints. Up and Down move between enabled items, Home and
//! End jump, Enter or Space chooses, and Escape, Tab or a click outside closes.
//!
//! An item is hovered (the pointer is over it), highlighted (the keyboard is on it: it has
//! focus) or selected (it is checked, the chosen option of a select), and each is drawn apart:
//! a hover is a light tint, the highlight a stronger one, and the selected item has a check
//! mark, whichever of the others it is too.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Bounds, Div, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Point, ScrollHandle, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, anchored, div, px,
};
use odk_a11y::{Accessible, Role};
use odk_theme::component::{MenuItemStyle, States};
use odk_theme::foundation::{Density, MotionDuration, TypeRole};

use crate::components;
use crate::focus::{
    Axis, FocusNext, FocusPrevious, child_id, focus_handle, move_focus, part_name, roving_target,
};
use crate::input::field_glyph;
use crate::interaction::{Pointer, pointer_states};
use crate::layout::{StyledExt, h_flex, inside_window, single_line, v_flex};
use crate::overlay::{Layer, Transition, dismiss_catcher, layer_over, lift};
use crate::placement::{Align, Placement, place};
use crate::select::{Typed, find, typed_char};
use crate::theme::{colour, mono};

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// The narrowest a menu is, so short menus still read as menus, in logical pixels. ODK's own
/// (ADR 0035).
const MENU_MIN_WIDTH: f32 = 192.0;

/// The mark of the selected item, and of the chosen one of a radio set.
const CHECK: &str = "✓";
const CHOSEN: &str = "●";

/// A shortcut at the end of a menu item, as text in the item's hint colour (muted, and the
/// item's own text colour on a highlight) and not as keycaps: a menu is a list of words. It
/// follows the pointer through the row's `group`. It only labels the item; the application binds
/// the key.
pub(crate) fn hint_text(
    id: ElementId,
    chord: SharedString,
    s: &MenuItemStyle,
    group: &SharedString,
) -> Stateful<Div> {
    let (hover, pressed) = (colour(s.hint.hover), colour(s.hint.pressed));
    let mut hint = div()
        .id(id)
        .flex_shrink_0()
        .text_role(TypeRole::Mono)
        .text_color(colour(s.hint.default))
        .child(chord);
    if s.responds {
        hint = hint
            .group_hover(group.clone(), move |s| s.text_color(hover))
            .group_active(group.clone(), move |s| s.text_color(pressed));
    }
    hint
}

/// One entry of a [`Menu`].
pub struct MenuItem {
    label: SharedString,
    hint: Option<SharedString>,
    icon: Option<SharedString>,
    checked: Option<bool>,
    /// A checked item is one of a set of which one is chosen, so it shows a dot, not a check.
    radio: bool,
    disabled: bool,
    danger: bool,
    on_select: Option<Action>,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            icon: None,
            checked: None,
            radio: false,
            disabled: false,
            danger: false,
            on_select: None,
        }
    }

    /// A key chord, such as `Ctrl+C`, shown at the end of the item as muted text. It only labels the
    /// item; the application binds the key.
    pub fn hint(mut self, chord: impl Into<SharedString>) -> Self {
        self.hint = Some(chord.into());
        self
    }

    /// Makes the item checkable, showing a check mark when `checked`.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Makes the item one of a set of which one is chosen, drawn with a dot when `chosen`. Give
    /// every item of the set a `radio`, and have each `on_select` choose its own.
    pub fn radio(mut self, chosen: bool) -> Self {
        self.checked = Some(chosen);
        self.radio = true;
        self
    }

    /// A glyph before the label, in the item's own colour. ODK has no icon set yet, so it is
    /// text. Items without one keep the column, so labels line up.
    pub fn icon(mut self, glyph: impl Into<SharedString>) -> Self {
        self.icon = Some(glyph.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Draws the item in the danger role, for destructive actions.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn accessible(&self) -> Accessible {
        let role = match (self.checked, self.radio) {
            (Some(_), true) => Role::MenuItemRadio,
            (Some(_), false) => Role::MenuItemCheckbox,
            (None, _) => Role::MenuItem,
        };
        let mut a = Accessible::new(role, self.label.to_string()).disabled(self.disabled);
        a.selected = self.checked;
        a
    }
}

enum Entry {
    Item(MenuItem),
    Separator,
    /// A label over the items after it, up to the next separator.
    Heading(SharedString),
}

/// The items of a menu, in order.
#[derive(Default)]
pub struct Menu {
    entries: Vec<Entry>,
}

impl Menu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// A quiet label over a group of items. It is not an item: the keyboard skips it.
    pub fn heading(mut self, label: impl Into<SharedString>) -> Self {
        self.entries.push(Entry::Heading(label.into()));
        self
    }

    /// What the menu, named `name` by whoever opens it, and each item report. Separators
    /// have no name.
    pub fn accessibles(&self, name: &str) -> Vec<Accessible> {
        let mut all = vec![Accessible::new(Role::Menu, name)];
        all.extend(self.entries.iter().map(|e| match e {
            Entry::Item(i) => i.accessible(),
            Entry::Separator => Accessible::new(Role::Separator, ""),
            Entry::Heading(label) => Accessible::new(Role::Group, label.to_string()),
        }));
        all
    }
}

/// Picks the entry to focus when a key is typed: given the key, the entry that has focus and
/// the app, the entry to focus next, if the key selects one. A `Select` matches what is typed
/// against its labels with it.
pub(crate) type Typeahead = Rc<dyn Fn(&KeyDownEvent, Option<usize>, &mut App) -> Option<usize>>;

/// How a [`surface`] keeps inside the window.
#[derive(Clone, Copy)]
pub(crate) enum Fit {
    /// Shifted back inside, a margin from the edges: a select's list, which stays on its field.
    Snap,
    /// Its corner goes to the other side of the point when there is no room: a context menu.
    Flip,
    /// Below `Bounds` (a dropdown's trigger), or above when there is no room below.
    Below(Bounds<Pixels>),
}

/// How a [`surface`] is drawn and what it does besides showing its entries.
pub(crate) struct Surface {
    /// Where its top left corner goes; for [`Fit::Below`], not used.
    pub at: Point<Pixels>,
    pub fit: Fit,
    pub open: bool,
    /// Called when the user picks an item or dismisses the surface.
    pub close: Action,
    /// The narrowest it is.
    pub min_width: Pixels,
    /// The entry that has focus when it opens; the first enabled one when `None`.
    pub focus: Option<usize>,
    /// Scrolls the entries when there are too many to fit the window. Entries are the scroll
    /// container's children, in order, so an entry's index is its child index.
    pub scroll: ScrollHandle,
    pub typeahead: Option<Typeahead>,
    /// The density of its items: a select's own, else the application's.
    pub density: Option<Density>,
}

impl Surface {
    /// A menu surface: [`MENU_MIN_WIDTH`] wide at least, focus on the first item, no typeahead,
    /// at the application's density.
    pub(crate) fn menu(at: Point<Pixels>, open: bool, close: Action, scroll: ScrollHandle) -> Self {
        Self {
            at,
            fit: Fit::Snap,
            open,
            close,
            min_width: px(MENU_MIN_WIDTH),
            focus: None,
            scroll,
            typeahead: None,
            density: None,
        }
    }
}

/// Draws an open (or closing) menu, or the list of a select. It fades in over the foundation's
/// `normal` and out over its `exit`, on its curve, and at once with reduced motion.
pub(crate) fn surface(
    id: &ElementId,
    menu: Menu,
    how: Surface,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let Surface {
        at,
        fit,
        open,
        close,
        min_width,
        focus,
        scroll,
        typeahead,
        density,
    } = how;
    let c = components();
    let f = c.foundation().clone();
    let transition = Transition::of(MotionDuration::Normal, MotionDuration::Exit);
    let frame = layer_over(child_id(id, "layer"), open, true, transition, window, cx);
    let amount = f.motion.easing.apply(frame.amount?);
    let density = c.density(density);
    let m = c.menu();
    let g = m.geometry;
    // Kept clear of the window's edges by the menu's own padding and a step more.
    let margin = px(f.spacing.s2);

    // A menu with no search of its own finds an item by the first letters of its label, as a
    // select's list does; a pause starts a new search.
    let typeahead = typeahead.or_else(|| {
        let labels: Vec<SharedString> = menu
            .entries
            .iter()
            .map(|e| match e {
                Entry::Item(item) => item.label.clone(),
                _ => SharedString::default(),
            })
            .collect();
        let enabled: Vec<usize> = menu
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| matches!(e, Entry::Item(item) if !item.disabled).then_some(i))
            .collect();
        let typed = window.use_keyed_state(child_id(id, "typed"), cx, |_, _| Typed::default());
        let find: Typeahead = Rc::new(move |event, from, cx| {
            let ch = typed_char(event)?;
            let now = cx.background_executor().now();
            let query = typed.update(cx, |t, _| t.push(&ch, now).to_string());
            find(&labels, &enabled, from, &query)
        });
        Some(find)
    });
    let enabled: Vec<usize> = menu
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| matches!(e, Entry::Item(item) if !item.disabled).then_some(i))
        .collect();
    let handles: Rc<Vec<FocusHandle>> = Rc::new(
        enabled
            .iter()
            .map(|i| focus_handle(child_id(id, format!("item-{i}")), false, window, cx))
            .collect(),
    );
    if frame.opened {
        let first = focus
            .and_then(|entry| enabled.iter().position(|e| *e == entry))
            .or((!handles.is_empty()).then_some(0));
        if let Some(n) = first {
            let (handle, entry, scroll) = (handles[n].clone(), enabled[n], scroll.clone());
            // After this frame, which is the first to give the scrolling list its size: a scroll
            // asked for sooner is measured against no size at all and lost.
            window.defer(cx, move |window, _| {
                scroll.scroll_to_item(entry);
                move_focus(&handle, window);
            });
        }
    }

    let (keys, key_close, key_enabled, key_scroll) = (
        handles.clone(),
        close.clone(),
        enabled.clone(),
        scroll.clone(),
    );
    let (out_close, next_close, prev_close) = (close.clone(), close.clone(), close.clone());
    let viewport = window.viewport_size();
    // Never taller than the room it has: the window, or for a menu under its trigger the larger
    // of the room under it and the room over it, so it fits on one side or the other.
    let max_h = match fit {
        Fit::Below(trigger) => {
            let room = (trigger.top() - margin).max(viewport.height - trigger.bottom() - margin);
            (room - px(f.spacing.s2)).max(px(0.0))
        }
        _ => inside_window(viewport.height, margin),
    };
    let mut list = v_flex()
        .debug_selector(|| part_name(id, "menu"))
        .id(child_id(id, "menu"))
        .track_focus(&frame.content)
        .min_w(min_width)
        // Never larger than the window; long labels end in `…` instead, and too many entries
        // scroll.
        .max_w(inside_window(viewport.width, margin))
        .max_h(max_h)
        .overflow_y_scroll()
        .track_scroll(&scroll)
        .px(px(g.padding_x))
        .py(px(g.padding_y))
        .bg(colour(m.surface.background))
        .text_color(colour(m.surface.foreground))
        .border(px(g.border_width))
        .border_color(colour(m.surface.border))
        .rounded(px(g.radius))
        .text_role(g.body)
        .opacity(amount)
        .occlude()
        .on_mouse_down_out(move |_, window, cx| out_close(window, cx))
        .on_action(move |_: &FocusNext, window, cx| next_close(window, cx))
        .on_action(move |_: &FocusPrevious, window, cx| prev_close(window, cx))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            if key == "escape" {
                key_close(window, cx);
                cx.stop_propagation();
                return;
            }
            let current = keys.iter().position(|h| h.is_focused(window)).unwrap_or(0);
            let next = roving_target(key, current, keys.len(), Axis::Vertical).or_else(|| {
                let find = typeahead.as_ref()?;
                let entry = find(event, key_enabled.get(current).copied(), cx)?;
                key_enabled.iter().position(|e| *e == entry)
            });
            if let Some(next) = next {
                key_scroll.scroll_to_item(key_enabled[next]);
                move_focus(&keys[next], window);
                cx.stop_propagation();
            }
        });

    for (i, entry) in menu.entries.into_iter().enumerate() {
        let item = match entry {
            Entry::Separator => {
                list = list.child(
                    div()
                        .flex_shrink_0()
                        .my(px(m.separator_margin))
                        .h(px(g.border_width))
                        .bg(colour(m.separator)),
                );
                continue;
            }
            Entry::Heading(label) => {
                list = list.child(
                    div()
                        .debug_selector(|| part_name(id, &format!("heading-{i}")))
                        .flex_shrink_0()
                        .px(px(c
                            .menu_item(density, States::NONE, false)
                            .geometry
                            .padding_x))
                        .pt(px(m.separator_margin))
                        .pb(px(f.spacing.s1))
                        .text_role(TypeRole::Small)
                        .text_color(colour(m.heading))
                        .child(single_line(div().child(label))),
                );
                continue;
            }
            Entry::Item(item) => item,
        };
        let at = enabled.iter().position(|e| *e == i);
        // The keyboard is on the item that has focus.
        let highlighted = at.is_some_and(|n| handles[n].is_focused(window));
        let states = States::NONE
            .disabled(item.disabled)
            .highlighted(highlighted)
            .selected(item.checked == Some(true));
        let s = c.menu_item(density, states, item.danger);
        let ig = s.geometry;
        #[cfg(test)]
        drawn::record(&part_name(id, &format!("row-{i}")), s);
        let mut row = h_flex()
            .group(format!("row-{i}"))
            .id(child_id(id, format!("row-{i}")))
            .debug_selector(|| part_name(id, &format!("row-{i}")))
            .flex_shrink_0()
            .h(px(ig.height))
            .px(px(ig.padding_x))
            .gap(px(ig.gap))
            .rounded(px(ig.radius))
            .text_role(ig.type_role)
            .child(
                // The check column is always there, so checking an item moves nothing.
                div()
                    .flex_shrink_0()
                    .size(px(ig.icon_size))
                    .when_some(s.indicator, |d, ink| {
                        let mark = if item.radio { CHOSEN } else { CHECK };
                        d.child(field_glyph(mark.into(), ig.icon_size, colour(ink)))
                    }),
            )
            .children(item.icon.map(|glyph| {
                // In the row's text colour, so it follows the hover and the highlight.
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(ig.icon_size))
                    .font_family(mono())
                    .child(glyph)
            }))
            .child(single_line(
                div()
                    .flex_1()
                    .min_w_0()
                    .debug_selector(|| part_name(id, &format!("label-{i}")))
                    .child(item.label),
            ))
            .children(item.hint.map(|h| {
                hint_text(
                    child_id(id, format!("hint-{i}")),
                    h,
                    &s,
                    &SharedString::from(format!("row-{i}")),
                )
            }));
        row = pointer_states(row, &s.colours, s.responds, &Pointer::Own);
        if let Some(n) = at {
            let (on_select, close) = (item.on_select.clone(), close.clone());
            row = row
                .track_focus(&handles[n])
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    close(window, cx);
                    if let Some(f) = &on_select {
                        f(window, cx);
                    }
                });
        }
        list = list.child(row);
    }

    // The catcher first, so the list is over it: a press anywhere else closes the menu and
    // presses nothing.
    let catcher = dismiss_catcher(id, close.clone(), window, cx);
    let menu = match fit {
        Fit::Snap => lift(
            id,
            Layer::Popover,
            anchored()
                .position(at)
                .snap_to_window_with_margin(margin)
                .child(list),
            window,
            cx,
        ),
        Fit::Flip => lift(
            id,
            Layer::Popover,
            anchored().position(at).child(list),
            window,
            cx,
        ),
        Fit::Below(trigger) => lift(
            id,
            Layer::Popover,
            place(Placement::Bottom, Align::Start, trigger).wrap(px(f.spacing.s1), list),
            window,
            cx,
        ),
    };
    // Out of flow, so a flex parent leaves no gap for the two of them.
    Some(
        div()
            .absolute()
            .child(catcher)
            .child(menu)
            .into_any_element(),
    )
}

/// What each list item drew on its last frame, for tests: its style, by the row's part name.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use odk_theme::component::MenuItemStyle;

    thread_local! {
        static DRAWN: RefCell<HashMap<String, MenuItemStyle>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(part: &str, style: MenuItemStyle) {
        DRAWN.with(|d| d.borrow_mut().insert(part.to_owned(), style));
    }

    /// What the row with part name `part` drew on its last frame.
    pub(crate) fn last(part: &str) -> MenuItemStyle {
        DRAWN.with(|d| {
            *d.borrow()
                .get(part)
                .unwrap_or_else(|| panic!("no row {part:?} has been drawn"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_kind_of_each_item() {
        let menu = Menu::new()
            .item(MenuItem::new("Copy").hint("Ctrl+C"))
            .separator()
            .item(MenuItem::new("Wrap").checked(true))
            .item(MenuItem::new("Delete").danger().disabled(true));
        let a = menu.accessibles("File");
        assert_eq!(a[0].role, Role::Menu);
        assert_eq!(a[1].role, Role::MenuItem);
        assert_eq!(a[2].role, Role::Separator);
        assert_eq!(
            (a[3].role, a[3].selected),
            (Role::MenuItemCheckbox, Some(true))
        );
        assert!(a[4].disabled);
    }
}
