//! A field with text before and after it and buttons at its end, in one box on the Rune host
//! (`docs/spec/odk-text-input.md`).

use crate::button::Button;
use crate::describe::pointer_target;
use crate::host::Hosted;
use crate::ink::ink;
use crate::input::Input;
use crate::style::{self, text_style};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use rune::{El, row, text};

type Action<T> = Box<dyn FnOnce(&Hosted<T>) -> El>;

/// What an addon may take of the box, so a long one never crowds the field out.
const ADDON_SHARE: f32 = 0.25;

/// The box's width in points at the base text size, unless the application sets one.
const DEFAULT_WIDTH: f32 = 240.0;

/// A field with a `prefix` and a `suffix` of quiet text, such as `https://` and `.com`, and
/// buttons after them. The addons are decoration: a screen reader does not read them, they
/// truncate with `…` at a quarter of the box, and a press on one focuses the field. An action is
/// a button inside the box that leaves focus, caret and selection in the field.
pub struct InputGroup<T, C, S> {
    id: String,
    name: String,
    prefix: Option<String>,
    suffix: Option<String>,
    actions: Vec<Action<T>>,
    placeholder: Option<String>,
    default_value: Option<String>,
    masked: bool,
    invalid: bool,
    required: bool,
    disabled: bool,
    width: f32,
    on_change: C,
    on_submit: S,
}

impl<T: 'static, C, S> InputGroup<T, C, S> {
    /// `name` is the field's accessible name.
    pub fn new(id: impl Into<String>, name: impl Into<String>, on_change: C, on_submit: S) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            prefix: None,
            suffix: None,
            actions: Vec::new(),
            placeholder: None,
            default_value: None,
            masked: false,
            invalid: false,
            required: false,
            disabled: false,
            width: DEFAULT_WIDTH,
            on_change,
            on_submit,
        }
    }

    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// A button after the text, drawn inside the box as a field action; the group's disabled
    /// state is its own to set on the button.
    pub fn action<F: Fn(&mut T) + 'static>(mut self, button: Button<F>) -> Self {
        self.actions
            .push(Box::new(move |hosted| button.in_field().show(hosted)));
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

    /// Masks the text, with no copy, cut or primary selection.
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
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
        self.width = width;
        self
    }
}

/// What the field reports; the `Input` inside draws it, and the addons are decoration.
impl<T, C, S> Describe for InputGroup<T, C, S> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::TextInput, self.name.clone())
            .invalid(self.invalid)
            .required(self.required)
            .disabled(self.disabled)
    }
}

impl<T: 'static, C, S> InputGroup<T, C, S>
where
    C: Fn(&mut T, String) + Clone + 'static,
    S: Fn(&mut T) + Clone + 'static,
{
    pub fn show(self, hosted: &Hosted<T>) -> El {
        let InputGroup {
            id,
            name,
            prefix,
            suffix,
            actions,
            placeholder,
            default_value,
            masked,
            invalid,
            required,
            disabled,
            width,
            on_change,
            on_submit,
        } = self;
        let paint = style::paint(hosted);
        let field = paint.tokens.field(
            paint.tokens.density(None),
            States::NONE.disabled(disabled).invalid(invalid),
        );
        let limit = paint.px(width) * ADDON_SHARE;
        // A press on an addon puts focus in the field, as a label does.
        let addon = |label: String, key: String, before: bool| {
            let target = id.clone();
            let gap = field.geometry.gap;
            let element = row()
                .key(key)
                .max_w(limit)
                .shrink()
                .center_y()
                .child(
                    text(label)
                        .font(text_style(paint.body))
                        .color(ink(field.addon))
                        .truncate()
                        .a11y_hidden(),
                )
                .on_press(move |hosted: &mut Hosted<T>| {
                    hosted.ui.input_edit(&target).focused = true;
                });
            // A press focuses the field: a target for the pointer, not a control of its own.
            let element = pointer_target(element);
            if before {
                element.pr(gap)
            } else {
                element.pl(gap)
            }
        };
        let mut input = Input::new(id.clone(), name, on_change, on_submit)
            .width(width)
            .invalid(invalid)
            .required(required)
            .disabled(disabled);
        if let Some(prefix) = prefix {
            input = input.before(addon(prefix, format!("{id}-prefix"), true));
        }
        if let Some(suffix) = suffix {
            input = input.trailing(addon(suffix, format!("{id}-suffix"), false));
        }
        for action in actions {
            input = input.trailing(action(hosted));
        }
        if let Some(placeholder) = placeholder {
            input = input.placeholder(placeholder);
        }
        if let Some(value) = default_value {
            input = input.default_value(value);
        }
        if masked {
            input = input.secure(false);
        }
        input.show(hosted)
    }
}
