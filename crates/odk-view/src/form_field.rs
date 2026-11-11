//! A control with its label, description and message, on the Rune host
//! (`docs/spec/odk-ui-components.md`, "Form controls"): the one place a form's labels and
//! messages are laid out and coloured, from `form_field` and `form_message`
//! (`docs/spec/odk-component-tokens.md`), so no control repeats it.

use crate::checkbox::Checkbox;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::input::Input;
use crate::radio::RadioGroup;
use crate::select::Select;
use crate::style;
use crate::switch::Switch;
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role as A11yRole};
use odk_theme::component::States;
use odk_theme::semantic::Role;
use rune::{El, col, row};

/// The word after the label of a field that may be left empty.
const OPTIONAL: &str = "(optional)";
/// The mark after the label of a field that must be filled in.
const REQUIRED: &str = "*";

/// What a [`FormField`] hands its control: the field's state, and the description the control
/// reports. Its fields are the crate's own.
pub struct Fielded {
    invalid: bool,
    required: bool,
    disabled: bool,
    description: Option<String>,
}

mod sealed {
    pub trait Sealed {}
}

/// A control a [`FormField`] can hold: `Input`, `Select`, `Checkbox`, `Switch` and
/// `RadioGroup`. It is sealed: the field passes its state through methods only ODK's controls
/// have.
pub trait FieldControl<T>: sealed::Sealed + Describe + 'static {
    /// The control as the field draws it, with the field's state applied.
    #[doc(hidden)]
    fn in_field(self: Box<Self>, field: Fielded, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El;

    /// The text field a press on the label focuses, as an HTML label does. A check box, a switch
    /// and a radio group have labels of their own, and the field's label is their legend. Rune
    /// cannot move focus to any other element by key (`docs/checks.md`), so a select's label
    /// does not focus it.
    #[doc(hidden)]
    fn label_focuses(&self) -> Option<String> {
        None
    }
}

/// A control with its label above it, an optional description under the label and an optional
/// message under the control:
///
/// ```text
/// Label *
/// Description
/// [ control ]
/// ! Message
/// ```
///
/// `error` marks the control invalid and says why, with the danger role's mark, so an error
/// never shows by colour alone, also in a monochrome theme; `warning` and `success` have their
/// own marks, and `help` is plain helper text. `required` puts `*` after the label and reports
/// the control required; `optional` puts `(optional)` there instead. The parts are the
/// foundation's `field_gap` apart; put fields in a [`form`] to space them `form_gap` apart. The
/// control reports the description and the message as its accessible description.
pub struct FormField<T> {
    id: String,
    label: String,
    description: Option<String>,
    message: Option<(Role, String)>,
    required: bool,
    optional: bool,
    disabled: bool,
    control: Option<Box<dyn FieldControl<T>>>,
}

impl<T: 'static> FormField<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            message: None,
            required: false,
            optional: false,
            disabled: false,
            control: None,
        }
    }

    /// The control the field labels.
    pub fn child(mut self, control: impl FieldControl<T>) -> Self {
        self.control = Some(Box::new(control));
        self
    }

    /// Text under the label that says more about the field.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Marks the control invalid and says what is wrong, under it.
    pub fn error(self, text: impl Into<String>) -> Self {
        self.message(Role::Danger, text)
    }

    /// A caution under the control, which leaves it valid.
    pub fn warning(self, text: impl Into<String>) -> Self {
        self.message(Role::Warning, text)
    }

    /// Confirms the value under the control, where that is useful, such as a name that is free.
    pub fn success(self, text: impl Into<String>) -> Self {
        self.message(Role::Success, text)
    }

    /// Helper text under the control.
    pub fn help(self, text: impl Into<String>) -> Self {
        self.message(Role::Neutral, text)
    }

    fn message(mut self, role: Role, text: impl Into<String>) -> Self {
        self.message = Some((role, text.into()));
        self
    }

    /// The control must be filled in: `*` after the label, and the control reports it.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// The control may be left empty: `(optional)` after the label.
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// Disables the control, and draws the label and description disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn invalid(&self) -> bool {
        matches!(self.message, Some((Role::Danger, _)))
    }

    /// The description and the message, as the control's accessible description, read as
    /// sentences: a part that does not end one is ended with a full stop.
    fn accessible_description(&self) -> Option<String> {
        join_sentences(
            [
                self.description.as_deref(),
                self.message.as_ref().map(|(_, text)| text.as_str()),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El {
        let paint = style::paint(hosted.theme());
        let described = self.control.as_ref().map(|c| c.accessible());
        let disabled = self.disabled || described.as_ref().is_some_and(|a| a.disabled);
        let required = self.required || described.as_ref().is_some_and(|a| a.required);
        let s = paint.tokens.form_field(States::NONE.disabled(disabled));
        let description = self.accessible_description();
        let invalid = self.invalid();
        let focus_target = self.control.as_ref().and_then(|c| c.label_focuses());

        let mut label = row()
            .key(format!("{}-label", self.id))
            // The control names itself and carries the description, so a screen reader does
            // not meet the words a second time.
            .a11y_hidden()
            .gap(paint.tokens.foundation().spacing.s1)
            .child(words(&paint, self.label, s.label_role, s.label, Flow::Wrap));
        if required {
            label = label.child(words(&paint, REQUIRED, s.label_role, s.required, Flow::End));
        } else if self.optional {
            label = label.child(words(
                &paint,
                OPTIONAL,
                s.description_role,
                s.optional,
                Flow::End,
            ));
        }
        if let Some(target) = focus_target
            && !disabled
        {
            label = label.on_press(move |hosted: &mut Hosted<T>| {
                hosted.ui.input_edit(&target).focused = true;
            });
        }

        let mut field = col().key(self.id.clone()).w_full().gap(s.gap).child(label);
        if let Some(text) = self.description {
            field = field.child(words(
                &paint,
                text,
                s.description_role,
                s.description,
                Flow::Wrap,
            ));
        }
        if let Some(control) = self.control {
            field = field.child(control.in_field(
                Fielded {
                    invalid,
                    required: self.required,
                    disabled: self.disabled,
                    description,
                },
                hosted,
                layers,
            ));
        }
        if let Some((role, text)) = self.message {
            let m = paint.tokens.form_message(role);
            let line = paint.role(s.message_role).line_height;
            let mut message = row()
                .key(format!("{}-message", self.id))
                .a11y_hidden()
                .items_start()
                .gap(paint.tokens.foundation().spacing.s1);
            if let Some(mark) = Icon::status(role) {
                // The mark sits on the message's first line.
                message = message.child(
                    row()
                        .h(line)
                        .center_y()
                        .child(mark.mark(s.icon_size, ink(m.colour))),
                );
            }
            field = field.child(message.child(words(
                &paint,
                text,
                s.message_role,
                m.colour,
                Flow::Wrap,
            )));
        }
        field
    }
}

impl<T: 'static> Describe for FormField<T> {
    /// What the control reports, with the field's state: invalid, required, disabled, and the
    /// description and the message as its description.
    fn accessible(&self) -> Accessible {
        let mut accessible = match &self.control {
            Some(control) => control.accessible(),
            None => Accessible::new(A11yRole::Group, self.label.clone()),
        };
        accessible.invalid |= self.invalid();
        accessible.required |= self.required;
        accessible.disabled |= self.disabled;
        if let Some(description) = self.accessible_description() {
            accessible.description = Some(description);
        }
        accessible
    }
}

/// Joins `parts` into one description, as sentences: a part that does not end one is ended
/// with a full stop. Empty parts are skipped.
fn join_sentences<'a>(parts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut out = String::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() {
            if !out.ends_with(['.', '!', '?']) {
                out.push('.');
            }
            out.push(' ');
        }
        out.push_str(part);
    }
    (!out.is_empty()).then_some(out)
}

