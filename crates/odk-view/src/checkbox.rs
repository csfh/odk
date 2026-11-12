//! A box that is checked or not, with a label beside it, on the Rune host.

use std::rc::Rc;

use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::style::{self, focus_ring};
use crate::toggle::{Change, Look, Toggle, labelled};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{Check, States};
use odk_theme::foundation::Density;
use rune::{El, el};

/// A box that is checked or not, with a label beside it.
///
/// Tab reaches the row and Space toggles it on its release; Enter does not, as for a native
/// checkbox. A press on the box or on the label toggles it once. The box is a square of its
/// density's icon size with the application's corners and a border that is always there, so
/// checking, focusing or pressing it moves nothing; its focus ring lies outside it, and a long
/// label wraps beside it.
///
/// Uncontrolled by default, starting from `default_checked` and keeping its own state in the host's
/// store; pass `checked` to control it and follow `on_change`, which hears each change once.
/// `indeterminate` shows a dash for a mixed group; a press then checks it. A ticked box shows a
/// tick and a mixed one a dash, never only a fill.
pub struct Checkbox<T> {
    id: String,
    label: String,
    checked: Option<bool>,
    default_checked: bool,
    indeterminate: bool,
    disabled: bool,
    invalid: bool,
    required: bool,
    description: Option<String>,
    density: Option<Density>,
    on_change: Option<Change<T>>,
}

impl<T: 'static> Checkbox<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked: None,
            default_checked: false,
            indeterminate: false,
            disabled: false,
            invalid: false,
            required: false,
            description: None,
            density: None,
            on_change: None,
        }
    }

    /// The text a form field gives its control, which the control reports as its description.
    pub(crate) fn described(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }

    /// Whether it is checked, making the checkbox controlled.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// What it starts as, while it keeps its own state.
    pub fn default_checked(mut self, checked: bool) -> Self {
        self.default_checked = checked;
        self
    }

    /// Hears the state a press made, once for each press.
    pub fn on_change(mut self, handler: impl Fn(&mut T, bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Shows a dash, for a box that stands for a group that is partly checked.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the choice as wrong: the danger edge and a mark after the label, so it never shows
    /// by colour alone. Say what is wrong in a form field's error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that the box must be checked, such as an agreement.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// The box's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let on = self
            .checked
            .unwrap_or_else(|| hosted.ui.toggle_value(&self.id, self.default_checked));
        let accessible = self.accessible_at(on);
        let paint = style::paint(hosted);
        let states = States::NONE
            .disabled(self.disabled)
            .checked(on)
            .indeterminate(self.indeterminate)
            .invalid(self.invalid);
        let s = paint
            .tokens
            .checkbox(paint.tokens.density(self.density), states);
        let g = s.geometry;
        let mark = match s.check {
            Check::Indeterminate => Some(Icon::Minus),
            Check::Checked => Some(Icon::Check),
            Check::Unchecked => None,
        };
        let mut check = el()
            .size(g.size, g.size)
            .center()
            .radius(g.shape.corner_radius(g.size, g.size))
            .fill(ink(s.colours.default.background))
            // The edge is drawn inside the box, which is `size` with its borders.
            .outline(
                g.border_width,
                -g.border_width,
                ink(s.colours.default.border),
            )
            .a11y_hidden();
        if s.responds {
            check = check
                .hover(ink(s.colours.hover.background))
                .press(ink(s.colours.pressed.background));
        }
        if let Some(icon) = mark {
            check = check.child(icon.mark(g.mark, ink(s.colours.default.foreground)));
        }
        let check = focus_ring(check, &s.focus);

        let (id, controlled, default) = (self.id.clone(), self.checked, self.default_checked);
        let (indeterminate, change) = (self.indeterminate, self.on_change);
        let toggle: Toggle<T> = Rc::new(move |hosted: &mut Hosted<T>| {
            let current = controlled.unwrap_or_else(|| hosted.ui.toggle_value(&id, default));
            let next = pressed(current, indeterminate);
            if controlled.is_none() {
                hosted.ui.set_toggle(&id, next);
            }
            if let Some(change) = &change {
                change(&mut hosted.app, next);
            }
        });
        let look = Look {
            colour: s.label,
            role: g.label,
            gap: g.gap,
            mark: s.invalid_mark,
            enter: false,
            stop: None,
        };
        labelled(
            &paint,
            &accessible,
            &self.id,
            self.label,
            check,
            look,
            toggle,
        )
    }
}

impl<T> Checkbox<T> {
    /// How it describes itself while it is `on`.
    fn accessible_at(&self, on: bool) -> Accessible {
        let accessible = Accessible::new(Role::Checkbox, self.label.clone())
            .selected(on && !self.indeterminate)
            .mixed(self.indeterminate)
            .disabled(self.disabled)
            .invalid(self.invalid)
            .required(self.required);
        match &self.description {
            Some(description) => accessible.description(description.clone()),
            None => accessible,
        }
    }
}

impl<T> Describe for Checkbox<T> {
    fn accessible(&self) -> Accessible {
        self.accessible_at(self.checked.unwrap_or(self.default_checked))
    }
}

/// What a press changes a checkbox to: on, from off or from mixed; off, from on.
fn pressed(checked: bool, indeterminate: bool) -> bool {
    indeterminate || !checked
}

#[cfg(test)]
mod tests {
    use super::pressed;

    #[test]
    fn a_press_checks_from_off_or_mixed_and_unchecks_from_on() {
        assert!(pressed(false, false));
        assert!(!pressed(true, false));
        assert!(pressed(false, true));
        assert!(pressed(true, true));
    }
}
