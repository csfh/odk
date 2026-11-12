//! A menu and the dropdown that opens it, on the Rune host (`docs/spec/odk-overlays.md`,
//! `DropdownMenu`, `Menu`): a button that opens a list of items below itself, which the keyboard
//! walks without leaving it.
//!
//! Rune's `focus_group` on a `menu` gives the keys: Up and Down move between the focusable items
//! and wrap, Home and End jump to the ends, and a letter jumps to the next item that starts with
//! it. Headings, separators and disabled items take no focus, so none of those reaches them.
//! Enter or Space chooses, Escape or a press outside closes it, and focus goes back to the button.

use std::rc::Rc;

use crate::button::Button;
use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::{Glyph, Icon};
use crate::ink::ink;
use crate::placement::{Align, Placement};
use crate::style::{self, focus_ring, inset_focus_ring};
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ButtonVariant, States};
use rune::{El, Key, KeyEvent, PointerPress, Tx, col, row, spacer};

/// The narrowest a menu is, in points at the base text size. ODK's own.
const MENU_MIN_WIDTH: f32 = 192.0;

type Select<T> = Rc<dyn Fn(&mut T)>;

/// Asks for the menu to be open (`true`) or closed (`false`).
type OpenChange<T> = Rc<dyn Fn(&mut T, bool)>;

/// The same at the host's level, for a menu that keeps its own open state in the store.
type HostedChange<T> = Rc<dyn Fn(&mut Hosted<T>, bool)>;

/// One choice in a menu: a label, and what choosing it does.
pub struct MenuItem<T> {
    label: String,
    hint: Option<String>,
    /// `Some(on)` for an item that is a check box.
    checked: Option<bool>,
    /// `Some(chosen)` for an item that is a radio.
    radio: Option<bool>,
    icon: Option<Glyph>,
    disabled: bool,
    danger: bool,
    on_select: Option<Select<T>>,
}

impl<T> MenuItem<T> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            checked: None,
            radio: None,
            icon: None,
            disabled: false,
            danger: false,
            on_select: None,
        }
    }

    /// Shortcut text at the item's end, muted. It only labels: the application binds the key.
    pub fn hint(mut self, chord: impl Into<String>) -> Self {
        self.hint = Some(chord.into());
        self
    }

    /// The item is a check box, with a check mark while it is on.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// The item is a radio, with a dot while it is the chosen one.
    pub fn radio(mut self, chosen: bool) -> Self {
        self.radio = Some(chosen);
        self
    }

    /// An icon before the label: one of ODK's, or a character.
    pub fn icon(mut self, glyph: impl Into<Glyph>) -> Self {
        self.icon = Some(glyph.into());
        self
    }

    /// The item is shown and takes no focus and no press.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The item does something that cannot be undone: the danger colours.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// What choosing it does. The menu closes after.
    pub fn on_select(mut self, handler: impl Fn(&mut T) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn role(&self) -> Role {
        if self.radio.is_some() {
            Role::MenuItemRadio
        } else if self.checked.is_some() {
            Role::MenuItemCheckbox
        } else {
            Role::MenuItem
        }
    }
}

enum Entry<T> {
    Item(MenuItem<T>),
    Separator,
    Heading(String),
}

/// A list of items, headings and separators.
pub struct Menu<T> {
    entries: Vec<Entry<T>>,
}

impl<T> Default for Menu<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Menu<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn item(mut self, item: MenuItem<T>) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// A label over a group of items: quiet text, not an item.
    pub fn heading(mut self, label: impl Into<String>) -> Self {
        self.entries.push(Entry::Heading(label.into()));
        self
    }

    /// What each item reports, for the checklist: its role and its label, and whether it is
    /// disabled or chosen.
    pub fn accessibles(&self) -> Vec<Accessible> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Item(item) => Some(item_accessible(item)),
                _ => None,
            })
            .collect()
    }
}

fn item_accessible<T>(item: &MenuItem<T>) -> Accessible {
    let accessible = Accessible::new(item.role(), item.label.clone()).disabled(item.disabled);
    match item.radio.or(item.checked) {
        Some(on) => accessible.selected(on),
        None => accessible,
    }
}

/// A button that opens a menu below itself. Enter, Space or Down opens it. Controlled: the
/// application owns `open` and hears `on_open_change`, which is asked for when the button is
/// pressed, when Escape or a press outside closes it, and when an item is chosen.
pub struct DropdownMenu<T> {
    id: String,
    label: String,
    menu: Option<Menu<T>>,
    open: bool,
    disabled: bool,
    on_open_change: Option<OpenChange<T>>,
}

