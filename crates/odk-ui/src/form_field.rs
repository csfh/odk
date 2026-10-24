//! A control in a form, with its label, description and message (`docs/spec/odk-ui-components.md`,
//! "Form controls"): the one place a form's labels and messages are laid out and coloured, from
//! `form_field` and `form_message` (`docs/spec/odk-component-tokens.md`), so no control repeats
//! it.

use gpui::{
    AnyElement, App, Div, ElementId, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe};
use odk_theme::component::States;
use odk_theme::semantic::Role;

use crate::focus::{child_id, focus_handle, move_focus, part_name};
use crate::input::field_glyph;
use crate::layout::{StyledExt, h_flex, v_flex};
use crate::theme::colour;
use crate::{
    Checkbox, Input, InputGroup, NumberInput, PasswordInput, RadioGroup, SearchInput, Select,
    Switch, Textarea, components,
};

/// The word after the label of a field that may be left empty.
const OPTIONAL: &str = "(optional)";
/// The mark after the label of a field that must be filled in.
const REQUIRED: &str = "*";

/// What a `FormField` hands its control: the field's state and a focus handle the label can
/// move focus to. Its fields are the crate's own.
pub struct Fielded {
    invalid: bool,
    required: bool,
    disabled: bool,
    description: Option<SharedString>,
    focus: FocusHandle,
}

mod sealed {
    pub trait Sealed {}
}

/// A control a [`FormField`] can hold: every ODK form control. It is sealed: the field passes
/// its state through methods only ODK's controls have.
pub trait FieldControl: sealed::Sealed + Describe + 'static {
    /// The control as the field draws it, with the field's state applied.
    #[doc(hidden)]
    fn in_field(self: Box<Self>, field: Fielded) -> AnyElement;

    /// Whether a press on the field's label focuses the control. A check box, a switch and a
    /// radio group have labels of their own, and the field's label is their legend.
    #[doc(hidden)]
    fn label_focuses(&self) -> bool {
        true
    }
}

/// The message under a control.
struct Message {
    role: Role,
    text: SharedString,
}

/// A control with its label above it, an optional description under the label and an optional
/// message under the control:
///
/// ```text
/// Label *
/// Description
/// [ control ]
/// × Message
/// ```
///
/// A press on the label focuses a text field or a select, as an HTML label does; the keyboard
/// is the control's own, and Tab stops on the control, never on the label. `error` marks the
/// control invalid and says why, with the danger role's mark `×`, so an error never shows by
/// colour alone, also in a monochrome theme; `warning` and `success` have their own marks
/// (`!`, `✓`), and `help` is plain helper text. `required` puts `*` after the label and reports
/// the control required; `optional` puts `(optional)` there instead. The parts are the
/// foundation's `field_gap` (6 px) apart; put fields in a [`form`] to space them `form_gap`
/// (16 px) apart. The control reports the description and the message as its accessible
/// description.
#[derive(IntoElement)]
pub struct FormField {
    id: ElementId,
    label: SharedString,
    description: Option<SharedString>,
    message: Option<Message>,
    required: bool,
    optional: bool,
    disabled: bool,
    control: Option<Box<dyn FieldControl>>,
}

impl FormField {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
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
    pub fn child(mut self, control: impl FieldControl) -> Self {
        self.control = Some(Box::new(control));
        self
    }

    /// Text under the label that says more about the field.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Marks the control invalid and says what is wrong, under it.
    pub fn error(self, text: impl Into<SharedString>) -> Self {
        self.message(Role::Danger, text)
    }

    /// A caution under the control, which leaves it valid.
    pub fn warning(self, text: impl Into<SharedString>) -> Self {
        self.message(Role::Warning, text)
    }

    /// Confirms the value under the control, where that is useful, such as a name that is free.
    pub fn success(self, text: impl Into<SharedString>) -> Self {
        self.message(Role::Success, text)
    }

    /// Helper text under the control.
    pub fn help(self, text: impl Into<SharedString>) -> Self {
        self.message(Role::Neutral, text)
    }

    fn message(mut self, role: Role, text: impl Into<SharedString>) -> Self {
        self.message = Some(Message {
            role,
            text: text.into(),
        });
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
        self.message
            .as_ref()
            .is_some_and(|m| m.role == Role::Danger)
    }

    /// The description and the message, as the control's accessible description, read as
    /// sentences: a part that does not end one is ended with a full stop.
    fn accessible_description(&self) -> Option<SharedString> {
        let mut out = String::new();
        for part in self
            .description
            .iter()
            .chain(self.message.as_ref().map(|m| &m.text))
        {
            if !out.is_empty() {
                if !out.ends_with(['.', '!', '?']) {
                    out.push('.');
                }
                out.push(' ');
            }
            out.push_str(part);
        }
        (!out.is_empty()).then(|| out.into())
    }
}

impl Describe for FormField {
    /// What the control reports, with the field's state: invalid, required, disabled, and the
    /// description and message as its description.
    fn accessible(&self) -> Accessible {
        let mut a = match &self.control {
            Some(control) => control.accessible(),
            None => Accessible::new(odk_a11y::Role::Group, self.label.to_string()),
        };
        a.invalid |= self.invalid();
        a.required |= self.required;
        a.disabled |= self.disabled;
        if let Some(d) = self.accessible_description() {
            a.description = Some(d.to_string());
        }
        a
    }
}