/// A column of form fields and actions, `form_gap` apart.
pub fn form<T>(hosted: &Hosted<T>) -> El {
    let paint = style::paint(hosted.theme());
    col().gap(paint.tokens.form_field(States::NONE).form_gap)
}

// The controls a field holds.

impl<C, S> sealed::Sealed for Input<C, S> {}

impl<T, C, S> FieldControl<T> for Input<C, S>
where
    T: 'static,
    C: Fn(&mut T, String) + Clone + 'static,
    S: Fn(&mut T) + Clone + 'static,
{
    fn in_field(self: Box<Self>, f: Fielded, hosted: &Hosted<T>, _: &mut Vec<El>) -> El {
        let mut input = *self;
        if f.invalid {
            input = input.invalid(true);
        }
        if f.required {
            input = input.required(true);
        }
        if f.disabled {
            input = input.disabled(true);
        }
        input.described(f.description).show(hosted)
    }

    fn label_focuses(&self) -> Option<String> {
        Some(self.id().to_string())
    }
}

impl<F> sealed::Sealed for Select<F> {}

impl<T, F> FieldControl<T> for Select<F>
where
    T: 'static,
    F: Fn(&mut T, usize) + Clone + 'static,
{
    fn in_field(self: Box<Self>, f: Fielded, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El {
        let mut select = *self;
        if f.invalid {
            select = select.invalid(true);
        }
        if f.required {
            select = select.required(true);
        }
        if f.disabled {
            select = select.disabled(true);
        }
        select.described(f.description).show(hosted, layers)
    }
}

impl<T> sealed::Sealed for Checkbox<T> {}

impl<T: 'static> FieldControl<T> for Checkbox<T> {
    fn in_field(self: Box<Self>, f: Fielded, hosted: &Hosted<T>, _: &mut Vec<El>) -> El {
        let mut check = *self;
        if f.invalid {
            check = check.invalid(true);
        }
        if f.required {
            check = check.required(true);
        }
        if f.disabled {
            check = check.disabled(true);
        }
        check.described(f.description).show(hosted)
    }
}

impl<T> sealed::Sealed for Switch<T> {}

impl<T: 'static> FieldControl<T> for Switch<T> {
    fn in_field(self: Box<Self>, f: Fielded, hosted: &Hosted<T>, _: &mut Vec<El>) -> El {
        let mut switch = *self;
        if f.disabled {
            switch = switch.disabled(true);
        }
        switch.described(f.description).show(hosted)
    }
}

impl<T> sealed::Sealed for RadioGroup<T> {}

impl<T: 'static> FieldControl<T> for RadioGroup<T> {
    fn in_field(self: Box<Self>, f: Fielded, hosted: &Hosted<T>, _: &mut Vec<El>) -> El {
        let mut group = *self;
        if f.invalid {
            group = group.invalid(true);
        }
        if f.required {
            group = group.required(true);
        }
        if f.disabled {
            group = group.disabled(true);
        }
        group.described(f.description).show(hosted)
    }
}

#[cfg(test)]
mod tests {
    use super::join_sentences;

    #[test]
    fn parts_are_read_as_sentences() {
        assert_eq!(join_sentences([]), None);
        assert_eq!(join_sentences(["", ""]), None);
        assert_eq!(join_sentences(["One"]).as_deref(), Some("One"));
        assert_eq!(
            join_sentences(["Pick a name", "Taken"]).as_deref(),
            Some("Pick a name. Taken")
        );
        assert_eq!(
            join_sentences(["Pick a name.", "Taken!", "Retry?", "Now"]).as_deref(),
            Some("Pick a name. Taken! Retry? Now")
        );
    }
}
