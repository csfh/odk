//! A group of radios, of which one is chosen (`docs/spec/odk-ui-components.md`, "Form
//! controls"), on the radio's component tokens: a check box's colours on a circle.

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, focus_ring};
use crate::toggle::{Look, Toggle, labelled};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::{Density, TypeRole};
use rune::{El, col, el, row};

/// What the application hears when an option is chosen: its value.
type Chosen<T> = Rc<dyn Fn(&mut T, &str)>;

struct Item {
    value: String,
    label: String,
    disabled: bool,
}

/// Options of which exactly one is chosen, each a circle with its label beside it.
///
/// The group is one tab stop: Tab reaches the chosen option, or the first enabled one when none
/// is. The arrow keys move to the next or previous enabled option, wrapping, and choose it, as
/// native radios do; Home and End go to the first and last. Space chooses the option that has
/// focus; Enter does not. A press on a circle or a label chooses that option and gives it
/// focus. Disabled options are skipped and cannot be chosen. Rune does the moving; the group
/// says which options are its members and which one Tab visits.
///
/// Uncontrolled by default, keeping its own choice from `default_value` in the host's store;
/// pass `value` to control it and follow `on_change`, which hears each change once. The chosen
/// circle is the accent's fill with a dot, so it reads without its colour, as in a monochrome
/// theme.
pub struct RadioGroup<T> {
    id: String,
    name: String,
    items: Vec<Item>,
    value: Option<String>,
    default_value: Option<String>,
    disabled: bool,
    invalid: bool,
    required: bool,
    horizontal: bool,
    density: Option<Density>,
    on_change: Option<Chosen<T>>,
}

impl<T: 'static> RadioGroup<T> {
    /// A group named `name` for assistive technology, such as "Theme"; a form field gives it a
    /// visible label.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            items: Vec::new(),
            value: None,
            default_value: None,
            disabled: false,
            invalid: false,
            required: false,
            horizontal: false,
            density: None,
            on_change: None,
        }
    }

    /// Adds an option: `value` is what `on_change` reports and `label` what the user reads.
    pub fn item(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(Item {
            value: value.into(),
            label: label.into(),
            disabled: false,
        });
        self
    }

    /// Disables the option with this value: it is shown but cannot be chosen or focused.
    pub fn disable(mut self, value: &str) -> Self {
        for item in self.items.iter_mut().filter(|i| i.value == value) {
            item.disabled = true;
        }
        self
    }

    /// The chosen option's value, making the group controlled.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// The first choice of an uncontrolled group.
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the choice as wrong: every circle takes the danger edge, and a mark follows the first
    /// option, so it never shows by colour alone. Say what is wrong in a form field's error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that an option must be chosen.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Lays the options out in a row rather than a column. The arrow keys work either way.
    pub fn horizontal(mut self, horizontal: bool) -> Self {
        self.horizontal = horizontal;
        self
    }

    /// The group's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// Called with the value of the option the user chose, when it differs from the chosen one.
    pub fn on_change(mut self, handler: impl Fn(&mut T, &str) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// What each option reports, given the chosen value.
    fn item_accessibles(&self, chosen: Option<&str>) -> Vec<Accessible> {
        self.items
            .iter()
            .map(|i| {
                Accessible::new(Role::Radio, i.label.clone())
                    .selected(chosen == Some(i.value.as_str()))
                    .disabled(self.disabled || i.disabled)
            })
            .collect()
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let density = paint.tokens.density(self.density);
        let chosen = self
            .value
            .clone()
            .or_else(|| hosted.ui.choice(&self.id).map(str::to_string))
            .or_else(|| self.default_value.clone());
        let enabled: Vec<usize> = (0..self.items.len())
            .filter(|i| !self.disabled && !self.items[*i].disabled)
            .collect();
        // The one option in the Tab order: the chosen one if it is enabled, else the first
        // enabled one.
        let stop = enabled
            .iter()
            .copied()
            .find(|i| chosen.as_deref() == Some(self.items[*i].value.as_str()))
            .or_else(|| enabled.first().copied());
        let accessibles = self.item_accessibles(chosen.as_deref());
        let geometry = paint.tokens.radio(density, States::NONE).geometry;

        let mut group = if self.horizontal {
            row()
                .items_start()
                .flex_wrap()
                .gap(paint.tokens.foundation().layout.form_gap)
        } else {
            // At small text the line is shorter than the target. The gap makes up the
            // difference.
            let line = paint.role(TypeRole::Body).line_height;
            let target = paint.tokens.foundation().geometry.target_min;
            col().gap(geometry.gap.max(target - line).max(0.0))
        };
        group = apply(group.key(self.id.clone()), &accessible).focus_group();

        let (controlled, default) = (self.value.clone(), self.default_value.clone());
        for (index, (item, accessible)) in self.items.iter().zip(accessibles).enumerate() {
            let on = chosen.as_deref() == Some(item.value.as_str());
            let s = paint.tokens.radio(
                density,
                States::NONE
                    .disabled(self.disabled || item.disabled)
                    .checked(on)
                    .invalid(self.invalid),
            );
            let g = s.geometry;
            let mut circle = el()
                .size(g.size, g.size)
                .center()
                .radius(g.shape.corner_radius(g.size, g.size))
                .fill(ink(s.colours.default.background))
                // The edge is drawn inside the circle, which is `size` with its borders.
                .outline(
                    g.border_width,
                    -g.border_width,
                    ink(s.colours.default.border),
                )
                .a11y_hidden();
            if s.responds {
                circle = circle
                    .hover(ink(s.colours.hover.background))
                    .press(ink(s.colours.pressed.background));
            }
            if on {
                circle = circle.child(
                    el().size(g.dot, g.dot)
                        .radius(g.dot / 2.0)
                        .fill(ink(s.colours.default.foreground))
                        .a11y_hidden(),
                );
            }
            let circle = focus_ring(circle, &s.focus);

            let (id, value) = (self.id.clone(), item.value.clone());
            let (controlled, default) = (controlled.clone(), default.clone());
            let change = self.on_change.clone();
            let choose: Toggle<T> = Rc::new(move |hosted: &mut Hosted<T>| {
                let current = controlled
                    .clone()
                    .or_else(|| hosted.ui.choice(&id).map(str::to_string))
                    .or_else(|| default.clone());
                if current.as_deref() == Some(value.as_str()) {
                    return;
                }
                if controlled.is_none() {
                    hosted.ui.set_choice(&id, &value);
                }
                if let Some(change) = &change {
                    change(&mut hosted.app, &value);
                }
            });
            let look = Look {
                colour: s.label,
                role: g.label,
                gap: g.gap,
                // The wrong-choice mark follows the first option.
                mark: if index == 0 { s.invalid_mark } else { None },
                enter: false,
                stop: Some(Some(index) == stop),
            };
            group = group.child(labelled(
                &paint,
                &accessible,
                &format!("{}-{}", self.id, item.value),
                item.label.clone(),
                circle,
                look,
                choose,
            ));
        }
        group
    }
}

impl<T> Describe for RadioGroup<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::RadioGroup, self.name.clone())
            .disabled(self.disabled)
            .invalid(self.invalid)
            .required(self.required)
    }
}
