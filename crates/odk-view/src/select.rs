//! A select on the Rune host. The open list and its highlight live in the store.

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, text_style};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::Density;
use rune::{Align, El, Key, KeyEvent, Side, Tx, col, row, text};

/// The trigger's and the list's width in points at the base text size, unless the application
/// sets one (ODK's own).
const DEFAULT_WIDTH: f32 = 180.0;

/// The space between the trigger and its open list, in points (ODK's own).
const LIST_GAP: f32 = 4.0;

/// One choice from `options`. The open list is drawn into `layers`.
pub struct Select<F> {
    id: String,
    label: String,
    options: Vec<String>,
    selected: Option<usize>,
    width: f32,
    on_select: F,
}

impl<F> Select<F> {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        options: Vec<String>,
        selected: Option<usize>,
        on_select: F,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options,
            selected,
            width: DEFAULT_WIDTH,
            on_select,
        }
    }

    /// The trigger's and the list's width in points at the base text size; the user's text size
    /// scales it.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl<F> Describe for Select<F> {
    fn accessible(&self) -> Accessible {
        let accessible = Accessible::new(Role::Combobox, self.label.clone());
        match self.selected.and_then(|index| self.options.get(index)) {
            Some(option) => accessible.value(option.clone()),
            None => accessible,
        }
    }
}

impl<F> Select<F> {
    pub fn show<T>(self, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El
    where
        T: 'static,
        F: Fn(&mut T, usize) + Clone + 'static,
    {
        let accessible = self.accessible();
        let Select {
            id,
            label,
            options,
            selected,
            width,
            on_select,
        } = self;
        let paint = style::paint(hosted.theme());
        let open = hosted.ui.select(&id).is_some_and(|slot| slot.open);
        let highlight = hosted
            .ui
            .select(&id)
            .map(|slot| slot.highlight)
            .unwrap_or(0);
        let field = paint
            .tokens
            .field(Density::Default, States::NONE.expanded(open));
        let width = paint.px(width);
        let trigger_key = format!("{id}-trigger");
        let value = selected
            .and_then(|index| options.get(index))
            .cloned()
            .unwrap_or_else(|| label.clone());
        let open_id = id.clone();
        let key_id = id.clone();
        let press_len = options.len();
        let key_len = options.len();
        let chosen = selected;
        let trigger = apply(row().key(trigger_key.clone()), &accessible)
            .expanded(open)
            .focusable(true)
            .focus_ring(
                field.focus.width,
                field.focus.offset,
                ink(field.focus.colour),
            )
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
            .on_press(move |hosted: &mut Hosted<T>| {
                let slot = hosted.ui.select_mut(&open_id);
                if !slot.open {
                    reveal(slot, chosen, press_len);
                }
            })
            .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                if tx.key_repeat() || !opens(ev) {
                    return false;
                }
                let slot = hosted.ui.select_mut(&key_id);
                if !slot.open {
                    reveal(slot, chosen, key_len);
                }
                true
            })
            .child(
                text(value)
                    .font(text_style(paint.body))
                    .color(ink(if selected.is_some() {
                        field.colours.default.foreground
                    } else {
                        field.placeholder
                    }))
                    .a11y_hidden(),
            )
            .child(
                text("▾")
                    .font(text_style(paint.body))
                    .color(ink(field.icon))
                    .a11y_hidden(),
            );
        if !open || options.is_empty() {
            return trigger;
        }
        let menu = paint.tokens.menu();
        let mut list = apply(
            col().key(format!("{id}-list")),
            &Accessible::new(Role::Listbox, label),
        )
        .focusable(true)
        .w(width)
        .padding(menu.geometry.padding_y)
        .on_key({
            let id = id.clone();
            let options = options.clone();
            let on_select = on_select.clone();
            move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| match list_action(
                hosted
                    .ui
                    .select(&id)
                    .map(|slot| slot.highlight)
                    .unwrap_or(0),
                &options,
                ev,
                tx.key_repeat(),
            ) {
                None => false,
                Some(Action::Move(index)) => {
                    hosted.ui.select_mut(&id).highlight = index;
                    true
                }
                Some(Action::Choose(index)) => {
                    choose(hosted, &id, index, &on_select);
                    true
                }
            }
        });
        for (index, option) in options.iter().enumerate() {
            let item = paint.tokens.select_item(
                Density::Default,
                States::NONE
                    .highlighted(index == highlight)
                    .selected(selected == Some(index)),
            );
            let id = id.clone();
            let on_select = on_select.clone();
            list = list.child(
                apply(
                    row().key(format!("{id}-{index}")),
                    &Accessible::new(Role::Option, option.clone())
                        .selected(selected == Some(index)),
                )
                .position_in_set(index + 1, options.len())
                .h(item.geometry.height)
                .px(item.geometry.padding_x)
                .center_y()
                .radius(item.geometry.radius)
                .fill(ink(item.current.background))
                .on_press(move |hosted: &mut Hosted<T>| {
                    choose(hosted, &id, index, &on_select);
                })
                .child(
                    text(option.clone())
                        .font(text_style(paint.body))
                        .color(ink(item.current.foreground))
                        .a11y_hidden(),
                ),
            );
        }
        // Escape is left for the modal, which closes the list and returns focus to the trigger.
        layers.push(
            col()
                .key(format!("{id}-popup"))
                .focus_return(trigger_key.clone())
                .min_w(width)
                .radius(menu.geometry.radius)
                .fill(ink(menu.surface.background))
                .outline(menu.geometry.border_width, 0.0, ink(menu.surface.border))
                .modal(true, {
                    let id = id.clone();
                    move |hosted: &mut Hosted<T>| {
                        hosted.ui.select_mut(&id).open = false;
                    }
                })
                .child(list)
                .anchored(trigger_key, Side::Bottom, Align::Start, LIST_GAP),
        );
        trigger
    }
}

