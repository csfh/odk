//! A text field (ADR 0038, `docs/spec/odk-text-input.md`): the editing model of `editor.rs`
//! behind GPUI's `EntityInputHandler`, a custom element that draws it, and the `Input`
//! component around both. `Textarea` is the same state and element with several lines
//! (`textarea.rs`).

use std::borrow::Cow;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    Action, App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, Div, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, GlobalElementId,
    Hitbox, HitboxBehavior, InspectorElementId, InteractiveElement, IntoElement, KeyBinding,
    KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    ParentElement, Pixels, Point, RenderOnce, Rgba, ScrollWheelEvent, SharedString, Stateful,
    Style, Styled, Subscription, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div,
    fill, point, px, relative, size,
};
use odk_a11y::{Accessible, Describe, Role};

use odk_theme::component::{
    ButtonVariant, ComponentTokens, FIELD_INVALID_MARK, FieldStyle, States,
};
use odk_theme::foundation::Density;

use crate::button::Size;
use crate::editor::Editor;
use crate::focus::{child_id, focus_handle, move_focus, part_name};
use crate::interaction::{Pointer, focus_ring, pointer_states};
use crate::layout::{StyledExt, single_line};
use crate::number::Rules;
use crate::text_layout::{Shown, TextLayout};
use crate::theme::{colour, mono};
use crate::{Button, Kbd, Spinner, components};

actions!(
    odk_text,
    [
        Left,
        Right,
        SelectLeft,
        SelectRight,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        LineStart,
        LineEnd,
        SelectLineStart,
        SelectLineEnd,
        TextStart,
        TextEnd,
        SelectTextStart,
        SelectTextEnd,
        Backspace,
        Delete,
        BackspaceWord,
        DeleteWord,
        RowUp,
        RowDown,
        SelectRowUp,
        SelectRowDown,
        PageUp,
        PageDown,
        SelectPageUp,
        SelectPageDown,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Submit,
        StepUp,
        StepDown,
    ]
);

/// The key context of a text input. The bindings below apply only inside it, so Tab and
/// Escape reach what is around the field.
const KEY_CONTEXT: &str = "TextInput";

/// The extra key context of a `Textarea`, which alone binds the keys that move between rows.
/// An `Input` leaves them to what is around it, such as the list of a `Command`.
const MULTILINE_CONTEXT: &str = "Textarea";

/// Both contexts, which is what a `Textarea` sets.
const MULTILINE_CONTEXTS: &str = "TextInput Textarea";

/// The extra key context of a `NumberInput`, which alone binds Up and Down to its steps.
const NUMBER_CONTEXT: &str = "NumberInput";

/// What a `NumberInput` sets.
const NUMBER_CONTEXTS: &str = "TextInput NumberInput";

/// The caret is this many border widths wide (ODK's own).
const CARET_BORDERS: f32 = 2.0;

/// A field of rows wraps its text to at least this width, however narrow the box (ODK's own).
const MIN_WRAP_WIDTH: f32 = 1.0;

/// A line break the selection covers is drawn this many ems wide (ODK's own), so that a
/// selected empty line shows.
const BREAK_EMS: f32 = 0.4;

type ValueHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// The bindings of every text input, installed by [`crate::run`] with the others.
pub(crate) fn bindings() -> Vec<KeyBinding> {
    let ctx = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("home", LineStart, ctx),
        KeyBinding::new("end", LineEnd, ctx),
        KeyBinding::new("shift-home", SelectLineStart, ctx),
        KeyBinding::new("shift-end", SelectLineEnd, ctx),
        KeyBinding::new("ctrl-home", TextStart, ctx),
        KeyBinding::new("ctrl-end", TextEnd, ctx),
        KeyBinding::new("ctrl-shift-home", SelectTextStart, ctx),
        KeyBinding::new("ctrl-shift-end", SelectTextEnd, ctx),
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("ctrl-backspace", BackspaceWord, ctx),
        KeyBinding::new("ctrl-delete", DeleteWord, ctx),
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("ctrl-z", Undo, ctx),
        KeyBinding::new("ctrl-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-y", Redo, ctx),
        KeyBinding::new("enter", Submit, ctx),
        // Only a `Textarea` has rows to move between.
        KeyBinding::new("up", RowUp, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("down", RowDown, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("shift-up", SelectRowUp, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("shift-down", SelectRowDown, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("pageup", PageUp, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("pagedown", PageDown, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("shift-pageup", SelectPageUp, Some(MULTILINE_CONTEXT)),
        KeyBinding::new("shift-pagedown", SelectPageDown, Some(MULTILINE_CONTEXT)),
        // Only a `NumberInput` steps.
        KeyBinding::new("up", StepUp, Some(NUMBER_CONTEXT)),
        KeyBinding::new("down", StepDown, Some(NUMBER_CONTEXT)),
    ]
}

/// A field that takes one line of text.
///
/// Tab reaches it once and Tab leaves it again; Escape is left to whatever is around it, so a
/// field in a `Dialog` still lets Escape close it. Arrows, Home and End move the caret, with
/// Shift they select, and with Ctrl they go by word or to the ends of the text; Backspace and
/// Delete remove a grapheme, with Ctrl a word. Ctrl+A selects all, Ctrl+C, Ctrl+X and Ctrl+V
/// use the clipboard, Ctrl+Z undoes and Ctrl+Shift+Z or Ctrl+Y redoes, and Enter calls
/// `on_submit`. A click puts the caret, a double-click selects a word, a triple-click all the
/// text, a drag selects on past the edges of the field, and a middle-click pastes the primary
/// selection. Reaching it with Tab selects all its text. A `clearable` field's clear button
/// takes the pointer only; it is not a tab stop, since Ctrl+A and Backspace do its job.
///
/// Uncontrolled by default, starting from `default_value`; pass `value` to control it and
/// follow `on_change`. `masked` draws a dot for each character and keeps the text from the
/// clipboard, the primary selection and `Describe`. The box is as high as its density's
/// controls (28, 34 or 40), and it and everything around it stay where they are however much
/// is typed and whatever state it is in: a long text scrolls inside it.
#[derive(IntoElement)]
pub struct Input {
    id: ElementId,
    name: SharedString,
    value: Option<SharedString>,
    default_value: SharedString,
    placeholder: Option<SharedString>,
    size: Size,
    density: Option<Density>,
    disabled: bool,
    read_only: bool,
    masked: bool,
    invalid: bool,
    required: bool,
    leading: Option<SharedString>,
    trailing: Option<SharedString>,
    clearable: bool,
    /// How many rows high the field is: `None` for the one line of an `Input`.
    pub(crate) rows: Option<usize>,
    /// The most rows a field of rows grows to as its text grows; `None` keeps it `rows` high.
    pub(crate) max_rows: Option<usize>,
    /// Drawn without its own box, inside the box of an `InputGroup`, which draws it.
    pub(crate) bare: bool,
    /// The focus handle to use in place of the field's own, for a group that must know and
    /// move the field's focus. Focus handles are kept by the element they are made in.
    pub(crate) focus: Option<FocusHandle>,
    /// What a search, password or number field adds.
    pub(crate) extras: Extras,
    /// What a `FormField` says about the field, for `Describe`.
    pub(crate) description: Option<SharedString>,
    on_change: Option<ValueHandler>,
    on_submit: Option<ValueHandler>,
}

/// What a search, password or number field adds to an `Input`, crate-private, so each is an
/// `Input` with the same text engine, box and states.
#[derive(Default)]
pub(crate) struct Extras {
    /// Escape clears the text while there is any (a search field); an empty field leaves it to
    /// what is around it.
    pub escape_clears: bool,
    /// A chord shown at the end while the field is empty and not focused.
    pub shortcut: Option<SharedString>,
    /// A spinner at the end while slow work runs.
    pub loading: bool,
    /// A button at the end that shows and hides a masked field's text.
    pub reveal: bool,
    /// The rules of a number field: Up and Down and the steppers step, Enter and leaving the
    /// field commit.
    pub number: Option<Rules>,
    /// A number field's value given from outside, making it controlled. The text follows it
    /// only when it changes, so a half-typed number is never overwritten.
    pub number_value: Option<Option<f64>>,
}

impl Input {
    /// A field with no visible label of its own: `name` is what it is called to assistive
    /// technology.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: None,
            default_value: SharedString::default(),
            placeholder: None,
            size: Size::default(),
            density: None,
            disabled: false,
            read_only: false,
            masked: false,
            invalid: false,
            required: false,
            leading: None,
            trailing: None,
            clearable: false,
            rows: None,
            max_rows: None,
            bare: false,
            focus: None,
            extras: Extras::default(),
            description: None,
            on_change: None,
            on_submit: None,
        }
    }

    /// The text, making the field controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = value.into();
        self
    }

    /// Shown, muted, while the field is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// A step from the density the field would otherwise take, as for a `Button`: `Sm` one
    /// tighter, `Lg` one roomier. `density` wins over it.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The field's own density, whatever the application's and whatever its `size`.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Selectable and copyable, not editable.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// For a password: dots instead of text, and nothing copied or reported.
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// Marks the value as wrong: the danger edge, which it keeps under the pointer and in
    /// focus, the danger ring, and a `!` at the end, so it never shows by colour alone. Say
    /// what is wrong in a `FormField`'s error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that the field must be filled in. A `FormField` draws the mark beside its label.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// An icon before the text, at the density's icon size and gap, in the field's icon
    /// colour. ODK has no icon set yet, so an icon is a character, such as `"⌕"`.
    pub fn leading_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.leading = Some(icon.into());
        self
    }

    /// An icon after the text, such as a unit's symbol.
    pub fn trailing_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.trailing = Some(icon.into());
        self
    }

    /// Shows a clear button at the end while the field holds text it can change. It empties
    /// the field as one edit, which Ctrl+Z undoes, and leaves the caret in it.
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    /// Called with the text after each edit; not while an IME composition is running, but
    /// when it ends.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Called with the text when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    /// The density the field draws at: its own, else its size's step from the application's.
    pub(crate) fn density_in(&self, c: &ComponentTokens) -> Density {
        self.density
            .unwrap_or_else(|| self.size.density(c.density(None)))
    }

    /// The states the field is drawn in, given whether it has focus.
    fn states(&self, focused: bool) -> States {
        States::NONE
            .focused(focused)
            .disabled(self.disabled)
            .read_only(self.read_only)
            .invalid(self.invalid)
    }
}

impl Describe for Input {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::TextInput, self.name.to_string())
            .disabled(self.disabled)
            .multiline(self.rows.is_some())
            .read_only(self.read_only)
            .invalid(self.invalid)
            .required(self.required)
            .busy(self.extras.loading);
        a.description = self.description.as_ref().map(|d| d.to_string());
        if self.masked {
            a
        } else {
            a.value(
                self.value
                    .as_ref()
                    .unwrap_or(&self.default_value)
                    .to_string(),
            )
        }
    }
}

// Text from outside

/// Text from outside, such as a paste or the IME, for a field of one line.
fn one_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\n', '\r', '\t']) {
        Cow::Owned(text.replace(['\n', '\r', '\t'], " "))
    } else {
        Cow::Borrowed(text)
    }
}

/// Text from outside for a field of several lines: line breaks are `\n`, tabs stay.
pub(crate) fn many_lines(text: &str) -> Cow<'_, str> {
    if text.contains('\r') {
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(text)
    }
}

/// How many bytes of `text` the first `units` UTF-16 units of it take.
fn bytes_of_utf16(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (at, c) in text.char_indices() {
        if seen >= units {
            return at;
        }
        seen += c.len_utf16();
    }
    text.len()
}

// State

/// What the last paint laid out, for turning pointer positions and IME ranges into offsets.
struct Layout {
    text: TextLayout,
    bounds: Bounds<Pixels>,
    scroll: Point<Pixels>,
}

/// One text input's state: its text and selection, and what it needs to answer the platform.
pub(crate) struct InputState {
    editor: Editor,
    focus: Option<FocusHandle>,
    editable: bool,
    masked: bool,
    /// Whether the field has rows (a `Textarea`) rather than one line.
    multiline: bool,
    on_change: Option<ValueHandler>,
    on_submit: Option<ValueHandler>,
    /// The text `on_change` last heard.
    reported: String,
    layout: Option<Layout>,
    /// How far the text is scrolled up and to the left, to keep the caret in view.
    scroll: Point<Pixels>,
    /// The caret and the length of the text when the wheel last scrolled the field. While
    /// neither has changed, painting leaves the view where the wheel put it instead of pulling
    /// it back to the caret.
    wheel: Option<(usize, usize)>,
    /// Where the caret was horizontally when it began moving between rows, so that a short row
    /// in between does not lose its place.
    goal_x: Option<Pixels>,
    /// Where a drag that is selecting started.
    drag: Option<usize>,
    /// Whether the focus came from the pointer, which puts the caret where it was clicked;
    /// focus from Tab selects everything.
    pointer_focus: bool,
    was_focused: bool,
    blur: Option<Subscription>,
    /// A masked field showing its text, for a password field's show button. Only the drawing
    /// changes: the text still stays out of the clipboard, the primary selection, the IME and
    /// `Describe`.
    revealed: bool,
    /// A number field's rules.
    rules: Option<Rules>,
    /// The number given from outside when the text last followed it.
    followed: Option<Option<f64>>,
    /// How many rows the text took when it was last drawn, for a field that grows.
    rows_drawn: usize,
}

