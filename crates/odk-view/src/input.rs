//! A one-line field on the Rune host. The caret lives in the store.
//!
//! A non-empty selection the person makes writes the primary selection. A selection set through
//! [`crate::Store::set_input_selection`] does not. Middle-click pastes that primary selection.

use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, text_style};
use odk_theme::component::States;
use odk_theme::foundation::Density;
use rune::{El, Key, KeyEvent, PointerButton, Tx, edit_access, row, text_input};

/// Enter submits on release, once per hold.
pub struct Input<C, S> {
    id: String,
    on_change: C,
    on_submit: S,
}

impl<C, S> Input<C, S> {
    pub fn new(id: impl Into<String>, on_change: C, on_submit: S) -> Self {
        Self {
            id: id.into(),
            on_change,
            on_submit,
        }
    }
}

impl<C, S> Input<C, S> {
    pub fn show<T>(self, hosted: &Hosted<T>) -> El
    where
        T: 'static,
        C: Fn(&mut T, String) + Clone + 'static,
        S: Fn(&mut T) + Clone + 'static,
    {
        let Input {
            id,
            on_change,
            on_submit,
        } = self;
        hosted.ui.note_input_selection(&id);
        let edit = hosted.ui.input_edit_clone(&id);
        let paint = style::paint(hosted.theme());
        let field = paint.tokens.field(Density::Default, States::NONE);
        let width = hosted.theme().text.px(240.0);
        let edit_id = id.clone();
        let change_id = id.clone();
        let arm_id = id.clone();
        let up_id = id.clone();
        let paste_id = id.clone();
        let on_paste = on_change.clone();
        let on_type = on_change;
        row()
            .key(format!("{id}-box"))
            .w(width)
            .h(field.geometry.height)
            .px(field.geometry.padding_x)
            .center_y()
            .radius(field.geometry.radius)
            .fill(ink(field.colours.default.background))
            .outline(
                field.geometry.border_width,
                0.0,
                ink(field.colours.default.border),
            )
            .child(
                text_input(&edit, text_style(paint.body))
                    .key(id.clone())
                    .label(id)
                    .single_line()
                    .grow(1.0)
                    .color(ink(field.colours.default.foreground))
                    .caret(ink(field.caret), ink(field.selection.background))
                    .pointer_buttons([PointerButton::Middle])
                    .on_edit(edit_access(move |hosted: &mut Hosted<T>| {
                        hosted.ui.input_edit(&edit_id)
                    }))
                    .on_input_change(move |hosted: &mut Hosted<T>, _: &mut Tx| {
                        let text = hosted.ui.input_text(&change_id);
                        on_type(&mut hosted.app, text);
                    })
                    .on_input_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                        if !plain_enter(ev) {
                            return false;
                        }
                        // The editor would submit on Enter's press. Hold it for the release.
                        if !tx.key_repeat() {
                            hosted.ui.arm_input_submit(&arm_id);
                        }
                        true
                    })
                    .on_key_up(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                        if !plain_enter(ev) || !hosted.ui.take_input_submit(&up_id) {
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
                    }),
            )
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
