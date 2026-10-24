//! A segmented control, on component tokens: a frame of segments, one or more of them on
//! (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::Density;

use crate::components;
use crate::focus::{Axis, child_id, focus_handle, move_focus, part_name, roving_among};
use crate::interaction::{Pointer, RingTarget, pointer_states};
use crate::layout::{StyledExt, h_flex, single_line, text_width};
use crate::theme::{colour, mono};

type ChangeHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;

struct Item {
    value: SharedString,
    label: SharedString,
    /// Shown alone when the item has no label of its own (`icon_item`), else before the label.
    icon: Option<SharedString>,
    name: Option<SharedString>,
    disabled: bool,
}

/// A segmented control: a row of options in one frame, such as List and Grid, or Daily, Weekly
/// and Monthly. In single mode exactly one is on once chosen, like a radio group; in multiple
/// mode each toggles on its own. Options are text, an icon alone (`icon_item`) or an icon before
/// text (`icon`). The one that is on has the neutral muted fill; hover and press are lighter
/// tints, so they never pass for it.
///
/// Tab reaches the group, the arrow keys move between options without choosing, Home and End
/// jump to the ends, and Space or Enter toggles the focused one. The arrow keys skip a disabled
/// option, and a disabled group is not a tab stop.
///
/// Uncontrolled by default, keeping its own selection from `default_value`; pass `value` to
/// control it and follow `on_change`.
#[derive(IntoElement)]
pub struct ToggleGroup {
    id: ElementId,
    items: Vec<Item>,
    multiple: bool,
    value: Option<Vec<SharedString>>,
    default_value: Vec<SharedString>,
    on_change: Option<ChangeHandler>,
    name: SharedString,
    disabled: bool,
    density: Option<Density>,
}

