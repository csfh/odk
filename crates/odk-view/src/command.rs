//! A searchable list of commands in a modal panel, on the Rune host, on the dialog's surface and
//! the menu item's tokens (`docs/spec/odk-overlays.md`, `docs/spec/odk-text-input.md`).

use std::rc::Rc;

use crate::describe::apply;
use crate::dialog::OnClose;
use crate::host::Hosted;
use crate::ink::ink;
use crate::input::{Input, KeyHook};
use crate::style::{self, text_style};
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::Density;
use rune::{El, GrowthSpec, Key, KeyEvent, ScrollSpec, Tx, col, el, row, spacer, text};

/// The widest the panel is, in points at the base text size. ODK's own (ADR 0035).
const PANEL_WIDTH: f32 = 560.0;

/// The tallest the list is, unless the window leaves less, in points at the base text size.
const LIST_HEIGHT: f32 = 320.0;

/// How far down the window the panel's top is. The panel hangs from there, so that the field
/// stays where it is as the list narrows.
const TOP: f32 = 0.18;

type Select<T> = Rc<dyn Fn(&mut T)>;

/// What a request to close does, from inside the host.
type Ask<T> = Rc<dyn Fn(&mut Hosted<T>)>;

/// One command: its label, an optional shortcut shown at the end, and words that find it without
/// being shown.
pub struct CommandItem<T> {
    label: String,
    hint: Option<String>,
    keywords: String,
    disabled: bool,
    on_select: Option<Select<T>>,
}

impl<T> CommandItem<T> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            keywords: String::new(),
            disabled: false,
            on_select: None,
        }
    }

    /// A shortcut, shown at the end of the row.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Words that find the command and that nothing shows.
    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.keywords = keywords.into();
        self
    }

    /// A command that cannot be chosen now: listed and dimmed, skipped by the arrows.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_select(mut self, select: impl Fn(&mut T) + 'static) -> Self {
        self.on_select = Some(Rc::new(select));
        self
    }
}

