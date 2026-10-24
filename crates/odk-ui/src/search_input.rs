//! A search field: an `Input` with a search icon, a clear button and Escape to clear
//! (`docs/spec/odk-text-input.md`, "Search, password and number fields").

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window};
use odk_a11y::{Accessible, Describe};
use odk_theme::foundation::Density;

use crate::Input;
use crate::button::Size;

/// The search icon. ODK has no icon set yet, so it is a character.
const SEARCH_GLYPH: &str = "⌕";

/// A field for a search: a search icon before the text, and a clear button after it while
/// there is text.
///
/// Everything an `Input` does with the keyboard it does. Enter calls `on_submit`. Escape
/// clears the text while there is any, as one edit Ctrl+Z undoes; in an empty field Escape is
/// left to what is around it, so it still closes a `Dialog`. The clear button is not a tab
/// stop, since Escape does its job; a click on it keeps focus in the field. A
/// `shortcut` is shown at the end while the field is empty and not focused; it only labels the
/// field, and the application binds the key. `loading` shows a spinner at the end while results
/// are on their way.
///
/// Uncontrolled by default, starting from `default_value`; pass `value` to control it and
/// follow `on_change`. It looks like every other field: the same box, density and states.
#[derive(IntoElement)]
pub struct SearchInput {
    input: Input,
}

impl SearchInput {
    /// A search field named `name` for assistive technology, such as "Search settings".
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        let mut input = Input::new(id, name)
            .leading_icon(SEARCH_GLYPH)
            .clearable(true);
        input.extras.escape_clears = true;
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

    /// Shown, muted, while the field is empty, such as `Search…`.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    /// The key chord that reaches the field, such as `Ctrl+K`, shown while it is empty.
    pub fn shortcut(mut self, chord: impl Into<SharedString>) -> Self {
        self.input.extras.shortcut = Some(chord.into());
        self
    }

    /// Shows a spinner at the end, and reports the field busy, while results are on their way.
    /// The field stays editable.
    pub fn loading(mut self, loading: bool) -> Self {
        self.input.extras.loading = loading;
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

    /// Called with the text after each edit, the clear button's and Escape's included.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_change(handler);
        self
    }

    /// Called with the text when Enter is pressed: the search to run.
    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_submit(handler);
        self
    }

    pub(crate) fn input(self) -> Input {
        self.input
    }
}

impl Describe for SearchInput {
    fn accessible(&self) -> Accessible {
        self.input.accessible()
    }
}

impl RenderOnce for SearchInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.input.render(window, cx)
    }
}
