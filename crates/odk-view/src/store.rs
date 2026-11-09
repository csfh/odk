//! Component state the application does not hold: a held key, an open list, a caret.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use rune::TextEdit;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Activator {
    Enter,
    Space,
}

#[derive(Default)]
pub(crate) struct ButtonSlot {
    pub armed: Option<Activator>,
}

#[derive(Default)]
pub(crate) struct SelectSlot {
    pub open: bool,
    pub highlight: usize,
}

pub(crate) struct InputSlot {
    pub edit: TextEdit,
    /// Enter is down. Its release submits once.
    pub submit_armed: bool,
    /// The selection last drawn. A change that is not [`Self::programmatic`] writes the primary
    /// selection when it is non-empty.
    seen: Cell<Option<(usize, usize)>>,
    /// A selection set by [`Store::set_input_selection`]. It is not a user selection.
    programmatic: Cell<Option<(usize, usize)>>,
}

impl Default for InputSlot {
    fn default() -> Self {
        Self {
            edit: TextEdit::new(""),
            submit_armed: false,
            seen: Cell::new(None),
            programmatic: Cell::new(None),
        }
    }
}

/// What a component keeps for itself, keyed by the id the builder takes.
#[derive(Default)]
pub struct Store {
    buttons: HashMap<String, ButtonSlot>,
    selects: HashMap<String, SelectSlot>,
    inputs: HashMap<String, InputSlot>,
    toggles: HashMap<String, bool>,
    choices: HashMap<String, String>,
    selections: HashMap<String, Vec<String>>,
    primary: RefCell<String>,
}

impl Store {
    /// Whether the uncontrolled toggle `id` is on: what a press last made it, else `default`.
    pub(crate) fn toggle_value(&self, id: &str, default: bool) -> bool {
        self.toggles.get(id).copied().unwrap_or(default)
    }

    /// The option an uncontrolled group `id` last chose, if one was.
    pub(crate) fn choice(&self, id: &str) -> Option<&str> {
        self.choices.get(id).map(String::as_str)
    }

    /// The options an uncontrolled segmented group `id` last had on, if it was ever changed.
    pub(crate) fn selection(&self, id: &str) -> Option<&[String]> {
        self.selections.get(id).map(Vec::as_slice)
    }

    pub(crate) fn set_selection(&mut self, id: &str, values: Vec<String>) {
        self.selections.insert(id.to_string(), values);
    }

    pub(crate) fn set_choice(&mut self, id: &str, value: &str) {
        self.choices.insert(id.to_string(), value.to_string());
    }

    pub(crate) fn set_toggle(&mut self, id: &str, on: bool) {
        self.toggles.insert(id.to_string(), on);
    }

    pub(crate) fn button_mut(&mut self, id: &str) -> &mut ButtonSlot {
        self.buttons.entry(id.to_string()).or_default()
    }

    pub(crate) fn select(&self, id: &str) -> Option<&SelectSlot> {
        self.selects.get(id)
    }

    pub(crate) fn select_mut(&mut self, id: &str) -> &mut SelectSlot {
        self.selects.entry(id.to_string()).or_default()
    }

    pub(crate) fn input_edit(&mut self, id: &str) -> &mut TextEdit {
        &mut self.inputs.entry(id.to_string()).or_default().edit
    }

    pub(crate) fn input_edit_clone(&self, id: &str) -> TextEdit {
        self.inputs
            .get(id)
            .map(|slot| slot.edit.clone())
            .unwrap_or_else(|| TextEdit::new(""))
    }

    pub(crate) fn arm_input_submit(&mut self, id: &str) {
        self.inputs.entry(id.to_string()).or_default().submit_armed = true;
    }

    pub(crate) fn take_input_submit(&mut self, id: &str) -> bool {
        let Some(slot) = self.inputs.get_mut(id) else {
            return false;
        };
        std::mem::take(&mut slot.submit_armed)
    }

    pub(crate) fn input_text(&self, id: &str) -> String {
        self.inputs
            .get(id)
            .map(|slot| slot.edit.text.clone())
            .unwrap_or_default()
    }

    /// The primary selection the focused field last wrote from a user selection.
    pub fn primary(&self) -> String {
        self.primary.borrow().clone()
    }

    /// Move the field's caret or selection without writing the primary selection.
    pub fn set_input_selection(&mut self, id: &str, anchor: usize, head: usize) {
        let slot = self.inputs.entry(id.to_string()).or_default();
        slot.edit.set_selection(anchor, head);
        slot.programmatic.set(Some(slot.edit.range()));
    }

    /// A user selection that is non-empty writes the primary selection. A programmatic one does not.
    pub(crate) fn note_input_selection(&self, id: &str) {
        let write = {
            let Some(slot) = self.inputs.get(id) else {
                return;
            };
            let range = slot.edit.range();
            if slot.programmatic.get() == Some(range) {
                slot.seen.set(Some(range));
                return;
            }
            if slot.programmatic.get().is_some() {
                slot.programmatic.set(None);
            }
            let write = if slot.seen.get() != Some(range) {
                let selected = slot.edit.selected().to_string();
                (!selected.is_empty()).then_some(selected)
            } else {
                None
            };
            slot.seen.set(Some(range));
            write
        };
        if let Some(selected) = write {
            *self.primary.borrow_mut() = selected;
        }
    }
}