impl<T: 'static> DropdownMenu<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            menu: None,
            open: false,
            disabled: false,
            on_open_change: None,
        }
    }

    pub fn menu(mut self, menu: Menu<T>) -> Self {
        self.menu = Some(menu);
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_open_change(mut self, handler: impl Fn(&mut T, bool) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// The button, with the open menu drawn into `layers`.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El {
        let ask: OpenChange<T> = self
            .on_open_change
            .clone()
            .unwrap_or_else(|| Rc::new(|_: &mut T, _: bool| {}));
        let open = self.open;
        let button = {
            let ask = ask.clone();
            Button::new(self.id.clone(), self.label.clone(), move |app: &mut T| {
                ask(app, !open)
            })
            .trailing_icon(Icon::ChevronDown)
            .variant(ButtonVariant::Outline)
            .disabled(self.disabled)
            .show(hosted)
            .expanded(open)
        };
        // Down opens it too, as the Button leaves it unhandled.
        let wrapper = {
            let ask = ask.clone();
            let enabled = !self.disabled;
            row().self_start().child(button).on_key(
                move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                    if enabled && !open && ev.key == Key::Down {
                        ask(&mut hosted.app, true);
                        return true;
                    }
                    false
                },
            )
        };
        if open
            && !self.disabled
            && let Some(menu) = self.menu
        {
            let gap = style::paint(hosted).tokens.foundation().spacing.s1;
            let hosted_ask: HostedChange<T> =
                Rc::new(move |h: &mut Hosted<T>, open| ask(&mut h.app, open));
            layers.push(
                surface(hosted, &self.id, &self.label, menu, &hosted_ask)
                    .modal(true, {
                        let close = hosted_ask.clone();
                        move |h: &mut Hosted<T>| close(h, false)
                    })
                    .focus_return(self.id.clone())
                    .anchored(
                        self.id.clone(),
                        Placement::Bottom.side(),
                        Align::Start.rune(),
                        gap,
                    ),
            );
        }
        wrapper
    }
}

/// A dropdown reports the button that opens it, named by its label; the open menu reports itself.
impl<T> Describe for DropdownMenu<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Button, self.label.clone()).disabled(self.disabled)
    }
}

/// The surface of an open menu: its items in a focus group, as the keys walk them. The caller
/// makes it modal and puts it where it goes.
fn surface<T: 'static>(
    hosted: &Hosted<T>,
    id: &str,
    name: &str,
    menu: Menu<T>,
    ask: &HostedChange<T>,
) -> El {
    let paint = style::paint(hosted);
    let s = paint.tokens.menu();
    let g = s.geometry;
    let accessible = Accessible::new(Role::Menu, name);
    let mut list = apply(col().key(format!("{id}-menu")), &accessible)
        .focus_group()
        .min_w(paint.px(MENU_MIN_WIDTH))
        .py(g.padding_y)
        .fill(ink(s.surface.background))
        .outline(g.border_width, 0.0, ink(s.surface.border))
        .radius(g.radius);
    for (index, entry) in menu.entries.into_iter().enumerate() {
        list = list.child(match entry {
            Entry::Separator => row()
                .h(s.separator_margin * 2.0 + g.border_width)
                .px(g.padding_x)
                .center_y()
                .child(
                    rune::el()
                        .h(g.border_width)
                        .grow(1.0)
                        .fill(ink(s.separator)),
                ),
            Entry::Heading(label) => row().px(g.padding_x).py(g.padding_y / 2.0).child(words(
                &paint,
                label,
                g.body,
                s.heading,
                Flow::End,
            )),
            Entry::Item(item) => item_row(&paint, id, index, item, ask),
        });
    }
    paint.present(list)
}

fn item_row<T: 'static>(
    paint: &style::Paint,
    id: &str,
    index: usize,
    item: MenuItem<T>,
    ask: &HostedChange<T>,
) -> El {
    let accessible = item_accessible(&item);
    let states = States::NONE.disabled(item.disabled);
    let chosen = item.radio.or(item.checked) == Some(true);
    let s = paint.tokens.menu_item(
        paint.tokens.density(None),
        states.selected(chosen),
        item.danger,
    );
    let g = s.geometry;
    let colour = s.colours.default.foreground;
    let mut content = row().center_y().gap(g.gap).grow(1.0);
    // The first column: a check mark, a dot, or the item's icon, so labels line up.
    let lead = if item.radio.is_some() {
        chosen.then_some(Glyph::from(Icon::Dot))
    } else if item.checked.is_some() {
        chosen.then_some(Glyph::from(Icon::Check))
    } else {
        item.icon.clone()
    };
    content = content.child(match lead {
        Some(glyph) => glyph.slot(g.icon_size, ink(colour)),
        None => rune::el().size(g.icon_size, g.icon_size),
    });
    content =
        content.child(words(paint, item.label.clone(), g.type_role, colour, Flow::End).grow(1.0));
    if let Some(hint) = &item.hint {
        content = content.child(spacer()).child(words(
            paint,
            hint.clone(),
            g.type_role,
            s.hint.default,
            Flow::End,
        ));
    }
    let mut element = inset_focus_ring(
        apply(row().key(format!("{id}-item-{index}")), &accessible),
        &paint.tokens.trigger_focus(),
    )
    .h(g.height)
    .px(g.padding_x)
    .center_y()
    .child(content);
    if item.disabled {
        return element;
    }
    element = element
        .focusable(true)
        .hover(ink(s.colours.hover.background))
        .press(ink(s.colours.pressed.background));
    let (select, close_ask) = (item.on_select, ask.clone());
    // Rune presses a focused menu item on Enter and on Space, as it does on a click.
    element.on_press(move |hosted: &mut Hosted<T>| {
        if let Some(select) = &select {
            select(&mut hosted.app);
        }
        close_ask(hosted, false);
    })
}