impl InputState {
    fn new(text: &str) -> Self {
        Self {
            editor: Editor::new(text),
            focus: None,
            editable: true,
            masked: false,
            multiline: false,
            on_change: None,
            on_submit: None,
            reported: text.to_owned(),
            layout: None,
            scroll: point(px(0.0), px(0.0)),
            wheel: None,
            goal_x: None,
            drag: None,
            pointer_focus: false,
            was_focused: false,
            blur: None,
            revealed: false,
            rules: None,
            followed: None,
            rows_drawn: 1,
        }
    }

    /// Text from outside, such as a paste or the IME, made fit for this field.
    fn clean<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.multiline {
            many_lines(text)
        } else {
            one_line(text)
        }
    }

    /// Tells `on_change` about the text, unless it has not changed or a composition is still
    /// running.
    fn report(&mut self, window: &mut Window, cx: &mut App) {
        if self.editor.is_composing() || self.editor.text() == self.reported {
            return;
        }
        self.reported = self.editor.text().to_owned();
        if let Some(f) = self.on_change.clone() {
            f(&self.reported, window, cx);
        }
    }

    fn text_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
        self.report(window, cx);
    }

    /// The selection is the user's own: write it to the primary selection, the Wayland way,
    /// unless the field hides its text.
    fn publish_selection(&self, cx: &mut Context<Self>) {
        if !self.masked && !self.editor.selection().is_empty() {
            cx.write_to_primary(ClipboardItem::new_string(
                self.editor.selected_text().to_owned(),
            ));
        }
    }

    fn navigate(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Editor)) {
        f(&mut self.editor);
        self.goal_x = None;
        self.publish_selection(cx);
        cx.notify();
    }

    /// Moves the caret `rows` rows down (up, if negative), to the spot nearest where it was
    /// horizontally when it began moving between rows. Above the first row it goes to the
    /// start of the text, below the last to the end.
    fn navigate_rows(&mut self, cx: &mut Context<Self>, rows: isize, extend: bool) {
        let Some(layout) = &self.layout else {
            return;
        };
        let line = layout.text.line_height();
        let here = layout.text.point_of(self.editor.cursor());
        let goal = self.goal_x.unwrap_or(here.x);
        let y = here.y + line * rows as f32 + line / 2.0;
        let to = if y < px(0.0) {
            0
        } else if y >= layout.text.height() {
            self.editor.text().len()
        } else {
            layout.text.offset_at(point(goal, y))
        };
        self.editor.move_to(to, extend);
        self.goal_x = Some(goal);
        self.publish_selection(cx);
        cx.notify();
    }

    /// A page is the rows the field shows, less one so that the last stays in view.
    fn navigate_page(&mut self, cx: &mut Context<Self>, down: bool, extend: bool) {
        let Some(layout) = &self.layout else {
            return;
        };
        let shown = (layout.bounds.size.height / layout.text.line_height()).floor() as isize;
        let rows = (shown - 1).max(1);
        self.navigate_rows(cx, if down { rows } else { -rows }, extend);
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Editor)) {
        if self.editable {
            f(&mut self.editor);
            self.goal_x = None;
            self.text_changed(window, cx);
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        if !self.masked && !self.editor.selection().is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.editor.selected_text().to_owned(),
            ));
        }
    }

    fn cut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.copy(cx);
        if !self.masked {
            self.edit(window, cx, |e| {
                e.cut();
            });
        }
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let text = self.clean(&text).into_owned();
        self.edit(window, cx, |e| e.paste(&text));
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |e| {
            e.undo();
        });
    }

    fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |e| {
            e.redo();
        });
    }

    /// Enter ends a line in a field of rows, and submits a field of one, a number field once
    /// it has committed its number.
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.edit(window, cx, |e| e.replace_text(None, "\n"));
            return;
        }
        self.commit_number(window, cx);
        if let Some(f) = self.on_submit.clone() {
            let text = self.editor.text().to_owned();
            f(&text, window, cx);
        }
    }

    /// The field lost focus: a half-made composition is taken as it stands, and a number field
    /// commits its number.
    fn blurred(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.commit_composition();
        self.pointer_focus = false;
        self.was_focused = false;
        self.drag = None;
        self.goal_x = None;
        self.commit_number(window, cx);
        self.text_changed(window, cx);
    }

    /// Puts `text` in place of the whole text as one step that Ctrl+Z undoes, with the caret
    /// at its end, unless it is the text already.
    fn replace_all(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.text() == text {
            return;
        }
        self.edit(window, cx, |e| {
            e.select_all();
            e.paste(text);
        });
    }

    /// Empties the field, for its clear button and a search field's Escape.
    pub(crate) fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_all("", window, cx);
    }

    /// A number field's text, clamped to its bounds and tidied, when it is a number. Text that
    /// is no number is kept, so nothing typed is lost; the field then shows itself invalid.
    fn commit_number(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rules) = self.rules else {
            return;
        };
        if let Some(text) = rules.commit(self.editor.text()) {
            self.replace_all(&text, window, cx);
        }
    }

    /// Moves a number field `steps` steps, up when positive.
    pub(crate) fn step(&mut self, steps: f64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rules) = self.rules else {
            return;
        };
        let text = rules.step(self.editor.text(), steps);
        self.replace_all(&text, window, cx);
    }

    /// Whether a number field's text is not a number, once the user has left it or committed it:
    /// half-typed text is not judged while it is being typed.
    fn wrong_number(&self) -> bool {
        self.rules.is_some()
            && !self.was_focused
            && !self.editor.text().trim().is_empty()
            && crate::number::parse(self.editor.text()).is_none()
    }

    // The pointer

    /// The grapheme boundary of the text nearest `position` on the screen.
    fn index_at(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = &self.layout else {
            return 0;
        };
        layout
            .text
            .offset_at(position - layout.bounds.origin + layout.scroll)
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(focus) = self.focus.clone() else {
            return;
        };
        self.pointer_focus = true;
        self.goal_x = None;
        move_focus(&focus, window);
        let at = self.index_at(event.position);
        match event.click_count {
            1 => {
                self.editor.move_to(at, event.modifiers.shift);
                self.drag = Some(self.editor.selection().anchor);
            }
            2 if !self.masked => {
                self.editor.select_word_at(at);
                self.publish_selection(cx);
            }
            3 if self.multiline && !self.masked => {
                self.editor.select_line_at(at);
                self.publish_selection(cx);
            }
            _ => {
                self.editor.select_all();
                self.publish_selection(cx);
            }
        }
        cx.notify();
    }

    fn drag_to(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(anchor) = self.drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            self.publish_selection(cx);
            return;
        }
        self.editor
            .set_selection(anchor, self.index_at(event.position));
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            self.publish_selection(cx);
            cx.notify();
        }
    }

    /// Scrolls a field of rows by the wheel. Returns whether it moved, so that a field already
    /// at its end leaves the wheel to whatever scrolls around it.
    fn scroll_by(&mut self, delta: Pixels, cx: &mut Context<Self>) -> bool {
        let Some(layout) = &self.layout else {
            return false;
        };
        let furthest = (layout.text.height() - layout.bounds.size.height).max(px(0.0));
        let y = (self.scroll.y - delta).max(px(0.0)).min(furthest);
        if y == self.scroll.y {
            return false;
        }
        self.scroll.y = y;
        self.wheel = Some((self.editor.cursor(), self.editor.text().len()));
        cx.notify();
        true
    }

    /// A middle click pastes the primary selection where it landed.
    fn paste_primary(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = cx.read_from_primary().and_then(|item| item.text()) else {
            return;
        };
        let text = self.clean(&text).into_owned();
        let at = self.index_at(event.position);
        self.edit(window, cx, |e| {
            e.move_to(at, false);
            e.paste(&text);
        });
    }
}

