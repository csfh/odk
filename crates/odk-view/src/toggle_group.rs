//! A segmented control on the Rune host: a row of options in one frame
//! (`docs/spec/odk-component-tokens.md`, `segmented`).

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Glyph;
use crate::ink::ink;
use crate::layout::inline;
use crate::style::{self, focus_ring};
use crate::text::{Flow, words};
use crate::toggle::{Toggle, pressable};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::Density;
use rune::{El, row};

/// What the application hears of a change: every option that is on, in the order they were
/// turned on.
type Changed<T> = Rc<dyn Fn(&mut T, &[String])>;

struct Item {
    value: String,
    label: String,
    /// Shown alone when the item has no label of its own (`icon_item`), else before the label.
    icon: Option<Glyph>,
    name: Option<String>,
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
/// option, and a disabled group is not a tab stop. A long label is cut with an ellipsis. A group
/// with no options draws nothing.
///
/// Uncontrolled by default, keeping its own selection from `default_value` in the host's store;
/// pass `value` to control it and follow `on_change`.
pub struct ToggleGroup<T> {
    id: String,
    name: String,
    items: Vec<Item>,
    multiple: bool,
    value: Option<Vec<String>>,
    default_value: Vec<String>,
    disabled: bool,
    density: Option<Density>,
    on_change: Option<Changed<T>>,
}

impl<T: 'static> ToggleGroup<T> {
    /// A group named `name` for assistive technology, such as "View"; it has no visible label
    /// of its own.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            items: Vec::new(),
            multiple: false,
            value: None,
            default_value: Vec::new(),
            disabled: false,
            density: None,
            on_change: None,
        }
    }

    /// Adds a text option: `value` is what `on_change` reports and `label` what the user reads.
    pub fn item(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(Item {
            value: value.into(),
            label: label.into(),
            icon: None,
            name: None,
            disabled: false,
        });
        self
    }

    /// Adds an option that is an icon alone. It has no text to be named by, so it takes its
    /// accessible name here.
    pub fn icon_item(
        mut self,
        value: impl Into<String>,
        icon: impl Into<Glyph>,
        name: impl Into<String>,
    ) -> Self {
        self.items.push(Item {
            value: value.into(),
            label: String::new(),
            icon: Some(icon.into()),
            name: Some(name.into()),
            disabled: false,
        });
        self
    }

    /// An icon before the label of the option with this value.
    pub fn icon(mut self, value: &str, glyph: impl Into<Glyph>) -> Self {
        let glyph = glyph.into();
        for item in self.items.iter_mut().filter(|i| i.value == value) {
            item.icon = Some(glyph.clone());
        }
        self
    }

    /// Disables the option with this value: it is shown but cannot be toggled or focused.
    pub fn disable(mut self, value: &str) -> Self {
        for item in self.items.iter_mut().filter(|i| i.value == value) {
            item.disabled = true;
        }
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Each option toggles on its own, in place of exactly one being on.
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// The group's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The options that are on, making the group controlled.
    pub fn value<S: Into<String>>(mut self, value: impl IntoIterator<Item = S>) -> Self {
        self.value = Some(value.into_iter().map(Into::into).collect());
        self
    }

    /// What is on at the start of an uncontrolled group.
    pub fn default_value<S: Into<String>>(mut self, value: impl IntoIterator<Item = S>) -> Self {
        self.default_value = value.into_iter().map(Into::into).collect();
        self
    }

    /// Hears the options that are on after each change.
    pub fn on_change(mut self, handler: impl Fn(&mut T, &[String]) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        // A group with no options draws nothing: no frame without segments.
        if self.items.is_empty() {
            return row();
        }
        let paint = style::paint(hosted);
        let density = paint.tokens.density(self.density);
        let frame = paint.tokens.segmented(density);
        let selected = self
            .value
            .clone()
            .or_else(|| hosted.ui.selection(&self.id).map(<[String]>::to_vec))
            .unwrap_or_else(|| self.default_value.clone());
        let enabled: Vec<usize> = (0..self.items.len())
            .filter(|i| !self.disabled && !self.items[*i].disabled)
            .collect();
        // The one option in the Tab order: the first enabled selected one, else the first
        // enabled one, else none.
        let stop = enabled
            .iter()
            .copied()
            .find(|i| selected.contains(&self.items[*i].value))
            .or_else(|| enabled.first().copied());

        // The frame is as high as its options and their edge: the edge is drawn inside it.
        let mut group = apply(row().key(self.id.clone()), &accessible)
            .focus_group()
            .h(frame.geometry.height)
            .padding(frame.geometry.border_width)
            .radius(frame.geometry.radius)
            .outline(
                frame.geometry.border_width,
                -frame.geometry.border_width,
                ink(frame.border),
            )
            .shrink();

        let (controlled, default) = (self.value.clone(), self.default_value.clone());
        for (index, item) in self.items.iter().enumerate() {
            let on = selected.contains(&item.value);
            let usable = enabled.contains(&index);
            let s = paint
                .tokens
                .segment(density, States::NONE.selected(on).disabled(!usable));
            let g = s.geometry;
            let icon_only = item.label.is_empty() && item.icon.is_some();
            let name = item.name.as_ref().unwrap_or(&item.label);
            let option_accessible = Accessible::new(Role::ToggleButton, name.clone())
                .selected(on)
                .disabled(!usable);
            let mut option = apply(
                row().key(format!("{}-{}", self.id, item.value)),
                &option_accessible,
            )
            .h(g.height)
            .px(g.padding_x)
            .gap(g.gap)
            .center()
            .radius(g.radius)
            .fill(ink(s.colours.default.background))
            .shrink()
            .focusable(usable)
            .focus_selected(Some(index) == stop)
            .no_focus_ring();
            if icon_only {
                // An icon alone is a square segment.
                option = option.w(g.height).px(0.0);
            }
            if s.responds {
                option = option
                    .hover(ink(s.colours.hover.background))
                    .press(ink(s.colours.pressed.background));
            }
            if let Some(glyph) = item.icon.clone() {
                option = option.child(glyph.slot(g.icon_size, ink(s.colours.default.foreground)));
            }
            if !icon_only {
                option = option.child(words(
                    &paint,
                    item.label.clone(),
                    g.type_role,
                    s.colours.default.foreground,
                    Flow::End,
                ));
            }
            option = focus_ring(option, &s.focus);
            if usable {
                let (id, value, multiple) = (self.id.clone(), item.value.clone(), self.multiple);
                let (controlled, default) = (controlled.clone(), default.clone());
                let change = self.on_change.clone();
                let act: Toggle<T> = Rc::new(move |hosted: &mut Hosted<T>| {
                    let current = controlled
                        .clone()
                        .or_else(|| hosted.ui.selection(&id).map(<[String]>::to_vec))
                        .unwrap_or_else(|| default.clone());
                    let next = toggled(&current, &value, multiple);
                    if controlled.is_none() {
                        hosted.ui.set_selection(&id, next.clone());
                    }
                    if let Some(change) = &change {
                        change(&mut hosted.app, &next);
                    }
                });
                // Space or Enter, on release, and a press.
                option = pressable(option, &format!("{}-{}", self.id, item.value), true, act);
            }
            // The option, and the focus group it is in, are what Tab and the arrows see.
            group = group.child(option);
        }
        // As wide as its options, not stretched across a column, and never wider than it.
        inline(group)
    }
}

impl<T> Describe for ToggleGroup<T> {
    fn accessible(&self) -> Accessible {
        let role = if self.multiple {
            Role::Group
        } else {
            Role::RadioGroup
        };
        Accessible::new(role, self.name.clone()).disabled(self.disabled)
    }
}

/// The options that are on once `value` is pressed: only it in single mode, and in multiple mode
/// each press turns it on or off.
fn toggled(current: &[String], value: &str, multiple: bool) -> Vec<String> {
    if !multiple {
        return vec![value.to_string()];
    }
    if current.iter().any(|v| v == value) {
        current.iter().filter(|v| *v != value).cloned().collect()
    } else {
        let mut next = current.to_vec();
        next.push(value.to_string());
        next
    }
}

#[cfg(test)]
mod tests {
    use super::toggled;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn single_selects_one_and_multiple_toggles() {
        assert_eq!(toggled(&s(&["b"]), "a", false), s(&["a"]));
        assert_eq!(toggled(&s(&["a"]), "a", false), s(&["a"]));
        assert_eq!(toggled(&s(&["b"]), "a", true), s(&["b", "a"]));
        assert_eq!(toggled(&s(&["a", "b"]), "a", true), s(&["b"]));
    }
}