impl ToggleGroup {
    /// A group named `name` for assistive technology, such as "Alignment"; it has no visible
    /// label of its own, so the name is required.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            multiple: false,
            value: None,
            default_value: Vec::new(),
            on_change: None,
            name: name.into(),
            disabled: false,
            density: None,
        }
    }

    /// Adds an option. `label` is text or a one-glyph icon.
    pub fn item(mut self, value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.items.push(Item {
            value: value.into(),
            label: label.into(),
            icon: None,
            name: None,
            disabled: false,
        });
        self
    }

    /// Adds an option shown as a glyph alone, a square segment, with an accessible name that says
    /// what it is.
    pub fn icon_item(
        mut self,
        value: impl Into<SharedString>,
        glyph: impl Into<SharedString>,
        name: impl Into<SharedString>,
    ) -> Self {
        self.items.push(Item {
            value: value.into(),
            label: SharedString::default(),
            icon: Some(glyph.into()),
            name: Some(name.into()),
            disabled: false,
        });
        self
    }

    /// Puts a glyph before the label of the option with this value, at the density's icon size.
    pub fn icon(mut self, value: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Self {
        let (value, glyph) = (value.into(), glyph.into());
        for item in self.items.iter_mut().filter(|i| i.value == value) {
            item.icon = Some(glyph.clone());
        }
        self
    }

    /// Disables the option with this value: it shows its state but cannot be changed or
    /// focused.
    pub fn disable(mut self, value: impl Into<SharedString>) -> Self {
        let value = value.into();
        for item in self.items.iter_mut().filter(|i| i.value == value) {
            item.disabled = true;
        }
        self
    }

    /// Disables every option.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// The group's own density, whatever the application's: a frame 28, 34 or 40 px high.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The selection, making the group controlled.
    pub fn value<S: Into<SharedString>>(mut self, value: impl IntoIterator<Item = S>) -> Self {
        self.value = Some(value.into_iter().map(Into::into).collect());
        self
    }

    /// The first selection of an uncontrolled group.
    pub fn default_value<S: Into<SharedString>>(
        mut self,
        value: impl IntoIterator<Item = S>,
    ) -> Self {
        self.default_value = value.into_iter().map(Into::into).collect();
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

/// The selection after `value` is pressed.
fn toggle(selected: &[SharedString], value: &SharedString, multiple: bool) -> Vec<SharedString> {
    if !multiple {
        return vec![value.clone()];
    }
    if selected.contains(value) {
        selected.iter().filter(|v| *v != value).cloned().collect()
    } else {
        let mut next = selected.to_vec();
        next.push(value.clone());
        next
    }
}

impl Describe for ToggleGroup {
    fn accessible(&self) -> Accessible {
        let role = if self.multiple {
            Role::Group
        } else {
            Role::RadioGroup
        };
        Accessible::new(role, self.name.to_string()).disabled(self.disabled)
    }
}

impl ToggleGroup {
    /// What each option reports, given the current selection.
    pub fn item_accessibles(&self, selected: &[SharedString]) -> Vec<Accessible> {
        let role = if self.multiple {
            Role::ToggleButton
        } else {
            Role::Radio
        };
        self.items
            .iter()
            .map(|i| {
                let name = i.name.as_ref().unwrap_or(&i.label);
                Accessible::new(role, name.to_string())
                    .selected(selected.contains(&i.value))
                    .disabled(self.disabled || i.disabled)
            })
            .collect()
    }
}

impl RenderOnce for ToggleGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let density = c.density(self.density);
        let frame = c.segmented(density);
        let fg = frame.geometry;
        let default = self.default_value.clone();
        let state = window.use_keyed_state(child_id(&self.id, "value"), cx, |_, _| default);
        let selected = self.value.clone().unwrap_or_else(|| state.read(cx).clone());
        let controlled = self.value.is_some();

        let enabled: Rc<Vec<usize>> = Rc::new(
            (0..self.items.len())
                .filter(|i| !self.disabled && !self.items[*i].disabled)
                .collect(),
        );
        // The one option in the Tab order: the first enabled selected one, else the first
        // enabled one, else none.
        let stop = enabled
            .iter()
            .copied()
            .find(|i| selected.contains(&self.items[*i].value))
            .or_else(|| enabled.first().copied());
        let handles: Rc<Vec<FocusHandle>> = Rc::new(
            (0..self.items.len())
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

        let (keys, key_enabled) = (handles.clone(), enabled.clone());
        let mut group = h_flex()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "group"))
            .relative()
            .h(px(fg.height))
            .rounded(px(fg.radius))
            .border(px(fg.border_width))
            .border_color(colour(frame.border))
            .text_role(fg.type_role)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let Some(current) = keys.iter().position(|h| h.is_focused(window)) else {
                    return;
                };
                let key = event.keystroke.key.as_str();
                if let Some(next) = roving_among(key, current, &key_enabled, Axis::Horizontal) {
                    move_focus(&keys[next], window);
                    cx.stop_propagation();
                }
            });

        // The focused option's ring is painted over the options after it, whose fills would
        // cover a ring of its own.
        let ring = RingTarget::default();
        let mut focus = None;
        let floor_cap = px(c.foundation().spacing.s6);
        for (i, item) in self.items.into_iter().enumerate() {
            let on = selected.contains(&item.value);
            let usable = enabled.contains(&i);
            let focused = usable && handles[i].is_focused(window);
            let states = States::NONE.selected(on).focused(focused).disabled(!usable);
            let s = c.segment(density, states);
            let g = s.geometry;
            #[cfg(test)]
            crate::drawn::record(&part_name(&self.id, &format!("item-{i}")), s);
            let icon_only = item.label.is_empty() && item.icon.is_some();
            let mut option = div()
                .id(child_id(&self.id, format!("item-{i}")))
                .debug_selector(|| part_name(&self.id, &format!("item-{i}")))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .control_geometry(&g);
            option = if icon_only {
                // An icon alone is a square segment.
                option.w(px(g.height)).px_0().flex_shrink_0()
            } else {
                // An option too long for the column shrinks and its label ends in `…`; it keeps
                // room for a letter and its ellipsis, or its label's own width when that is
                // shorter. GPUI would otherwise keep an option as wide as its whole label.
                let size = px(c.foundation().typography.style(g.type_role).size);
                let floor = text_width(window, &item.label, size).min(floor_cap);
                let icon = if item.icon.is_some() {
                    px(g.icon_size + g.gap)
                } else {
                    px(0.0)
                };
                option.min_w(floor + icon + px(g.padding_x + g.padding_x))
            };
            if let Some(glyph) = item.icon {
                let size = px(g.icon_size);
                option = option.child(
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
            if !icon_only {
                option = option.child(single_line(
                    div()
                        .debug_selector(|| part_name(&self.id, &format!("label-{i}")))
                        .min_w_0()
                        .child(item.label),
                ));
            }
            // Last, so hover and press keep the type role.
            option = pointer_states(option, &s.colours, s.responds, &Pointer::Own);
            if usable {
                let next = toggle(&selected, &item.value, self.multiple);
                let (state, on_change) = (state.clone(), self.on_change.clone());
                option = option.track_focus(&handles[i]).cursor_pointer().on_click(
                    move |_, window, cx| {
                        if !controlled {
                            state.update(cx, |v, cx| {
                                *v = next.clone();
                                cx.notify();
                            });
                        }
                        if let Some(f) = &on_change {
                            f(&next, window, cx);
                        }
                    },
                );
            }
            if s.focus_visible {
                option = option.child(ring.probe());
                focus = Some((s.focus, g.radius));
            }
            group = group.child(option);
        }
        if let Some((focus, radius)) = focus {
            group = group.child(ring.ring(focus, radius, &self.id));
        }
        // As wide as its options, not stretched across a column, and never wider than it.
        group.style().align_self = Some(gpui::AlignSelf::Start);
        group.max_w_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<SharedString> {
        v.iter()
            .map(|x| SharedString::from(x.to_string()))
            .collect()
    }

    #[test]
    fn single_selects_one_and_multiple_toggles() {
        let a = SharedString::from("a");
        assert_eq!(toggle(&s(&["b"]), &a, false), s(&["a"]));
        assert_eq!(toggle(&s(&["a"]), &a, false), s(&["a"]));
        assert_eq!(toggle(&s(&["b"]), &a, true), s(&["b", "a"]));
        assert_eq!(toggle(&s(&["a", "b"]), &a, true), s(&["b"]));
    }

    #[test]
    fn reports_radios_or_toggles() {
        let g = ToggleGroup::new("g", "Options")
            .item("l", "Left")
            .icon_item("r", "→", "Right");
        let a = g.item_accessibles(&s(&["r"]));
        assert_eq!((a[0].role, a[0].selected), (Role::Radio, Some(false)));
        assert_eq!((a[1].name.as_str(), a[1].selected), ("Right", Some(true)));
        assert_eq!(g.multiple(true).accessible().role, Role::Group);
    }
}