impl EntityInputHandler for InputState {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        if self.masked {
            return None;
        }
        let range = self.editor.range_from_utf16(&range);
        let range = range.start..range.end.max(range.start);
        adjusted.replace(self.editor.range_to_utf16(&range));
        Some(self.editor.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.editor.range_to_utf16(&self.editor.selected_range()),
            reversed: self.editor.selection().reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.editor
            .marked_range()
            .map(|range| self.editor.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.commit_composition();
        self.text_changed(window, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|r| self.editor.range_from_utf16(&r));
        let text = self.clean(text).into_owned();
        self.edit(window, cx, |e| e.replace_text(range, &text));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|r| self.editor.range_from_utf16(&r));
        let text = self.clean(text).into_owned();
        let selected =
            selected.map(|s| bytes_of_utf16(&text, s.start)..bytes_of_utf16(&text, s.end));
        self.edit(window, cx, |e| e.mark_text(range, &text, selected));
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.editor.range_from_utf16(&range);
        let (start, end) = (
            layout.text.point_of(range.start),
            layout.text.point_of(range.end),
        );
        // A range that runs over several rows is given as its first row.
        let end_x = if start.y == end.y { end.x } else { start.x };
        let origin = element_bounds.origin - layout.scroll;
        Some(Bounds::from_corners(
            origin + start,
            origin + point(end_x, start.y + layout.text.line_height()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        layout.bounds.localize(&point)?;
        Some(self.editor.offset_to_utf16(self.index_at(point)))
    }
}

// The element that draws the text, the selection and the caret

/// The colours of one field's text, from its style when it renders.
#[derive(Clone, Copy)]
struct Ink {
    text: Rgba,
    selection: Rgba,
    /// Text inside the selection.
    selected: Rgba,
    caret: Rgba,
    /// The underline under IME pre-edit text: a border width.
    underline: Pixels,
}

impl Ink {
    fn of(s: &FieldStyle) -> Self {
        Self {
            text: colour(s.current.foreground),
            selection: colour(s.selection.background),
            selected: colour(s.selection.foreground),
            caret: colour(s.caret),
            underline: px(s.geometry.border_width),
        }
    }
}

/// The runs `len` bytes of drawn text are shaped in: `base`, with the `selected` range in the
/// selection's text colour and the `marked` range (IME pre-edit text) underlined. Offsets are
/// in the drawn text.
fn runs(
    len: usize,
    base: &TextRun,
    ink: &Ink,
    selected: Option<Range<usize>>,
    marked: Option<Range<usize>>,
) -> Vec<TextRun> {
    if len == 0 {
        return Vec::new();
    }
    let mut cuts = vec![0, len];
    for r in selected.iter().chain(marked.iter()) {
        cuts.extend([r.start.min(len), r.end.min(len)]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let inside = |r: &Option<Range<usize>>, at: usize| r.as_ref().is_some_and(|r| r.contains(&at));
    cuts.windows(2)
        .map(|w| {
            let color = if inside(&selected, w[0]) {
                ink.selected.into()
            } else {
                base.color
            };
            TextRun {
                len: w[1] - w[0],
                color,
                underline: inside(&marked, w[0]).then_some(UnderlineStyle {
                    color: Some(color),
                    thickness: ink.underline,
                    wavy: false,
                }),
                ..base.clone()
            }
        })
        .collect()
}

struct InputText {
    state: Entity<InputState>,
    ink: Ink,
    /// How many rows high the element is; `None` for one line.
    rows: Option<usize>,
    /// Whether the field's height follows its text, between its rows and its most rows.
    grows: bool,
}

struct Prepaint {
    text: Option<TextLayout>,
    scroll: Point<Pixels>,
    selection: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
    /// What the wheel is over, for a field of rows.
    hitbox: Option<Hitbox>,
}

impl IntoElement for InputText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InputText {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = (window.line_height() * self.rows.unwrap_or(1) as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self.state.read(cx);
        let editor = &state.editor;
        let (text, masked, multiline) = (editor.text(), state.masked, state.multiline);
        let focused = state.focus.as_ref().is_some_and(|f| f.is_focused(window));

        let shown = Shown::new(text, masked && !state.revealed, multiline);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let run = TextRun {
            len: shown.text.len(),
            font: style.font(),
            color: self.ink.text.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        // Selected text takes the selection's own text colour, and pre-edit text from an IME is
        // underlined.
        let in_shown = |r: Range<usize>| shown.display(r.start)..shown.display(r.end);
        let selected =
            (focused && !editor.selection().is_empty()).then(|| in_shown(editor.selected_range()));
        let marked = editor.marked_range().map(in_shown);
        let runs = runs(shown.text.len(), &run, &self.ink, selected, marked);
        // Rows wrap short of the caret's width, so that a caret after the last character of
        // a full row is still inside the box.
        let caret_width = self.ink.underline * CARET_BORDERS;
        let wrap_width =
            multiline.then(|| (bounds.size.width - caret_width).max(px(MIN_WRAP_WIDTH)));
        let lines = window
            .text_system()
            .shape_text(
                shown.text.clone().into(),
                font_size,
                &runs,
                wrap_width,
                None,
            )
            .map(|lines| lines.into_vec())
            .unwrap_or_default();
        let layout = TextLayout::new(shown, lines, line_height);

        // Scroll just far enough to keep the caret in view. A one-line field without focus
        // shows the start of its text; a field of rows stays where it was.
        let cursor = layout.point_of(editor.cursor());
        let mut scroll = state.scroll;
        if multiline {
            scroll.x = px(0.0);
            let held = state.wheel == Some((editor.cursor(), text.len()));
            if focused && !held {
                if cursor.y < scroll.y {
                    scroll.y = cursor.y;
                } else if cursor.y + line_height > scroll.y + bounds.size.height {
                    scroll.y = cursor.y + line_height - bounds.size.height;
                }
            }
            let furthest = (layout.height() - bounds.size.height).max(px(0.0));
            scroll.y = scroll.y.max(px(0.0)).min(furthest);
        } else {
            scroll.y = px(0.0);
            if !focused {
                scroll.x = px(0.0);
            } else {
                if cursor.x < scroll.x {
                    scroll.x = cursor.x;
                } else if cursor.x + caret_width > scroll.x + bounds.size.width {
                    scroll.x = cursor.x + caret_width - bounds.size.width;
                }
                let furthest = (layout.width() + caret_width - bounds.size.width).max(px(0.0));
                scroll.x = scroll.x.max(px(0.0)).min(furthest);
            }
        }

        let origin = bounds.origin - scroll;
        let selection = editor.selected_range();
        let (selection_quads, caret_quad) = if !focused {
            (Vec::new(), None)
        } else if selection.is_empty() {
            let caret = state.editable.then(|| {
                fill(
                    Bounds::new(origin + cursor, size(caret_width, line_height)),
                    self.ink.caret,
                )
            });
            (Vec::new(), caret)
        } else {
            let quads = layout
                .rects_of(selection, font_size * BREAK_EMS)
                .into_iter()
                .map(|r| fill(Bounds::new(origin + r.origin, r.size), self.ink.selection))
                .collect();
            (quads, None)
        };
        let hitbox = multiline.then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal));
        Prepaint {
            text: Some(layout),
            scroll,
            selection: selection_quads,
            caret: caret_quad,
            hitbox,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus, editable, dragging) = {
            let state = self.state.read(cx);
            (state.focus.clone(), state.editable, state.drag.is_some())
        };
        if let Some(focus) = &focus
            && editable
        {
            window.handle_input(
                focus,
                ElementInputHandler::new(bounds, self.state.clone()),
                cx,
            );
        }
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let Some(text) = prepaint.text.take() else {
            return;
        };
        let scroll = prepaint.scroll;
        text.paint(
            bounds.origin - scroll,
            scroll.y..scroll.y + bounds.size.height,
            window,
            cx,
        );
        if let Some(caret) = prepaint.caret.take() {
            window.paint_quad(caret);
        }

        // A drag keeps selecting when the pointer leaves the field, so it listens to the whole
        // window while it lasts (ADR 0038). The listeners see both phases of an event.
        if dragging {
            let state = self.state.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    state.update(cx, |s, cx| s.drag_to(event, cx));
                }
            });
            let state = self.state.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    state.update(cx, |s, cx| s.end_drag(cx));
                }
            });
        }
        if let Some(hitbox) = prepaint.hitbox.take() {
            let state = self.state.clone();
            let line_height = window.line_height();
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    let delta = event.delta.pixel_delta(line_height).y;
                    if state.update(cx, |s, cx| s.scroll_by(delta, cx)) {
                        cx.stop_propagation();
                    }
                }
            });
        }

        let rows = ((text.height() / text.line_height()).round() as usize).max(1);
        let grew = self.state.update(cx, |state, _| {
            state.scroll = scroll;
            state.layout = Some(Layout {
                text,
                bounds,
                scroll,
            });
            std::mem::replace(&mut state.rows_drawn, rows) != rows
        });
        // A field that grows with its text takes its new height on the next frame.
        if grew && self.grows {
            crate::interaction::request_frame(window);
        }
    }
}

