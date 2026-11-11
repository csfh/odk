//! A search field on the Rune host: the field with a search icon, a clear button and Escape that
//! clears (`docs/spec/odk-text-input.md`).

use std::rc::Rc;

use crate::button::{Button, Press};
use crate::host::Hosted;
use crate::icon::Icon;
use crate::input::{Input, KeyHook};
use crate::kbd::Kbd;
use crate::spinner::Spinner;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ButtonVariant;
use rune::{El, Key, KeyEvent, TextEdit, Tx};

/// A field for searching: a search icon first, and after the text a clear button while there is
/// text, a shortcut hint while there is none, or a spinner while `loading`. The clear button is not
/// a tab stop and a press on it leaves focus in the field. Escape clears a field that has text, as
/// one edit, and leaves an empty field's Escape to whatever is open around it.
pub struct SearchInput<C, S> {
    id: String,
    name: String,
    placeholder: String,
    default_value: Option<String>,
    shortcut: Option<String>,
    loading: bool,
    disabled: bool,
    width: Option<f32>,
    on_change: C,
    on_submit: S,
}

impl<C, S> SearchInput<C, S> {
    /// `name` is the field's accessible name, such as "Search files".
    pub fn new(id: impl Into<String>, name: impl Into<String>, on_change: C, on_submit: S) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            placeholder: "Search…".into(),
            default_value: None,
            shortcut: None,
            loading: false,
            disabled: false,
            width: None,
            on_change,
            on_submit,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// A hint of the chord that focuses the field, shown while it is empty.
    pub fn shortcut(mut self, chord: impl Into<String>) -> Self {
        self.shortcut = Some(chord.into());
        self
    }

    /// Shows a spinner at the end while a search is running.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
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
        let SearchInput {
            id,
            name,
            placeholder,
            default_value,
            shortcut,
            loading,
            disabled,
            width,
            on_change,
            on_submit,
        } = self;
        let has_text = !hosted.ui.input_text(&id).is_empty();
        // Clearing is one edit that the application hears about like any other.
        let clear: Press<T> = {
            let (id, on_change) = (id.clone(), on_change.clone());
            Rc::new(move |hosted: &mut Hosted<T>| {
                let edit = hosted.ui.input_edit(&id);
                // Emptied, but still the field that has focus.
                let focused = edit.focused;
                *edit = TextEdit::new("");
                edit.focused = focused;
                on_change(&mut hosted.app, String::new());
            })
        };
        let hook: KeyHook<T> = {
            let (id, clear) = (id.clone(), clear.clone());
            Rc::new(
                move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx, up: bool| {
                    if ev.key != Key::Escape || hosted.ui.input_text(&id).is_empty() {
                        return false;
                    }
                    if !up {
                        clear(hosted);
                    }
                    true
                },
            )
        };
        let mut input = Input::new(id.clone(), name, on_change, on_submit)
            .placeholder(placeholder)
            .leading_icon(Icon::Search)
            .disabled(disabled);
        if let Some(width) = width {
            input = input.width(width);
        }
        if let Some(value) = default_value {
            input = input.default_value(value);
        }
        if loading {
            input = input.trailing(Spinner::new().label("Searching").show(hosted));
        }
        if has_text && !disabled {
            input = input.trailing(
                Button::icon(format!("{id}-clear"), Icon::Close, "Clear", ())
                    .variant(ButtonVariant::Ghost)
                    .in_field()
                    .tab_stop(false)
                    .show_hosted(hosted, clear),
            );
        } else if let Some(chord) = shortcut.filter(|_| !disabled && !loading) {
            input = input.trailing(Kbd::new(&chord).show(hosted));
        }
        input.show_with(hosted, Some(hook))
    }
}

/// What the field reports; the `Input` inside draws it.
impl<C, S> Describe for SearchInput<C, S> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::TextInput, self.name.clone()).disabled(self.disabled)
    }
}
