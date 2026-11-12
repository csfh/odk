//! A one-line field on the Rune host. The caret lives in the store.
//!
//! A non-empty selection the person makes writes the primary selection. A selection set through
//! [`crate::Store::set_input_selection`] does not. Middle-click pastes that primary selection.

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::{Glyph, Icon};
use crate::ink::ink;
use crate::style::{self, text_style};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use rune::{El, Key, KeyEvent, PointerButton, Tx, auto_grow, col, edit_access, row, text_input};

/// The most rows a field of rows grows to when it is given no limit.
const MOST_ROWS: usize = 10_000;

/// The box's width in points at the base text size, unless the application sets one (ODK's own).
const DEFAULT_WIDTH: f32 = 240.0;

/// Enter submits on release, once per hold.
pub struct Input<C, S> {
    id: String,
    label: String,
    width: f32,
    invalid: bool,
    required: bool,
    disabled: bool,
    description: Option<String>,
    full_width: bool,
    placeholder: Option<String>,
    leading: Option<Glyph>,
    before: Vec<El>,
    trailing: Vec<El>,
    default_value: Option<String>,
    /// A masked field: `Some(revealed)`. It shows dots, or its text while revealed, and it
    /// never writes the clipboard or the primary selection.
    secure: Option<bool>,
    /// A field of rows: how many it starts with and the most it grows to. Enter ends a line.
    rows: Option<(usize, Option<usize>)>,
    on_change: C,
    on_submit: S,
}

impl<C, S> Input<C, S> {
    /// `label` is the field's accessible name: what a screen reader says on entering it.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        on_change: C,
        on_submit: S,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            width: DEFAULT_WIDTH,
            invalid: false,
            required: false,
            disabled: false,
            description: None,
            full_width: false,
            placeholder: None,
            leading: None,
            before: Vec::new(),
            trailing: Vec::new(),
            default_value: None,
            secure: None,
            rows: None,
            on_change,
            on_submit,
        }
    }

    /// The field's key, which a form field's label focuses.
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// The box's width in points at the base text size; the user's text size scales it.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Marks the value as wrong: the danger edge and a mark at the end, so it never shows by
    /// colour alone. Say what is wrong in a form field's error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that the field must be filled in.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Takes the field out of the Tab order and draws it disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The text a form field gives its control, which the control reports as its description.
    pub(crate) fn described(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }
}

/// Rune reports the editor's text itself, so the value is not part of this description.
impl<C, S> Describe for Input<C, S> {
    fn accessible(&self) -> Accessible {
        let accessible = Accessible::new(Role::TextInput, self.label.clone())
            .invalid(self.invalid)
            .required(self.required)
            .disabled(self.disabled);
        match &self.description {
            Some(description) => accessible.description(description.clone()),
            None => accessible,
        }
    }
}

/// A key the owner of a field takes before the field does: `true` when it did. The last
/// argument is whether the key is going up.
pub(crate) type KeyHook<T> = Rc<dyn Fn(&mut Hosted<T>, &KeyEvent, &mut Tx, bool) -> bool>;

impl<C, S> Input<C, S> {
    /// Text shown while the field is empty, in the placeholder colour. It is not the field's name.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// The text the field starts with, until the person edits it.
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// An icon before the text, such as a search field's.
    pub(crate) fn leading_icon(mut self, glyph: impl Into<Glyph>) -> Self {
        self.leading = Some(glyph.into());
        self
    }

    /// Something before the text, inside the box: an input group's prefix.
    pub(crate) fn before(mut self, element: El) -> Self {
        self.before.push(element);
        self
    }

    /// Something after the text, inside the box: a field action.
    pub(crate) fn trailing(mut self, element: El) -> Self {
        self.trailing.push(element);
        self
    }

    /// Masks the text, unless `revealed`, and keeps it out of the clipboard and the primary
    /// selection.
    pub(crate) fn secure(mut self, revealed: bool) -> Self {
        self.secure = Some(revealed);
        self
    }

    /// A field of `rows` lines that grows to `max_rows`, and scrolls past that.
    pub(crate) fn multiline(mut self, rows: usize, max_rows: Option<usize>) -> Self {
        self.rows = Some((rows.max(1), max_rows.map(|m| m.max(rows.max(1)))));
        self
    }

    /// As wide as its column, for a field inside a panel.
    pub(crate) fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El
    where
        T: 'static,
        C: Fn(&mut T, String) + Clone + 'static,
        S: Fn(&mut T) + Clone + 'static,
    {
        self.show_with(hosted, None)
    }