/// A column of form fields and actions, `form_gap` (16 px) apart.
pub fn form() -> Div {
    v_flex().gap(px(crate::foundation().layout.form_gap))
}

impl RenderOnce for FormField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let described = self.control.as_ref().map(|c| c.accessible());
        let disabled = self.disabled || described.as_ref().is_some_and(|a| a.disabled);
        let required = self.required || described.as_ref().is_some_and(|a| a.required);
        let s = c.form_field(States::NONE.disabled(disabled));
        let focus = focus_handle(child_id(&self.id, "control-focus"), !disabled, window, cx);
        let label_focuses = self.control.as_ref().is_some_and(|c| c.label_focuses());
        let description = self.accessible_description();
        let invalid = self.invalid();

        let mut label = h_flex()
            .id(child_id(&self.id, "label"))
            .debug_selector(|| part_name(&self.id, "label"))
            .gap(px(c.foundation().spacing.s1))
            .text_role(s.label_role)
            .text_color(colour(s.label))
            .child(div().min_w_0().child(self.label));
        if required {
            label = label.child(
                div()
                    .debug_selector(|| part_name(&self.id, "required"))
                    .text_color(colour(s.required))
                    .child(REQUIRED),
            );
        } else if self.optional {
            label = label.child(
                div()
                    .debug_selector(|| part_name(&self.id, "optional"))
                    .text_role(s.description_role)
                    .text_color(colour(s.optional))
                    .child(OPTIONAL),
            );
        }
        if label_focuses && !disabled {
            let focus = focus.clone();
            label = label
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, window, _| {
                    move_focus(&focus, window);
                });
        }

        let mut field = v_flex()
            .debug_selector(|| part_name(&self.id, "field"))
            .w_full()
            .min_w_0()
            .gap(px(s.gap))
            .child(label);
        if let Some(text) = self.description {
            field = field.child(
                div()
                    .debug_selector(|| part_name(&self.id, "description"))
                    .text_role(s.description_role)
                    .text_color(colour(s.description))
                    .child(text),
            );
        }
        if let Some(control) = self.control {
            field = field.child(control.in_field(Fielded {
                invalid,
                required: self.required,
                disabled: self.disabled,
                description,
                focus,
            }));
        }
        if let Some(message) = self.message {
            let m = c.form_message(message.role);
            let mut row = h_flex()
                .debug_selector(|| part_name(&self.id, "message"))
                .items_start()
                .gap(px(c.foundation().spacing.s1))
                .text_role(s.message_role)
                .text_color(colour(m.colour));
            if let Some(mark) = m.mark {
                // The mark sits on the message's first line.
                let line = c.foundation().typography.style(s.message_role).line_height;
                row = row.child(
                    div()
                        .debug_selector(|| part_name(&self.id, "message-mark"))
                        .h(px(line))
                        .flex()
                        .items_center()
                        .child(field_glyph(mark.into(), s.icon_size, colour(m.colour))),
                );
            }
            field = field.child(row.child(div().min_w_0().child(message.text)));
        }
        field
    }
}

// The controls a field holds.

/// Applies a field's state to an `Input`, whatever kind of field it draws.
fn input_in_field(mut input: Input, f: Fielded) -> AnyElement {
    if f.invalid {
        input = input.invalid(true);
    }
    if f.required {
        input = input.required(true);
    }
    if f.disabled {
        input = input.disabled(true);
    }
    input.description = f.description;
    input.focus = Some(f.focus);
    input.into_any_element()
}

macro_rules! sealed {
    ($($t:ty),*) => { $(impl sealed::Sealed for $t {})* };
}
sealed!(
    Input,
    Textarea,
    SearchInput,
    PasswordInput,
    NumberInput,
    InputGroup,
    Select,
    Checkbox,
    Switch,
    RadioGroup
);

impl FieldControl for Input {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
        input_in_field(*self, f)
    }
}

impl FieldControl for Textarea {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
        input_in_field(self.input(), f)
    }
}

impl FieldControl for SearchInput {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
        input_in_field(self.input(), f)
    }
}

impl FieldControl for PasswordInput {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
        input_in_field(self.input(), f)
    }
}

impl FieldControl for NumberInput {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
        input_in_field(self.input(), f)
    }
}

impl FieldControl for InputGroup {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
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
        group = group.describe(f.description);
        group.focus = Some(f.focus);
        group.into_any_element()
    }
}

impl FieldControl for Select {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
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
        select.description = f.description;
        select.focus = Some(f.focus);
        select.into_any_element()
    }
}

impl FieldControl for Checkbox {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
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
        check.description = f.description;
        check.into_any_element()
    }

    fn label_focuses(&self) -> bool {
        false
    }
}

impl FieldControl for Switch {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
        let mut switch = *self;
        if f.disabled {
            switch = switch.disabled(true);
        }
        switch.description = f.description;
        switch.into_any_element()
    }

    fn label_focuses(&self) -> bool {
        false
    }
}

impl FieldControl for RadioGroup {
    fn in_field(self: Box<Self>, f: Fielded) -> AnyElement {
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
        group.description = f.description;
        group.into_any_element()
    }

    fn label_focuses(&self) -> bool {
        false
    }
}
