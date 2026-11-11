//! A password field on the Rune host: masked text, a show button, and nothing written to the
//! clipboard or the primary selection (`docs/spec/odk-text-input.md`).

use std::rc::Rc;

use crate::button::Button;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::input::Input;
use odk_theme::component::ButtonVariant;
use rune::El;

/// A field for a secret. Its text is dots, with no copy, cut or primary selection, and a screen
/// reader hears no value. A button after the text, a tab stop, shows it and hides it again; it
/// keeps the text, the caret and the selection. Typing, pasting and the editing keys work as in
/// any field.
pub struct PasswordInput<C, S> {
    id: String,
    name: String,
    placeholder: Option<String>,
    revealable: bool,
    invalid: bool,
    required: bool,
    disabled: bool,
    width: Option<f32>,
    on_change: C,
    on_submit: S,
}

impl<C, S> PasswordInput<C, S> {
    /// `name` is the field's accessible name, such as "Password".
    pub fn new(id: impl Into<String>, name: impl Into<String>, on_change: C, on_submit: S) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            placeholder: None,
            revealable: true,
            invalid: false,
            required: false,
            disabled: false,
            width: None,
            on_change,
            on_submit,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Whether the show button is there. On by default.
    pub fn revealable(mut self, revealable: bool) -> Self {
        self.revealable = revealable;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The box's width in points at the base text size.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El
    where
        T: 'static,
        C: Fn(&mut T, String) + Clone + 'static,
        S: Fn(&mut T) + Clone + 'static,
    {
        let PasswordInput {
            id,
            name,
            placeholder,
            revealable,
            invalid,
            required,
            disabled,
            width,
            on_change,
            on_submit,
        } = self;
        let reveal_id = format!("{id}-reveal");
        let revealed = hosted.ui.toggle_value(&reveal_id, false);
        let mut input = Input::new(id, name, on_change, on_submit)
            .secure(revealed)
            .invalid(invalid)
            .required(required)
            .disabled(disabled);
        if let Some(width) = width {
            input = input.width(width);
        }
        if let Some(placeholder) = placeholder {
            input = input.placeholder(placeholder);
        }
        if revealable {
            let (icon, label) = if revealed {
                (Icon::EyeOff, "Hide password")
            } else {
                (Icon::Eye, "Show password")
            };
            let toggle = reveal_id.clone();
            input = input.trailing(
                Button::icon(reveal_id, icon, label, ())
                    .variant(ButtonVariant::Ghost)
                    .in_field()
                    .disabled(disabled)
                    .show_hosted(
                        hosted,
                        Rc::new(move |hosted: &mut Hosted<T>| {
                            let now = hosted.ui.toggle_value(&toggle, false);
                            hosted.ui.set_toggle(&toggle, !now);
                        }),
                    ),
            );
        }
        input.show(hosted)
    }
}
