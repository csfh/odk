//! Component state the application does not hold: a held key, an open list, a caret.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

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
pub(crate) struct ResizeSlot {
    /// Each panel's fraction of the group, once it has been changed.
    pub sizes: Vec<f32>,
    /// The handle being dragged and the sizes the drag began from, which Escape puts back.
    pub start: Option<(usize, Vec<f32>)>,
    /// The handle under the pointer.
    pub hovered: Option<usize>,
    /// How long the group was laid out along its axis, in points.
    pub length: f32,
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

/// Which part of a hover overlay the pointer is on.
#[derive(Clone, Copy)]
pub(crate) enum Part {
    Trigger,
    Panel,
}

/// A tooltip's or a hover card's pointer: on its trigger, on its panel, and whether Escape closed it
/// until the pointer has left both.
#[derive(Default, Clone, Copy)]
struct Hovering {
    trigger: bool,
    panel: bool,
    dismissed: bool,
}

/// What a component keeps for itself, keyed by the id the builder takes.
#[derive(Default)]
pub struct Store {
    buttons: HashMap<String, ButtonSlot>,
    selects: HashMap<String, SelectSlot>,
    resizes: HashMap<String, ResizeSlot>,
    menu_at: HashMap<String, (f32, f32)>,
    inputs: HashMap<String, InputSlot>,
    toggles: HashMap<String, bool>,
    choices: HashMap<String, String>,
    selections: HashMap<String, Vec<String>>,
    /// Which overlays are open: a hover card, a tooltip or an uncontrolled popover.
    open: HashMap<String, bool>,
    /// The pointer of each tooltip and hover card.
    hovers: HashMap<String, Hovering>,
    /// Whether the build in progress has a spinner in it.
    spinner_building: Cell<bool>,
    /// Whether the last finished build had a spinner in it, so the host's poll has one to turn.
    spinner_seen: Cell<bool>,
    /// The glyph of the turn the poll last chose, which every spinner shows.
    spinner_frame: Cell<usize>,
    /// The moment the turns count from, so every spinner shows the same glyph at the same time.
    spinner_epoch: Cell<Option<f64>>,
    primary: RefCell<String>,
    /// The decoded images of the window's `Image`s.
    pub(crate) images: crate::images::Images,
    /// The boxes, transitions and drags of the window's `Chart`s.
    pub(crate) charts: crate::chart::Charts,
    /// The window's toasts, which the host's poll steps.
    toasts: RefCell<Option<Rc<dyn crate::toast::Clock>>>,
}

impl Store {
    /// Remembers the toasts' clock, for the host's poll.
    pub(crate) fn attach_clock(&self, clock: Rc<dyn crate::toast::Clock>) {
        *self.toasts.borrow_mut() = Some(clock);
    }

    /// Steps the toasts' clocks to `now`; says whether a toast went.
    pub(crate) fn step_toasts(&self, now: f64, tx: &mut rune::Tx) -> bool {
        let clock = self.toasts.borrow().clone();
        clock.is_some_and(|clock| clock.advance(now, tx))
    }

    /// A spinner was built in the view that is being built.
    pub(crate) fn note_spinner(&self) {
        self.spinner_building.set(true);
    }

    /// The view is built: what it had in it is what the poll turns, until the next build.
    pub(crate) fn end_build(&self) {
        self.spinner_seen.set(self.spinner_building.replace(false));
    }

    /// Whether the last build had a spinner: the only time the host asks for a wake.
    pub(crate) fn spinning(&self) -> bool {
        self.spinner_seen.get()
    }

    pub(crate) fn spinner_frame(&self) -> usize {
        self.spinner_frame.get()
    }

    /// Keeps `frame` and says whether it differs from the one before.
    pub(crate) fn set_spinner_frame(&self, frame: usize) -> bool {
        self.spinner_frame.replace(frame) != frame
    }

    /// Where the turns count from: the first moment a poll asked.
    pub(crate) fn spinner_epoch(&self, now: f64) -> f64 {
        match self.spinner_epoch.get() {
            Some(epoch) => epoch,
            None => {
                self.spinner_epoch.set(Some(now));
                now
            }
        }
    }

    /// Whether the uncontrolled toggle `id` is on: what a press last made it, else `default`.
    pub(crate) fn toggle_value(&self, id: &str, default: bool) -> bool {
        self.toggles.get(id).copied().unwrap_or(default)
    }

    /// Whether the hover overlay `id` is shown: the pointer is on its trigger or its panel, and
    /// Escape has not closed it since.
    pub(crate) fn hover_open(&self, id: &str) -> bool {
        self.hovers
            .get(id)
            .is_some_and(|h| (h.trigger || h.panel) && !h.dismissed)
    }

    /// The pointer entered or left `part` of the hover overlay `id`. Once it has left both, an
    /// overlay that Escape closed may open again.
    pub(crate) fn hover_set(&mut self, id: &str, part: Part, inside: bool) {
        let slot = self.hovers.entry(id.to_string()).or_default();
        match part {
            Part::Trigger => slot.trigger = inside,
            Part::Panel => slot.panel = inside,
        }
        if !slot.trigger && !slot.panel {
            slot.dismissed = false;
        }
    }

    /// Escape: closed until the pointer has left.
    pub(crate) fn hover_dismiss(&mut self, id: &str) {
        self.hovers.entry(id.to_string()).or_default().dismissed = true;
    }

    /// Whether the overlay `id` is open: what was last set, else `default`.
    pub(crate) fn is_open(&self, id: &str, default: bool) -> bool {
        self.open.get(id).copied().unwrap_or(default)
    }

    pub(crate) fn set_open(&mut self, id: &str, open: bool) {
        self.open.insert(id.to_string(), open);
    }

    /// Where a context menu was asked for, in window points.
    pub(crate) fn menu_at(&self, id: &str) -> Option<(f32, f32)> {
        self.menu_at.get(id).copied()
    }

    pub(crate) fn set_menu_at(&mut self, id: &str, at: (f32, f32)) {
        self.menu_at.insert(id.to_string(), at);
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

    pub(crate) fn resize(&self, id: &str) -> Option<&ResizeSlot> {
        self.resizes.get(id)
    }

    pub(crate) fn resize_mut(&mut self, id: &str) -> &mut ResizeSlot {
        self.resizes.entry(id.to_string()).or_default()
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