fn reveal(slot: &mut crate::store::SelectSlot, selected: Option<usize>, len: usize) {
    slot.open = true;
    if len == 0 {
        slot.highlight = 0;
        return;
    }
    slot.highlight = selected.unwrap_or(0).min(len - 1);
}

fn choose<T>(hosted: &mut Hosted<T>, id: &str, index: usize, on_select: &impl Fn(&mut T, usize)) {
    let slot = hosted.ui.select_mut(id);
    slot.open = false;
    slot.highlight = index;
    on_select(&mut hosted.app, index);
}

enum Action {
    Move(usize),
    Choose(usize),
}

fn list_action(
    highlight: usize,
    options: &[String],
    ev: &KeyEvent,
    repeat: bool,
) -> Option<Action> {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return None;
    }
    let len = options.len();
    if len == 0 {
        return None;
    }
    let last = len - 1;
    match &ev.key {
        Key::Down => Some(Action::Move((highlight + 1).min(last))),
        Key::Up => Some(Action::Move(highlight.saturating_sub(1))),
        Key::Home => Some(Action::Move(0)),
        Key::End => Some(Action::Move(last)),
        Key::Enter => (!repeat).then_some(Action::Choose(highlight)),
        Key::Text(text) if text == " " => (!repeat).then_some(Action::Choose(highlight)),
        Key::Text(text) => typed(text)
            .and_then(|letter| next_match(options, highlight, letter))
            .map(Action::Move),
        _ => None,
    }
}

fn opens(ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return false;
    }
    matches!(ev.key, Key::Enter | Key::Down | Key::Up)
        || matches!(&ev.key, Key::Text(text) if text == " ")
}

fn typed(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let letter = chars.next()?;
    if chars.next().is_some() || letter.is_whitespace() {
        return None;
    }
    Some(letter)
}

fn next_match(options: &[String], highlight: usize, letter: char) -> Option<usize> {
    let len = options.len();
    let letter = letter.to_ascii_lowercase();
    (1..=len)
        .map(|offset| (highlight + offset) % len)
        .find(|&index| {
            options[index]
                .chars()
                .next()
                .is_some_and(|c| c.to_ascii_lowercase() == letter)
        })
}