// The component

/// Runs `f` on the state when action `A` arrives. A field binds some forty actions on every
/// frame, so each takes a clone of one shared handle: cloning the entity itself is an atomic
/// count up and down per action per field per frame, a seventh of a page of fields (`benches/`).
fn bind<A: Action>(
    field: Stateful<Div>,
    state: &Rc<Entity<InputState>>,
    f: impl Fn(&mut InputState, &mut Window, &mut Context<InputState>) + 'static,
) -> Stateful<Div> {
    let state = state.clone();
    field.on_action(move |_: &A, window, cx| state.update(cx, |s, cx| f(s, window, cx)))
}

#[cfg(test)]
thread_local! {
    /// The state of each field rendered on this thread, by id, for tests to read.
    static STATES: std::cell::RefCell<std::collections::HashMap<String, Entity<InputState>>> =
        Default::default();
}

/// What a test sees of the field with this id: its text, its selection and its pre-edit text.
#[cfg(test)]
pub(crate) fn probe(
    cx: &mut gpui::VisualTestContext,
    id: &str,
) -> (String, Range<usize>, Option<Range<usize>>) {
    let state = STATES.with(|s| s.borrow()[id].clone());
    state.read_with(cx, |s, _| {
        (
            s.editor.text().to_owned(),
            s.editor.selected_range(),
            s.editor.marked_range(),
        )
    })
}

/// Where the text offset `offset` of the field with this id is on screen, in the middle of
/// its row, for tests that click or drag there.
#[cfg(test)]
pub(crate) fn point_of(cx: &mut gpui::VisualTestContext, id: &str, offset: usize) -> Point<Pixels> {
    entity_of(id).read_with(cx, |s, _| {
        let layout = s.layout.as_ref().expect("the field has been drawn");
        let at = layout.text.point_of(offset);
        layout.bounds.origin - layout.scroll + point(at.x, at.y + layout.text.line_height() / 2.0)
    })
}

/// Runs `f` on what the last paint of the field with this id laid out.
#[cfg(test)]
pub(crate) fn layout_of<R>(
    cx: &mut gpui::VisualTestContext,
    id: &str,
    f: impl FnOnce(&TextLayout, Bounds<Pixels>, Point<Pixels>) -> R,
) -> R {
    entity_of(id).read_with(cx, |s, _| {
        let layout = s.layout.as_ref().expect("the field has been drawn");
        f(&layout.text, layout.bounds, layout.scroll)
    })
}

/// The entity behind the field with this id, for tests that drive its input handler.
#[cfg(test)]
pub(crate) fn entity_of(id: &str) -> Entity<InputState> {
    STATES.with(|s| s.borrow()[id].clone())
}