/// Whether `query` finds an item: every word of it, lowercased, is somewhere in the label or the
/// keywords, lowercased. An empty query finds everything.
fn finds(query: &str, label: &str, keywords: &str) -> bool {
    let haystack = format!("{label} {keywords}").to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

/// What the palette lists for a query: the items that match, those of them that can be chosen,
/// and the one Enter would run, which is `highlighted` if it can be chosen and otherwise the first
/// that can.
struct Listing {
    matching: Vec<usize>,
    enabled: Vec<usize>,
    current: Option<usize>,
}

fn listing<T>(query: &str, items: &[CommandItem<T>], highlighted: Option<usize>) -> Listing {
    let matching: Vec<usize> = (0..items.len())
        .filter(|i| finds(query, &items[*i].label, &items[*i].keywords))
        .collect();
    let enabled: Vec<usize> = matching
        .iter()
        .copied()
        .filter(|i| !items[*i].disabled)
        .collect();
    let current = highlighted
        .filter(|h| enabled.contains(h))
        .or(enabled.first().copied());
    Listing {
        matching,
        enabled,
        current,
    }
}

/// Where the highlight goes from `current` among the `enabled` items (indices, in order) when
/// moving `forward` or back, wrapping at the ends. With no current item, the first or last.
fn step(enabled: &[usize], current: Option<usize>, forward: bool) -> Option<usize> {
    let len = enabled.len();
    let at = current.and_then(|c| enabled.iter().position(|e| *e == c));
    let next = match (at, forward) {
        (Some(at), true) => (at + 1) % len,
        (Some(at), false) => (at + len - 1) % len,
        (None, true) => 0,
        (None, false) => len.checked_sub(1)?,
    };
    enabled.get(next).copied()
}

/// A searchable list of commands in a modal panel hanging from the top of the window. Controlled:
/// the application owns `open` and hears `on_close`, which is asked for when Escape, a press on the
/// scrim or a choice closes it. Each time it opens the field is empty and the first command that
/// can be chosen is highlighted. Focus is in the field and stays there: Up and Down move the
/// highlight and wrap past the commands that cannot be chosen, and Enter, on its release, runs the
/// highlighted command and asks to close. A press runs a command the same way. Typing narrows the
/// list to the commands that have every word of the query in their label or keywords.
pub struct Command<T> {
    id: String,
    name: String,
    open: bool,
    placeholder: String,
    empty: String,
    items: Vec<CommandItem<T>>,
    on_close: Option<OnClose<T>>,
    returns_focus_to: Option<String>,
}

impl<T: 'static> Command<T> {
    /// A palette named `name` for assistive technology, such as "Command palette".
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            open: false,
            placeholder: "Search…".into(),
            empty: "No results".into(),
            items: Vec::new(),
            on_close: None,
            returns_focus_to: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The field's text while it is empty. It is also the field's accessible name.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The line shown when nothing matches.
    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.empty = empty.into();
        self
    }

    pub fn item(mut self, item: CommandItem<T>) -> Self {
        self.items.push(item);
        self
    }

    pub fn on_close(mut self, close: impl Fn(&mut T) + 'static) -> Self {
        self.on_close = Some(Rc::new(close));
        self
    }

    /// The key of the element that gets focus back when the palette closes.
    pub fn returns_focus_to(mut self, key: impl Into<String>) -> Self {
        self.returns_focus_to = Some(key.into());
        self
    }

    /// What each row reports, for a query and the highlighted item: an option, selected when it is
    /// the highlighted one and disabled when it cannot be chosen.
    pub fn item_accessibles(&self, query: &str, highlighted: Option<usize>) -> Vec<Accessible> {
        let found = listing(query, &self.items, highlighted);
        found
            .matching
            .iter()
            .map(|i| {
                Accessible::new(Role::Option, self.items[*i].label.clone())
                    .selected(found.current == Some(*i))
                    .disabled(self.items[*i].disabled)
            })
            .collect()
    }

    /// Draws the palette into `layers` when it is open.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) {
        let field_id = format!("{}-field", self.id);
        if hosted.ui.command_opened(&self.id, self.open) {
            hosted.ui.reset_input(&field_id);
        }
        if !self.open {
            return;
        }
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.command();
        let o = s.overlay;
        let g = o.geometry;
        let spacing = &paint.tokens.foundation().spacing;

        let query = hosted.ui.input_text(&field_id);
        let found = listing(
            &query,
            &self.items,
            hosted.ui.command_highlight(&self.id, &query),
        );
        let close_app: OnClose<T> = self
            .on_close
            .clone()
            .unwrap_or_else(|| Rc::new(|_: &mut T| {}));
        let ask: Ask<T> = {
            let close_app = close_app.clone();
            Rc::new(move |hosted: &mut Hosted<T>| close_app(&mut hosted.app))
        };

        // Rows.
        // Rune reveals the selected row of a virtualized stack, so the list is one, of rows that
        // are all as tall as a menu item.
        let row_height = paint
            .tokens
            .menu_item(Density::Default, States::NONE, false)
            .geometry
            .height;
        let mut rows = col()
            .w_full()
            .growth(GrowthSpec::default())
            .virtualize(row_height)
            .reveal_selected(true);
        for i in &found.matching {
            let item = &self.items[*i];
            let highlighted = found.current == Some(*i);
            let states = States::NONE
                .disabled(item.disabled)
                .highlighted(highlighted);
            let m = paint.tokens.menu_item(Density::Default, states, false);
            let mg = m.geometry;
            let colour = m.colours.default.foreground;
            let row_accessible = Accessible::new(Role::Option, item.label.clone())
                .selected(highlighted)
                .disabled(item.disabled);
            let mut content = row().center_y().gap(mg.gap).grow(1.0).child(
                words(&paint, item.label.clone(), mg.type_role, colour, Flow::End).grow(1.0),
            );
            if let Some(hint) = &item.hint {
                content = content.child(spacer()).child(words(
                    &paint,
                    hint.clone(),
                    mg.type_role,
                    m.hint.default,
                    Flow::End,
                ));
            }
            let mut element = apply(row().key(format!("{}-item-{i}", self.id)), &row_accessible)
                .w_full()
                .h(mg.height)
                .px(mg.padding_x)
                .center_y()
                .fill(ink(m.colours.default.background))
                .child(content);
            if !item.disabled {
                let (index, id, query_here) = (*i, self.id.clone(), query.clone());
                let (select, ask) = (item.on_select.clone(), ask.clone());
                element = element
                    .on_hover_change(move |hosted: &mut Hosted<T>, inside: bool, _: &mut Tx| {
                        if inside {
                            hosted.ui.set_command_highlight(&id, index, &query_here);
                        }
                    })
                    .press(ink(m.colours.pressed.background))
                    .on_press(move |hosted: &mut Hosted<T>| {
                        if let Some(select) = &select {
                            select(&mut hosted.app);
                        }
                        ask(hosted);
                    });
            }
            rows = rows.child(element);
        }
        let list = if found.matching.is_empty() {
            row()
                .key(format!("{}-empty", self.id))
                .w_full()
                .center()
                .py(spacing.s4)
                .child(
                    text(self.empty.clone())
                        .font(text_style(paint.body))
                        .color(ink(s.empty)),
                )
        } else {
            apply(
                col().key(format!("{}-list", self.id)),
                &Accessible::new(Role::Listbox, self.name.clone()),
            )
            .w_full()
            .max_h(paint.px(LIST_HEIGHT))
            .shrink()
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            // A press on a row must not take focus out of the field.
            .preserve_focus_on_press(true)
            .child(rows)
        };

        // The keys the field hands over: Up and Down move the highlight, Enter runs it on release.
        let hook: KeyHook<T> = {
            let (id, field, enabled) = (self.id.clone(), field_id.clone(), found.enabled.clone());
            let current = found.current;
            let select: Vec<Option<Select<T>>> =
                self.items.iter().map(|i| i.on_select.clone()).collect();
            let ask = ask.clone();
            Rc::new(
                move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx, up: bool| {
                    if up {
                        let plain =
                            !ev.mods.shift && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.logo;
                        if ev.key != Key::Enter || !plain || !hosted.ui.take_input_submit(&field) {
                            return false;
                        }
                        let Some(at) = current else {
                            return true;
                        };
                        if let Some(Some(select)) = select.get(at) {
                            select(&mut hosted.app);
                        }
                        ask(hosted);
                        return true;
                    }
                    match ev.key {
                        Key::Down | Key::Up => {
                            let query = hosted.ui.input_text(&field);
                            if let Some(next) = step(&enabled, current, ev.key == Key::Down) {
                                hosted.ui.set_command_highlight(&id, next, &query);
                            }
                            true
                        }
                        Key::Enter if !tx.key_repeat() => {
                            hosted.ui.arm_input_submit(&field);
                            true
                        }
                        Key::Enter => true,
                        _ => false,
                    }
                },
            )
        };
        let field = Input::new(
            field_id,
            self.placeholder.clone(),
            |_: &mut T, _: String| {},
            |_: &mut T| {},
        )
        .placeholder(self.placeholder.clone())
        .full_width()
        .show_with(hosted, Some(hook));

        let panel = apply(col().key(self.id.clone()), &accessible)
            .w_full()
            .max_w(paint.px(PANEL_WIDTH))
            .padding(spacing.s2)
            .gap(spacing.s2)
            .fill(ink(o.surface.background))
            .outline(g.border_width, 0.0, ink(o.surface.border))
            .radius(g.radius)
            // A press on the panel is the panel's: it is not the scrim's.
            .on_press(|_: &mut Hosted<T>| {})
            .child(field)
            .child(list);
        let close = {
            let ask = ask.clone();
            move |hosted: &mut Hosted<T>| ask(hosted)
        };
        // The panel hangs from a fixed fraction of the window, so the field stays put as the list
        // narrows: both parts have no height of their own and share the window's.
        let mut scrim = col()
            .key(format!("{}-scrim", self.id))
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .items_center()
            .padding(spacing.s4)
            .child(el().h(0.0).grow(TOP))
            .child(
                col()
                    .w_full()
                    .h(0.0)
                    .grow(1.0 - TOP)
                    .min_h(0.0)
                    .items_center()
                    .child(panel),
            )
            .modal(false, close.clone())
            .on_press(move |hosted: &mut Hosted<T>| close(hosted));
        if let Some(key) = self.returns_focus_to {
            scrim = scrim.focus_return(key);
        }
        if let Some(backdrop) = o.backdrop {
            scrim = scrim.fill(ink(backdrop));
        }
        layers.push(scrim);
    }
}

impl<T> Describe for Command<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.name.clone())
    }
}
