//! A sidebar's navigation, on component tokens: items in the tab's mapping with a fill, one of
//! them the current page (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::{Density, TypeRole};

use crate::components;
use crate::focus::{Axis, child_id, focus_handle, move_focus, part_name, roving_among};
use crate::interaction::{Pointer, RingTarget, pointer_states};
use crate::layout::{StyledExt, single_line, v_flex};
use crate::theme::{colour, mono};

type ChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;

struct Item {
    value: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    counter: Option<SharedString>,
    disabled: bool,
}

enum Entry {
    /// The heading of the items after it, such as "Workspace".
    Group(SharedString),
    Item(Item),
}

/// The navigation of a sidebar: rows that each open a page, optionally under group headings,
/// one of them the current page. A row is a control's height at the density, with an optional
/// icon before its label and a counter after it; the current page has a quiet fill and an
/// accent indicator along its leading edge, which stay under the pointer and in focus.
///
/// The list is one tab stop, as every composite control is: Tab reaches the current page's row,
/// or the first enabled one. Up and Down move focus between enabled rows, wrapping, and Home and
/// End jump to the ends; Enter or Space opens the focused row, and so does a click. Moving focus
/// never opens a page. Disabled rows and headings take no focus.
///
/// Uncontrolled by default, starting on `default_value`; pass `value` to control it and follow
/// `on_change`, which hears of each new page once.
#[derive(IntoElement)]
pub struct NavList {
    id: ElementId,
    name: SharedString,
    entries: Vec<Entry>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    on_change: Option<ChangeHandler>,
    disabled: bool,
    density: Option<Density>,
}

impl NavList {
    /// A navigation named `name` for assistive technology, such as "Settings".
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            entries: Vec::new(),
            value: None,
            default_value: None,
            on_change: None,
            disabled: false,
            density: None,
        }
    }

    /// Starts a group: a heading over the items added after it.
    pub fn group(mut self, heading: impl Into<SharedString>) -> Self {
        self.entries.push(Entry::Group(heading.into()));
        self
    }

    /// Adds a row that opens the page `value`, labelled `label`.
    pub fn item(mut self, value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.entries.push(Entry::Item(Item {
            value: value.into(),
            label: label.into(),
            icon: None,
            counter: None,
            disabled: false,
        }));
        self
    }

    /// Puts a glyph before the label of the row with this value, at the density's icon size.
    pub fn icon(mut self, value: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Self {
        let (value, glyph) = (value.into(), glyph.into());
        for item in self.items_mut(&value) {
            item.icon = Some(glyph.clone());
        }
        self
    }

    /// Puts a counter, such as `"12"`, at the end of the row with this value.
    pub fn counter(
        mut self,
        value: impl Into<SharedString>,
        counter: impl Into<SharedString>,
    ) -> Self {
        let (value, counter) = (value.into(), counter.into());
        for item in self.items_mut(&value) {
            item.counter = Some(counter.clone());
        }
        self
    }

    /// Disables the row with this value: it is shown but cannot be opened or focused.
    pub fn disable(mut self, value: impl Into<SharedString>) -> Self {
        let value = value.into();
        for item in self.items_mut(&value) {
            item.disabled = true;
        }
        self
    }

    /// Disables every row.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The list's own density, whatever the application's: rows 28, 34 or 40 px high.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The current page, making the list controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn items_mut<'a>(&'a mut self, value: &'a SharedString) -> impl Iterator<Item = &'a mut Item> {
        self.entries.iter_mut().filter_map(move |e| match e {
            Entry::Item(item) if item.value == *value => Some(item),
            _ => None,
        })
    }

    /// What each row reports, in order, given the current page: a link, selected when it is the
    /// current page.
    pub fn item_accessibles(&self, current: Option<&str>) -> Vec<Accessible> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item(i) => Some(i),
                Entry::Group(_) => None,
            })
            .map(|i| {
                let name = match &i.counter {
                    Some(counter) => format!("{} ({counter})", i.label),
                    None => i.label.to_string(),
                };
                Accessible::new(Role::Link, name)
                    .selected(current == Some(i.value.as_ref()))
                    .disabled(self.disabled || i.disabled)
            })
            .collect()
    }
}

impl Describe for NavList {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Navigation, self.name.to_string()).disabled(self.disabled)
    }
}