/// The part names of a field's end, for tests.
const CLEAR: &str = "clear";
const REVEAL: &str = "reveal";
const DECREMENT: &str = "decrement";
const INCREMENT: &str = "increment";

/// The glyphs of a field's own buttons and marks. ODK has no icon set yet, so they are
/// characters, like a button's icons.
const CLEAR_GLYPH: &str = "×";
/// A password field's show button shows an open eye while the text is hidden, a closed one
/// while it shows.
const SHOW_GLYPH: &str = "◉";
const HIDE_GLYPH: &str = "○";
const DECREMENT_GLYPH: &str = "−";
const INCREMENT_GLYPH: &str = "+";

/// One glyph of `size`, centred in a square of that size, for a field's icons and marks.
pub(crate) fn field_glyph(glyph: SharedString, size: f32, ink: Rgba) -> Div {
    let size = px(size);
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(size)
        .font_family(mono())
        .text_size(size)
        .line_height(size)
        .text_color(ink)
        .child(glyph)
}

/// Puts a field's box on `el`: its height, padding, corners, border and type from `s`, its
/// colours under the pointer, and its focus ring while focused. `end_inset` is set when the
/// box ends with a button, which sits the thumb gap from the border rather than the padding.
/// Call it after everything else that sets `el`'s text style.
pub(crate) fn field_box(
    el: Stateful<Div>,
    s: &FieldStyle,
    end_inset: Option<f32>,
) -> Stateful<Div> {
    let g = s.geometry;
    let mut el = el
        .relative()
        .flex()
        .items_center()
        .min_w_0()
        .control_geometry(&g);
    if let Some(inset) = end_inset {
        el = el.pr(px(inset));
    }
    el = pointer_states(el, &s.colours, s.responds, &Pointer::Own);
    if s.focus_visible {
        el = el.child(focus_ring(&s.focus, g.radius, g.border_width));
    }
    el
}