    pub(crate) fn show_with<T>(self, hosted: &Hosted<T>, hook: Option<KeyHook<T>>) -> El
    where
        T: 'static,
        C: Fn(&mut T, String) + Clone + 'static,
        S: Fn(&mut T) + Clone + 'static,
    {
        let accessible = self.accessible();
        let Input {
            id,
            width,
            full_width,
            placeholder,
            leading,
            before,
            trailing,
            default_value,
            secure,
            rows,
            invalid,
            disabled,
            on_change,
            on_submit,
            ..
        } = self;
        if let Some(value) = default_value {
            hosted.ui.seed_input(&id, value);
        }
        if secure.is_none() {
            hosted.ui.note_input_selection(&id);
        }
        let edit = hosted.ui.input_edit_clone(&id);
        let paint = style::paint(hosted);
        let states = States::NONE.disabled(disabled).invalid(invalid);
        let field = match rows {
            Some((rows, _)) => paint
                .tokens
                .textarea(paint.tokens.density(None), rows, states),
            None => paint.tokens.field(paint.tokens.density(None), states),
        };
        let multiline = rows.is_some();
        let width = paint.px(width);
        let edit_id = id.clone();
        let change_id = id.clone();
        let arm_id = id.clone();
        let up_id = id.clone();
        let paste_id = id.clone();
        let on_paste = on_change.clone();
        let on_enter = on_change.clone();
        let enter_id = id.clone();
        let on_type = on_change;
        let press_hook = hook.clone();
        let release_hook = hook;
        let editor = apply(
            text_input(&edit, text_style(paint.body)).key(id.clone()),
            &accessible,
        )
        .grow(1.0)
        .color(ink(field.colours.default.foreground))
        .caret(ink(field.caret), ink(field.selection.background))
        .pointer_buttons([PointerButton::Middle]);
        let editor = if rows.is_some() {
            editor
        } else {
            editor.single_line()
        };
        let editor = match secure {
            Some(revealed) => editor.secure().revealed(revealed),
            None => editor,
        };
        let editor = match placeholder {
            Some(text) => editor.placeholder(text, ink(field.placeholder)),
            None => editor,
        };
        // Rune keeps an editor that is bound to its text focusable, disabled or not, so a
        // disabled field leaves it unbound: it shows its text and takes no focus or edit.
        let editor = if disabled {
            editor
        } else {
            editor
                .on_edit(edit_access(move |hosted: &mut Hosted<T>| {
                    hosted.ui.input_edit(&edit_id)
                }))
                .on_input_change(move |hosted: &mut Hosted<T>, _: &mut Tx| {
                    let text = hosted.ui.input_text(&change_id);
                    on_type(&mut hosted.app, text);
                })
                .on_input_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                    if let Some(hook) = &press_hook
                        && hook(hosted, ev, tx, false)
                    {
                        return true;
                    }
                    // Rune's editor submits on a plain Enter; a field of rows ends a line instead.
                    if multiline && plain_enter(ev) {
                        let text = {
                            let edit = hosted.ui.input_edit(&enter_id);
                            edit.insert("\n");
                            edit.text.clone()
                        };
                        on_enter(&mut hosted.app, text);
                        return true;
                    }
                    if multiline || !plain_enter(ev) {
                        return false;
                    }
                    // The editor would submit on Enter's press. Hold it for the release.
                    if !tx.key_repeat() {
                        hosted.ui.arm_input_submit(&arm_id);
                    }
                    true
                })
                .on_key_up(move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                    if let Some(hook) = &release_hook
                        && hook(hosted, ev, tx, true)
                    {
                        return true;
                    }
                    if multiline || !plain_enter(ev) || !hosted.ui.take_input_submit(&up_id) {
                        return false;
                    }
                    on_submit(&mut hosted.app);
                    true
                })
                .on_press_tx(move |hosted: &mut Hosted<T>, tx: &mut Tx| {
                    if tx.button() != PointerButton::Middle {
                        return;
                    }
                    let primary = hosted.ui.primary();
                    if primary.is_empty() {
                        return;
                    }
                    let text = {
                        let edit = hosted.ui.input_edit(&paste_id);
                        edit.insert(&primary);
                        edit.text.clone()
                    };
                    on_paste(&mut hosted.app, text);
                })
        };
        // A field of rows is fitted to its text: `rows` lines at least, growing to `max_rows`, then
        // scrolling inside with the caret kept in view. The editor has its handlers by now.
        let editor = match rows {
            // In a column, where the viewport's height is the main axis and grows with its content.
            Some((rows, max_rows)) => col().grow(1.0).min_w(0.0).child(auto_grow(
                editor,
                rows,
                max_rows.unwrap_or(MOST_ROWS),
            )),
            None => editor,
        };
        let boxed = row().key(format!("{id}-box"));
        let boxed = if full_width {
            boxed.w_full()
        } else {
            boxed.w(width)
        };
        let boxed = match rows {
            // The text starts at the top, and the box is as tall as the viewport fitted to it.
            Some(_) => boxed.items_start().py(field.geometry.padding_y),
            None => boxed.h(field.geometry.height).center_y(),
        };
        boxed
            .px(field.geometry.padding_x)
            .radius(field.geometry.radius)
            .fill(ink(field.colours.default.background))
            .outline(
                field.geometry.border_width,
                0.0,
                ink(field.colours.default.border),
            )
            .children(before)
            .children(leading.map(|glyph| {
                // A leading icon sits `gap` before the text.
                row()
                    .pr(field.geometry.gap)
                    .child(glyph.slot(field.geometry.icon_size, ink(field.icon)))
            }))
            .child(editor)
            .children(field.invalid_mark.map(|colour| {
                // A trailing icon sits `gap` from the text.
                row()
                    .pl(field.geometry.gap)
                    .child(Icon::Warning.mark(field.geometry.icon_size, ink(colour)))
            }))
            .children(trailing)
    }
}

fn plain_enter(ev: &KeyEvent) -> bool {
    !ev.mods.cmd
        && !ev.mods.ctrl
        && !ev.mods.alt
        && !ev.mods.logo
        && !ev.mods.shift
        && ev.key == Key::Enter
}