/// A menu that opens at the pointer when its trigger is right-clicked, and below the trigger with
/// the Menu key or Shift+F10 while it has focus. Rune sends both to one handler, so the keys are
/// not ours to bind. It keeps its own open state: an application that wants to know what was
/// chosen hears the items' `on_select`.
pub struct ContextMenu<T> {
    id: String,
    name: String,
    trigger: El,
    menu: Option<Menu<T>>,
    focusable: bool,
}

impl<T: 'static> ContextMenu<T> {
    /// A menu named `name` for assistive technology, such as "Row actions", for `trigger`; the
    /// name is required because the menu has no visible label of its own.
    pub fn new(id: impl Into<String>, name: impl Into<String>, trigger: El) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            trigger,
            menu: None,
            focusable: true,
        }
    }

    pub fn menu(mut self, menu: Menu<T>) -> Self {
        self.menu = Some(menu);
        self
    }

    /// Whether the trigger is a tab stop, so the keyboard can ask for the menu. True by default;
    /// off for a trigger that holds a control of its own.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// The trigger, with the open menu drawn into `layers`.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El {
        let paint = style::paint(hosted);
        let open = hosted.ui.is_open(&self.id, false);
        let id = self.id.clone();
        let ring = paint.tokens.trigger_focus();
        let trigger = focus_ring(
            apply(row().key(self.id.clone()), &self.accessible()).child(self.trigger),
            &ring,
        )
        .focusable(self.focusable)
        .on_secondary_press(
            move |hosted: &mut Hosted<T>, at: PointerPress, _: &mut Tx| {
                hosted.ui.set_menu_at(&id, (at.x, at.y));
                hosted.ui.set_open(&id, true);
            },
        );
        if open
            && let Some(menu) = self.menu
            && let Some((x, y)) = hosted.ui.menu_at(&self.id)
        {
            let id = self.id.clone();
            let ask: HostedChange<T> =
                Rc::new(move |h: &mut Hosted<T>, open| h.ui.set_open(&id, open));
            let close = ask.clone();
            layers.push(
                surface(hosted, &self.id, &self.name, menu, &ask)
                    .modal(true, move |h: &mut Hosted<T>| close(h, false))
                    .focus_return(self.id.clone())
                    // At the pointer, in window points.
                    .pin(Some(x), Some(y), None, None),
            );
        }
        trigger
    }
}

impl<T> Describe for ContextMenu<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Group, self.name.clone())
    }
}

/// Opens `menu` below the element keyed `id` while the store says it is open, closing it the same
/// way: for a trigger that keeps no state of its own, such as a breadcrumb's folded `…`. Escape or
/// a press outside closes it and focus goes back to the trigger.
pub(crate) fn below_trigger<T: 'static>(
    hosted: &Hosted<T>,
    id: &str,
    name: &str,
    menu: Menu<T>,
    layers: &mut Vec<El>,
) {
    if !hosted.ui.is_open(id, false) {
        return;
    }
    let gap = style::paint(hosted).tokens.foundation().spacing.s1;
    let key = id.to_string();
    let ask: HostedChange<T> = Rc::new(move |h: &mut Hosted<T>, open| h.ui.set_open(&key, open));
    let close = ask.clone();
    layers.push(
        surface(hosted, id, name, menu, &ask)
            .modal(true, move |h: &mut Hosted<T>| close(h, false))
            .focus_return(id.to_string())
            .anchored(
                id.to_string(),
                Placement::Bottom.side(),
                Align::Start.rune(),
                gap,
            ),
    );
}

/// Opens `menu` at the point the store holds for `id` while it is open: for a menu that belongs to
/// a row or an item the caller keeps, such as a table row's. Escape or a press outside closes it,
/// and focus goes back to the element keyed `focus`.
pub(crate) fn at_point<T: 'static>(
    hosted: &Hosted<T>,
    id: &str,
    name: &str,
    focus: &str,
    menu: Menu<T>,
    layers: &mut Vec<El>,
) {
    let Some((x, y)) = hosted.ui.menu_at(id) else {
        return;
    };
    let key = id.to_string();
    let ask: HostedChange<T> = Rc::new(move |h: &mut Hosted<T>, open| h.ui.set_open(&key, open));
    let close = ask.clone();
    layers.push(
        surface(hosted, id, name, menu, &ask)
            .modal(true, move |h: &mut Hosted<T>| close(h, false))
            .focus_return(focus.to_string())
            .pin(Some(x), Some(y), None, None),
    );
}