/// Wraps the buttons at a field's end so a press on one stays the button's: it does not reach
/// the field, which would move its caret. The buttons themselves take no focus from the
/// pointer (`Button::keep_focus`), so the field keeps its focus, selection and IME composition.
fn end_buttons(id: &ElementId) -> Stateful<Div> {
    div()
        .id(child_id(id, "end"))
        .flex()
        .flex_shrink_0()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let usable = !self.disabled;
        let editable = usable && !self.read_only;
        let focus = match &self.focus {
            Some(handle) => handle.clone().tab_stop(usable),
            None => focus_handle(child_id(&self.id, "focus"), usable, window, cx),
        };
        let multiline = self.rows.is_some();
        // A field of rows keeps its line breaks as `\n`, whatever the application gave it.
        let given = |text: &SharedString| -> String {
            if multiline {
                many_lines(text).into_owned()
            } else {
                text.to_string()
            }
        };
        let rules = self.extras.number;
        let start = match (rules, self.extras.number_value) {
            (Some(r), Some(Some(v))) => r.text_of(v),
            _ => given(self.value.as_ref().unwrap_or(&self.default_value)),
        };
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| {
            InputState::new(&start)
        });
        #[cfg(test)]
        STATES.with(|s| {
            s.borrow_mut()
                .insert(crate::focus::id_path(&self.id), state.clone());
        });
        let focused = usable && focus.is_focused(window);

        state.update(cx, |s, _| {
            s.focus = Some(focus.clone());
            s.editable = editable;
            s.masked = self.masked;
            s.multiline = multiline;
            s.rules = rules;
            s.editor.set_masked(self.masked);
            if !self.masked {
                s.revealed = false;
            }
            s.on_change = self.on_change.clone();
            s.on_submit = self.on_submit.clone();
            // A controlled value that differs from the text replaces it, except while the IME
            // is composing: the application has not heard of the pre-edit text.
            if let Some(value) = self.value.as_ref().map(given)
                && !s.editor.is_composing()
                && value != s.editor.text()
            {
                s.editor.set_text(&value);
                s.reported = value;
            }
            // A number from outside replaces the text only when it changes, so `-` or `1.` on
            // the way to a number is never overwritten by the number before it.
            if let (Some(r), Some(value)) = (rules, self.extras.number_value)
                && !s.editor.is_composing()
                && s.followed != Some(value)
            {
                s.followed = Some(value);
                let text = value.map(|v| r.text_of(v)).unwrap_or_default();
                if crate::number::parse(s.editor.text()) != value {
                    s.editor.set_text(&text);
                    s.reported = text;
                }
            }
            if focused && !s.was_focused {
                if !s.pointer_focus {
                    s.editor.select_all();
                }
                s.pointer_focus = false;
            }
            s.was_focused = focused;
        });
        if usable && state.read(cx).blur.is_none() {
            let weak = state.downgrade();
            let subscription = window.on_focus_out(&focus, cx, move |_, window, cx| {
                weak.update(cx, |s, cx| s.blurred(window, cx)).ok();
            });
            state.update(cx, |s, _| s.blur = Some(subscription));
        }

        let (empty, wrong_number, revealed, drawn_rows) = {
            let s = state.read(cx);
            (
                s.editor.text().is_empty(),
                s.wrong_number(),
                s.revealed,
                s.rows_drawn,
            )
        };
        let density = self.density_in(&c);
        let states = self.states(focused).invalid(self.invalid || wrong_number);
        // However much is typed, a field of rows stays `rows` high, or grows up to `max_rows`.
        let rows = self.rows.map(|rows| match self.max_rows {
            Some(most) => drawn_rows.clamp(rows, most.max(rows)),
            None => rows,
        });
        let style = match rows {
            Some(rows) => c.textarea(density, rows, states),
            None => c.field(density, states),
        };
        let g = style.geometry;
        #[cfg(test)]
        drawn::record(&self.id, style);

        let line = px(g.line_height);
        let height = line * rows.unwrap_or(1) as f32;
        let mut inner = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h(height)
            .overflow_hidden()
            .child(InputText {
                state: state.clone(),
                ink: Ink::of(&style),
                rows,
                grows: self.max_rows.is_some(),
            });
        if empty && let Some(placeholder) = self.placeholder.clone() {
            let hint = div()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(height)
                .overflow_hidden();
            let hint = hint
                .debug_selector(|| part_name(&self.id, "placeholder"))
                .text_color(colour(style.placeholder));
            // A field of rows wraps its placeholder inside its box; one line truncates it.
            inner = inner.child(if multiline {
                hint.child(
                    div()
                        .debug_selector(|| part_name(&self.id, "placeholder-text"))
                        .child(placeholder),
                )
            } else {
                single_line(hint).child(placeholder)
            });
        }

        let icon = |glyph: SharedString, part: &str, ink| {
            let name = part_name(&self.id, part);
            field_glyph(glyph, g.icon_size, colour(ink)).debug_selector(|| name)
        };
        let key_context = match (multiline, rules.is_some()) {
            (true, _) => MULTILINE_CONTEXTS,
            (false, true) => NUMBER_CONTEXTS,
            (false, false) => KEY_CONTEXT,
        };
        let mut field = div()
            .id(child_id(&self.id, "box"))
            .debug_selector(|| part_name(&self.id, "box"))
            .key_context(key_context)
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .text_role(g.type_role)
            .text_color(colour(style.current.foreground));
        if !self.bare {
            field = field.gap(px(g.gap));
            if let Some(glyph) = self.leading.clone() {
                field = field.child(icon(glyph, "leading-icon", style.icon));
            }
        }
        field = field.child(inner);

        // The end of a one-line field: its icon, the mark of a wrong value, a spinner, a
        // shortcut, and its own buttons, in that order.
        let mut ends_in_button = false;
        if !self.bare && !multiline {
            if let Some(glyph) = self.trailing.clone() {
                field = field.child(icon(glyph, "trailing-icon", style.icon));
            }
            if let Some(mark) = style.invalid_mark {
                field = field.child(icon(FIELD_INVALID_MARK.into(), "invalid-mark", mark));
            }
            if self.extras.loading {
                let name = part_name(&self.id, "spinner");
                field = field.child(
                    div()
                        .debug_selector(|| name)
                        .flex_shrink_0()
                        .size(px(g.icon_size))
                        .text_color(colour(style.icon))
                        .child(Spinner::new(child_id(&self.id, "spinner")).glyph_size(g.icon_size)),
                );
            }
            if let Some(chord) = self.extras.shortcut.clone()
                && empty
                && !focused
            {
                let name = part_name(&self.id, "shortcut");
                field = field.child(
                    div()
                        .debug_selector(|| name)
                        .flex_shrink_0()
                        .child(Kbd::new(&chord)),
                );
            }
            let button = |part: &'static str, glyph: &'static str, name: &'static str| {
                Button::icon(child_id(&self.id, part), glyph, name)
                    .variant(ButtonVariant::Ghost)
                    .density(density)
                    .in_field()
                    .keep_focus()
            };
            let mut ends: Vec<&'static str> = Vec::new();
            let mut end = end_buttons(&self.id).gap(px(c.foundation().geometry.thumb_gap));
            let mut buttons = 0;
            if self.clearable && editable && !empty {
                let state = state.clone();
                let focus = focus.clone();
                ends.push(CLEAR);
                end = end.child(button(CLEAR, CLEAR_GLYPH, "Clear").skip_tab().on_click(
                    move |_, window, cx| {
                        state.update(cx, |s, cx| s.clear(window, cx));
                        move_focus(&focus, window);
                    },
                ));
                buttons += 1;
            }
            if self.extras.reveal && self.masked {
                let (glyph, name) = if revealed {
                    (HIDE_GLYPH, "Hide password")
                } else {
                    (SHOW_GLYPH, "Show password")
                };
                let state = state.clone();
                ends.push(REVEAL);
                end = end.child(button(REVEAL, glyph, name).disabled(!usable).on_click(
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.revealed = !s.revealed;
                            cx.notify();
                        });
                    },
                ));
                buttons += 1;
            }
            if let Some(r) = rules {
                let text = state.read(cx).editor.text().to_owned();
                for (part, glyph, name, steps) in [
                    (DECREMENT, DECREMENT_GLYPH, "Decrease", -1.0),
                    (INCREMENT, INCREMENT_GLYPH, "Increase", 1.0),
                ] {
                    let state = state.clone();
                    let can = editable && r.can_step(&text, steps > 0.0);
                    ends.push(part);
                    end = end.child(
                        button(part, glyph, name)
                            .disabled(!can)
                            .skip_tab()
                            .on_click(move |_, window, cx| {
                                state.update(cx, |s, cx| s.step(steps, window, cx));
                            }),
                    );
                    buttons += 1;
                }
            }
            if buttons > 0 {
                field = field.child(end);
                ends_in_button = true;
            }
            #[cfg(test)]
            {
                if style.invalid_mark.is_some() {
                    ends.push("invalid-mark");
                }
                if self.extras.shortcut.is_some() && empty && !focused {
                    ends.push("shortcut");
                }
                drawn::record_ends(&self.id, ends);
            }
        }
        if !self.bare {
            let inset = ends_in_button.then(|| c.foundation().geometry.thumb_gap);
            field = field_box(field, &style, inset);
        }

        if usable {
            field = field.track_focus(&focus).cursor(CursorStyle::IBeam);
            let s = state.clone();
            field = field.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                s.update(cx, |s, cx| s.mouse_down(event, window, cx));
            });
            let s = state.clone();
            field = field.on_mouse_down(MouseButton::Middle, move |event, window, cx| {
                s.update(cx, |s, cx| s.paste_primary(event, window, cx));
            });
            if self.extras.escape_clears && editable {
                let s = state.clone();
                field = field.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                        let cleared = s.update(cx, |s, cx| {
                            let had = !s.editor.text().is_empty();
                            s.clear(window, cx);
                            had
                        });
                        // An empty field leaves Escape to what is around it, such as a dialog.
                        if cleared {
                            cx.stop_propagation();
                        }
                    }
                });
            }

            let shared = Rc::new(state.clone());
            let st = &shared;
            // Enter always has a listener: left alone, it would arrive as a typed line break.
            field = bind::<Submit>(field, st, |s, w, cx| s.enter(w, cx));
            field = bind::<Left>(field, st, |s, _, cx| s.navigate(cx, |e| e.left(false)));
            field = bind::<Right>(field, st, |s, _, cx| s.navigate(cx, |e| e.right(false)));
            field = bind::<SelectLeft>(field, st, |s, _, cx| s.navigate(cx, |e| e.left(true)));
            field = bind::<SelectRight>(field, st, |s, _, cx| s.navigate(cx, |e| e.right(true)));
            field = bind::<WordLeft>(field, st, |s, _, cx| s.navigate(cx, |e| e.word_left(false)));
            field = bind::<WordRight>(field, st, |s, _, cx| {
                s.navigate(cx, |e| e.word_right(false))
            });
            field =
                bind::<SelectWordLeft>(field, st, |s, _, cx| s.navigate(cx, |e| e.word_left(true)));
            field = bind::<SelectWordRight>(field, st, |s, _, cx| {
                s.navigate(cx, |e| e.word_right(true))
            });
            field = bind::<LineStart>(field, st, |s, _, cx| {
                s.navigate(cx, |e| e.line_start(false))
            });
            field = bind::<LineEnd>(field, st, |s, _, cx| s.navigate(cx, |e| e.line_end(false)));
            field = bind::<SelectLineStart>(field, st, |s, _, cx| {
                s.navigate(cx, |e| e.line_start(true))
            });
            field =
                bind::<SelectLineEnd>(field, st, |s, _, cx| s.navigate(cx, |e| e.line_end(true)));
            field = bind::<TextStart>(field, st, |s, _, cx| {
                s.navigate(cx, |e| e.text_start(false))
            });
            field = bind::<TextEnd>(field, st, |s, _, cx| s.navigate(cx, |e| e.text_end(false)));
            field = bind::<SelectTextStart>(field, st, |s, _, cx| {
                s.navigate(cx, |e| e.text_start(true))
            });
            field =
                bind::<SelectTextEnd>(field, st, |s, _, cx| s.navigate(cx, |e| e.text_end(true)));
            if multiline {
                field = bind::<RowUp>(field, st, |s, _, cx| s.navigate_rows(cx, -1, false));
                field = bind::<RowDown>(field, st, |s, _, cx| s.navigate_rows(cx, 1, false));
                field = bind::<SelectRowUp>(field, st, |s, _, cx| s.navigate_rows(cx, -1, true));
                field = bind::<SelectRowDown>(field, st, |s, _, cx| s.navigate_rows(cx, 1, true));
                field = bind::<PageUp>(field, st, |s, _, cx| s.navigate_page(cx, false, false));
                field = bind::<PageDown>(field, st, |s, _, cx| s.navigate_page(cx, true, false));
                field =
                    bind::<SelectPageUp>(field, st, |s, _, cx| s.navigate_page(cx, false, true));
                field =
                    bind::<SelectPageDown>(field, st, |s, _, cx| s.navigate_page(cx, true, true));
            }
            if rules.is_some() {
                field = bind::<StepUp>(field, st, |s, w, cx| s.step(1.0, w, cx));
                field = bind::<StepDown>(field, st, |s, w, cx| s.step(-1.0, w, cx));
            }
            field = bind::<SelectAll>(field, st, |s, _, cx| s.navigate(cx, Editor::select_all));
            field = bind::<Backspace>(field, st, |s, w, cx| s.edit(w, cx, Editor::backspace));
            field = bind::<Delete>(field, st, |s, w, cx| s.edit(w, cx, Editor::delete));
            field =
                bind::<BackspaceWord>(field, st, |s, w, cx| s.edit(w, cx, Editor::backspace_word));
            field = bind::<DeleteWord>(field, st, |s, w, cx| s.edit(w, cx, Editor::delete_word));
            field = bind::<Copy>(field, st, |s, _, cx| s.copy(cx));
            field = bind::<Cut>(field, st, |s, w, cx| s.cut(w, cx));
            field = bind::<Paste>(field, st, |s, w, cx| s.paste(w, cx));
            field = bind::<Undo>(field, st, |s, w, cx| s.undo(w, cx));
            field = bind::<Redo>(field, st, |s, w, cx| s.redo(w, cx));
        }
        field
    }
}

