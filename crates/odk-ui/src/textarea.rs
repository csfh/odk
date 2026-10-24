//! A field of several lines (ADR 0038, `docs/spec/odk-text-input.md`): `Input`'s state and
//! element, drawn with rows that wrap and scroll up and down.

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window};
use odk_a11y::{Accessible, Describe};

use odk_theme::foundation::Density;

use crate::Input;
use crate::button::Size;

/// How many rows high a `Textarea` is unless `rows` says otherwise (ODK's own).
const DEFAULT_ROWS: usize = 3;

/// A field that takes several lines of text.
///
/// Tab reaches it once and Tab leaves it again; Escape is left to whatever is around it, so a
/// field in a `Dialog` still lets Escape close it. Enter ends a line. The arrows, Home and End
/// move the caret, with Shift they select, and with Ctrl they go by word or to the ends of the
/// text; Up and Down move between rows and keep their place across a short one, PageUp and
/// PageDown go a page, and Home and End go to the start and end of the line. Backspace and
/// Delete remove a grapheme, with Ctrl a word. Ctrl+A selects all, Ctrl+C, Ctrl+X and Ctrl+V
/// use the clipboard, Ctrl+Z undoes and Ctrl+Shift+Z or Ctrl+Y redoes. A click puts the caret,
/// a double-click selects a word, a triple-click a line, a drag selects on past the edges of
/// the field, the wheel scrolls it, and a middle-click pastes the primary selection. Reaching
/// it with Tab selects all its text.
///
/// Uncontrolled by default, starting from `default_value`; pass `value` to control it and
/// follow `on_change`. Text that is too long for a row wraps, and breaks inside a word that
/// does not fit. The box is `rows` rows high however much is typed, and the text scrolls
/// inside it; with `max_rows` it grows with its text up to that many rows first.
#[derive(IntoElement)]
pub struct Textarea {
    input: Input,
}

impl Textarea {
    /// A field with no visible label of its own: `name` is what it is called to assistive
    /// technology.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        let mut input = Input::new(id, name);
        input.rows = Some(DEFAULT_ROWS);
        Self { input }
    }

    /// How many rows high the field is, and so how much of its text shows at once. With
    /// `max_rows` it is the fewest rows the field shows.
    pub fn rows(mut self, rows: usize) -> Self {
        self.input.rows = Some(rows.max(1));
        self
    }

    /// The fewest rows the field shows: `rows` by another name, for a field that grows.
    pub fn min_rows(self, rows: usize) -> Self {
        self.rows(rows)
    }

    /// Lets the field grow with its text, a row at a time, from `rows` up to `rows` at most,
    /// then scroll. Without it the field stays `rows` high whatever it holds.
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.input.max_rows = Some(rows.max(1));
        self
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

    /// A step from the density the field would otherwise take, as for an `Input`.
    pub fn size(mut self, size: Size) -> Self {
        self.input = self.input.size(size);
        self
    }

    /// The field's own density: its padding and type, not its rows.
    pub fn density(mut self, density: Density) -> Self {
        self.input = self.input.density(density);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.input = self.input.disabled(disabled);
        self
    }

    /// Selectable and copyable, not editable.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.input = self.input.read_only(read_only);
        self
    }

    /// Dots instead of text, and nothing copied or reported.
    pub fn masked(mut self, masked: bool) -> Self {
        self.input = self.input.masked(masked);
        self
    }

    /// Marks the value as wrong: the danger edge and ring, as for an `Input`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.input = self.input.invalid(invalid);
        self
    }

    /// Reports that the field must be filled in.
    pub fn required(mut self, required: bool) -> Self {
        self.input = self.input.required(required);
        self
    }

    /// Called with the text after each edit; not while an IME composition is running, but
    /// when it ends.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_change(handler);
        self
    }
}

impl Textarea {
    pub(crate) fn input(self) -> Input {
        self.input
    }
}

impl Describe for Textarea {
    fn accessible(&self) -> Accessible {
        self.input.accessible()
    }
}

impl RenderOnce for Textarea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.input.render(window, cx)
    }
}