impl RenderOnce for NavList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let density = c.density(self.density);
        let list = c.nav_list(density);
        let default = self.default_value.clone();
        let state = window.use_keyed_state(child_id(&self.id, "value"), cx, |_, _| default);
        let controlled = self.value.is_some();
        let current = self.value.clone().or_else(|| state.read(cx).clone());

        // Opening a page: the uncontrolled state follows, then the application hears of it, once.
        let open: ChangeHandler = {
            let (state, on_change, current) =
                (state.clone(), self.on_change.clone(), current.clone());
            Rc::new(move |value, window, cx| {
                if current.as_ref() == Some(value) {
                    return;
                }
                if !controlled {
                    state.update(cx, |v, cx| {
                        *v = Some(value.clone());
                        cx.notify();
                    });
                }
                if let Some(f) = &on_change {
                    f(value, window, cx);
                }
            })
        };

        let rows: Vec<&Item> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item(item) => Some(item),
                Entry::Group(_) => None,
            })
            .collect();
        let enabled: Rc<Vec<usize>> = Rc::new(
            (0..rows.len())
                .filter(|i| !self.disabled && !rows[*i].disabled)
                .collect(),
        );
        // The one row in the Tab order: the current page's if it is enabled, else the first
        // enabled one, else none.
        let stop = enabled
            .iter()
            .copied()
            .find(|i| current.as_ref() == Some(&rows[*i].value))
            .or_else(|| enabled.first().copied());
        let handles: Rc<Vec<FocusHandle>> = Rc::new(
            (0..rows.len())
                .map(|i| {
                    focus_handle(
                        child_id(&self.id, format!("focus-{i}")),
                        Some(i) == stop,
                        window,
                        cx,
                    )
                })
                .collect(),
        );

        // The focused row's ring is painted over the rows after it, whose fills would cover a
        // ring of its own.
        let ring = RingTarget::default();
        let mut focus = None;
        let (keys, key_enabled) = (handles.clone(), enabled.clone());
        let mut column = v_flex()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "list"))
            .relative()
            .min_w_0()
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let Some(current) = keys.iter().position(|h| h.is_focused(window)) else {
                    return;
                };
                let key = event.keystroke.key.as_str();
                if let Some(next) = roving_among(key, current, &key_enabled, Axis::Vertical) {
                    move_focus(&keys[next], window);
                    cx.stop_propagation();
                }
            });
        let (mut row_n, mut group_n) = (0, 0);
        for entry in self.entries {
            let item = match entry {
                Entry::Group(heading) => {
                    let mut header = single_line(
                        div()
                            .debug_selector(|| part_name(&self.id, &format!("group-{group_n}")))
                            .px(px(list.header_padding_x))
                            .mb(px(list.header_gap))
                            .text_role(list.header_role)
                            .text_color(colour(list.header)),
                    )
                    .child(heading);
                    if row_n > 0 || group_n > 0 {
                        header = header.mt(px(list.group_gap));
                    }
                    column = column.child(header);
                    group_n += 1;
                    continue;
                }
                Entry::Item(item) => item,
            };
            let i = row_n;
            row_n += 1;
            let usable = enabled.contains(&i);
            let on = current.as_ref() == Some(&item.value);
            // A disabled row tracks no focus handle, so it is neither tabbed to nor clicked
            // into focus.
            let handle = usable.then(|| &handles[i]);
            let focused = handle.is_some_and(|h| h.is_focused(window));
            let states = States::NONE.selected(on).focused(focused).disabled(!usable);
            let s = c.nav_item(density, states);
            let g = s.geometry;
            #[cfg(test)]
            crate::drawn::record(&part_name(&self.id, &format!("item-{i}")), s);

            let mut row = div()
                .id(child_id(&self.id, format!("item-{i}")))
                .debug_selector(|| part_name(&self.id, &format!("item-{i}")))
                .relative()
                .flex()
                .items_center()
                .flex_shrink_0()
                .control_geometry(&g);
            if let Some(glyph) = item.icon {
                let size = px(g.icon_size);
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .size(size)
                        .font_family(mono())
                        .text_size(size)
                        .line_height(size)
                        .child(glyph),
                );
            }
            row = row.child(single_line(
                div()
                    .debug_selector(|| part_name(&self.id, &format!("label-{i}")))
                    .flex_1()
                    .min_w_0()
                    .child(item.label),
            ));
            if let Some(counter) = item.counter {
                row = row.child(
                    div()
                        .debug_selector(|| part_name(&self.id, &format!("counter-{i}")))
                        .flex_shrink_0()
                        .text_role(TypeRole::Small)
                        .child(counter),
                );
            }
            if let Some(indicator) = s.indicator {
                // Along the leading edge, over the fill: it takes no space.
                row = row.child(
                    div()
                        .debug_selector(|| part_name(&self.id, "indicator"))
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(s.indicator_width))
                        // The row's own leading corners, so it never pokes past them.
                        .rounded_tl(px(g.radius))
                        .rounded_bl(px(g.radius))
                        .bg(colour(indicator)),
                );
            }
            // Last, so hover and press keep the type role.
            row = pointer_states(row, &s.colours, s.responds, &Pointer::Own);
            if let Some(handle) = handle {
                let (value, open) = (item.value.clone(), open.clone());
                row = row
                    .track_focus(handle)
                    .cursor_pointer()
                    .on_click(move |_, window, cx| open(&value, window, cx));
            }
            if s.focus_visible {
                row = row.child(ring.probe());
                focus = Some((s.focus, g.radius));
            }
            column = column.child(row);
        }
        if let Some((focus, radius)) = focus {
            column = column.child(ring.ring(focus, radius, &self.id));
        }
        column
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_each_row_and_the_current_page() {
        let nav = NavList::new("n", "Settings")
            .group("Workspace")
            .item("general", "General")
            .item("members", "Members")
            .counter("members", "4")
            .disable("members");
        assert_eq!(nav.accessible().role, Role::Navigation);
        let items = nav.item_accessibles(Some("general"));
        assert_eq!(items.len(), 2, "a heading is not a row");
        assert_eq!((items[0].role, items[0].selected), (Role::Link, Some(true)));
        assert_eq!(items[1].name, "Members (4)");
        assert!(items[1].disabled && items[1].selected == Some(false));
    }
}
