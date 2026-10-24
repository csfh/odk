//! A password field: a masked `Input` with a button that shows and hides its text
//! (`docs/spec/odk-text-input.md`, "Search, password and number fields").

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window};
use odk_a11y::{Accessible, Describe};
use odk_theme::foundation::Density;

use crate::Input;
use crate::button::Size;

/// A field for a password: its text is dots, and a button at its end shows and hides it.
///
/// Everything an `Input` does with the keyboard it does, as a masked field: no copying, no
/// cutting, and word movement goes to the ends. The show button is a tab stop after the
/// field, which Enter and Space press, named "Show password" or "Hide password". Pressing it
/// keeps the text, the caret, the selection and the box's size, and a click on it keeps focus
/// in the field. While the text is shown it is still a password: it stays out of the
/// clipboard, the primary selection, the input method and `Describe`.
///
/// Uncontrolled by default, starting from `default_value`; pass `value` to control it and
/// follow `on_change`. Nothing in ODK logs or reports the text.
#[derive(IntoElement)]
pub struct PasswordInput {
    input: Input,
}

impl PasswordInput {
    /// A password field named `name` for assistive technology, such as "Password".
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        let mut input = Input::new(id, name).masked(true);
        input.extras.reveal = true;
        Self { input }
    }

    /// The text, making the field controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.input = self.input.value(value);
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.input = self.input.default_value(value);
        self
    }

    /// Shown, muted, while the field is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    /// Whether the show button is there; it is by default.
    pub fn revealable(mut self, revealable: bool) -> Self {
        self.input.extras.reveal = revealable;
        self
    }

    /// A step from the density the field would otherwise take, as for an `Input`.
    pub fn size(mut self, size: Size) -> Self {
        self.input = self.input.size(size);
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.input = self.input.density(density);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.input = self.input.disabled(disabled);
        self
    }

    /// Selectable but not editable, and still not copyable.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.input = self.input.read_only(read_only);
        self
    }

    /// Marks the password as wrong, as for an `Input`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.input = self.input.invalid(invalid);
        self
    }

    /// Reports that the field must be filled in.
    pub fn required(mut self, required: bool) -> Self {
        self.input = self.input.required(required);
        self
    }

    /// Called with the text after each edit.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_change(handler);
        self
    }

    /// Called with the text when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_submit(handler);
        self
    }

    pub(crate) fn input(self) -> Input {
        self.input
    }
}

impl Describe for PasswordInput {
    fn accessible(&self) -> Accessible {
        self.input.accessible()
    }
}

impl RenderOnce for PasswordInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.input.render(window, cx)
    }
}