/// What each field drew on its last frame, for tests: the style it looked up. Kept per thread,
/// like the test theme.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use gpui::ElementId;
    use odk_theme::component::FieldStyle;

    use crate::focus::id_path;

    thread_local! {
        static DRAWN: RefCell<HashMap<String, FieldStyle>> = RefCell::new(HashMap::new());
        static ENDS: RefCell<HashMap<String, Vec<&'static str>>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(id: &ElementId, style: FieldStyle) {
        DRAWN.with(|d| d.borrow_mut().insert(id_path(id), style));
    }

    pub(crate) fn record_ends(id: &ElementId, ends: Vec<&'static str>) {
        ENDS.with(|d| d.borrow_mut().insert(id_path(id), ends));
    }

    /// What the one-line field with id `id` drew at its end on its last frame, by part name:
    /// its buttons, its invalid mark and its shortcut. GPUI keeps the bounds of parts drawn on
    /// earlier frames, so a part that went away still has bounds; this says what is drawn now.
    pub(crate) fn ends(id: &str) -> Vec<&'static str> {
        ENDS.with(|d| d.borrow().get(id).cloned().unwrap_or_default())
    }

    /// What the field with id `id` drew on its last frame.
    pub(crate) fn last(id: &str) -> FieldStyle {
        DRAWN.with(|d| {
            *d.borrow()
                .get(id)
                .unwrap_or_else(|| panic!("no field {id:?} has been drawn"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_from_outside_becomes_one_line() {
        assert_eq!(one_line("a\nb\tc"), "a b c");
        assert!(matches!(one_line("abc"), Cow::Borrowed(_)));
    }

    #[test]
    fn text_from_outside_keeps_its_lines_in_a_field_of_rows() {
        assert_eq!(many_lines("a\r\nb\rc\td"), "a\nb\nc\td");
        assert!(matches!(many_lines("a\nb"), Cow::Borrowed(_)));
    }

    /// Selected text takes the selection's own text colour, and pre-edit text is underlined,
    /// each over exactly its range, however the two overlap.
    #[test]
    fn runs_colour_the_selection_and_underline_the_pre_edit() {
        let red = Rgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let blue = Rgba {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };
        let ink = Ink {
            text: red,
            selection: blue,
            selected: blue,
            caret: red,
            underline: px(1.0),
        };
        let base = TextRun {
            len: 10,
            font: gpui::font("Sans"),
            color: red.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        assert!(runs(0, &base, &ink, None, None).is_empty());
        assert_eq!(runs(10, &base, &ink, None, None).len(), 1);
        let r = runs(10, &base, &ink, Some(2..6), Some(4..8));
        let lens: Vec<usize> = r.iter().map(|r| r.len).collect();
        assert_eq!(lens, [2, 2, 2, 2, 2]);
        let selected: Vec<bool> = r.iter().map(|r| r.color == blue.into()).collect();
        assert_eq!(selected, [false, true, true, false, false]);
        let underlined: Vec<bool> = r.iter().map(|r| r.underline.is_some()).collect();
        assert_eq!(underlined, [false, false, true, true, false]);
    }

    #[test]
    fn ime_offsets_inside_pre_edit_text_become_bytes() {
        assert_eq!(bytes_of_utf16("a😀b", 0), 0);
        assert_eq!(bytes_of_utf16("a😀b", 3), 5);
        assert_eq!(bytes_of_utf16("a😀b", 9), 6);
    }
}
