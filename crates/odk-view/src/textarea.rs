//! A field of several lines on the Rune host (`docs/spec/odk-text-input.md`).

use crate::host::Hosted;
use crate::input::Input;
use odk_a11y::{Accessible, Describe, Role};
use rune::El;

/// A field of `rows` lines. Text wraps at the box and breaks inside a word that is too long, Enter
/// ends a line, Shift+Enter too, and Tab leaves the field. The box starts `rows` high and grows
/// with its text to `max_rows`, and scrolls past that.
pub struct Textarea<C> {
    id: String,
    name: String,
    rows: usize,
    max_rows: Option<usize>,
    placeholder: Option<String>,
    default_value: Option<String>,
    invalid: bool,
    required: bool,
    disabled: bool,
    width: Option<f32>,
    on_change: C,
}

impl<C> Textarea<C> {
    /// `name` is the field's accessible name, such as "Notes".
    pub fn new(id: impl Into<String>, name: impl Into<String>, on_change: C) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            rows: 3,
            max_rows: None,
            placeholder: None,
            default_value: None,
            invalid: false,
            required: false,
            disabled: false,
            width: None,
            on_change,
        }
    }

    /// How many lines high the box starts. Three by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// The most lines the box grows to; past that it scrolls.
    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = Some(max_rows);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
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
}

/// What the field reports; the `Input` inside draws it.
impl<C> Describe for Textarea<C> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::TextInput, self.name.clone())
            .invalid(self.invalid)
            .required(self.required)
            .disabled(self.disabled)
    }
}

impl<C> Textarea<C> {
    pub fn show<T>(self, hosted: &Hosted<T>) -> El
    where
        T: 'static,
        C: Fn(&mut T, String) + Clone + 'static,
    {
        let Textarea {
            id,
            name,
            rows,
            max_rows,
            placeholder,
            default_value,
            invalid,
            required,
            disabled,
            width,
            on_change,
        } = self;
        let mut input = Input::new(id, name, on_change, |_: &mut T| {})
            .multiline(rows, max_rows)
            .invalid(invalid)
            .required(required)
            .disabled(disabled);
        if let Some(width) = width {
            input = input.width(width);
        }
        if let Some(placeholder) = placeholder {
            input = input.placeholder(placeholder);
        }
        if let Some(value) = default_value {
            input = input.default_value(value);
        }
        input.show(hosted)
    }
}
