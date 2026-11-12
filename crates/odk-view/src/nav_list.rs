//! The navigation of a sidebar on the Rune host: rows that each open a page, optionally under
//! group headings, one of them the current page (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Glyph;
use crate::ink::ink;
use crate::style::{self, focus_ring};
use crate::text::{Flow, words};
use crate::toggle::{Toggle, pressable};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::{Density, TypeRole};
use rune::{El, col, el, row};

/// What a screen reader is told of the current page's row, which Rune's tree has no state for.
const CURRENT_PAGE: &str = "Current page";

/// What the application hears when a page is opened: its value.
type Opened<T> = Rc<dyn Fn(&mut T, &str)>;

struct Item {
    value: String,
    label: String,
    icon: Option<Glyph>,
    counter: Option<String>,
    disabled: bool,
}

enum Entry {
    /// The heading of the items after it, such as "Workspace".
    Group(String),
    Item(Item),
}

/// The navigation of a sidebar: rows that each open a page, optionally under group headings, one
/// of them the current page. A row is a control's height at the density, with an optional icon
/// before its label and a counter after it; the current page has a quiet fill and an accent
/// indicator along its leading edge, which stay under the pointer and in focus.
///
/// The list is one tab stop, as every composite control is: Tab reaches the current page's row,
/// or the first enabled one. Up and Down move focus between enabled rows, wrapping, and Home and
/// End jump to the ends; Enter or Space opens the focused row, on its release, and so does a
/// press. Moving focus never opens a page. Disabled rows and headings take no focus. Rune does
/// the moving; Left and Right move as well, which the spec does not ask for.
///
/// Uncontrolled by default, starting on `default_value` and keeping the page in the host's
/// store; pass `value` to control it and follow `on_change`, which hears each new page once.
pub struct NavList<T> {
    id: String,
    name: String,
    entries: Vec<Entry>,
    value: Option<String>,
    default_value: Option<String>,
    on_change: Option<Opened<T>>,
    disabled: bool,
    density: Option<Density>,
}

impl<T: 'static> NavList<T> {
    /// A navigation named `name` for assistive technology, such as "Settings".
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            entries: Vec::new(),
            value: None,
            default_value: None,
            on_change: None,
            disabled: false,
            density: None,
        }
    }

    /// Starts a group: a heading over the items added after it.
    pub fn group(mut self, heading: impl Into<String>) -> Self {
        self.entries.push(Entry::Group(heading.into()));
        self
    }

    /// Adds a row that opens the page `value`, labelled `label`.
    pub fn item(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.entries.push(Entry::Item(Item {
            value: value.into(),
            label: label.into(),
            icon: None,
            counter: None,
            disabled: false,
        }));
        self
    }

    /// Puts an icon before the label of the row with this value, at the density's icon size: one
    /// of ODK's icons or a character.
    pub fn icon(mut self, value: &str, glyph: impl Into<Glyph>) -> Self {
        let glyph = glyph.into();
        for item in self.items_mut(value) {
            item.icon = Some(glyph.clone());
        }
        self
    }

    /// Puts a counter, such as `"12"`, at the end of the row with this value.
    pub fn counter(mut self, value: &str, counter: impl Into<String>) -> Self {
        let counter = counter.into();
        for item in self.items_mut(value) {
            item.counter = Some(counter.clone());
        }
        self
    }

    /// Disables the row with this value: it is shown but cannot be opened or focused.
    pub fn disable(mut self, value: &str) -> Self {
        for item in self.items_mut(value) {
            item.disabled = true;
        }
        self
    }

    /// Disables every row.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The list's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The current page, making the list controlled.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// Hears the page the user opened, when it is not the current one.
    pub fn on_change(mut self, handler: impl Fn(&mut T, &str) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn items_mut<'a>(&'a mut self, value: &'a str) -> impl Iterator<Item = &'a mut Item> {
        self.entries
            .iter_mut()
            .filter_map(move |entry| match entry {
                Entry::Item(item) if item.value == value => Some(item),
                _ => None,
            })
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let density = paint.tokens.density(self.density);
        let list = paint.tokens.nav_list(density);
        let current = self
            .value
            .clone()
            .or_else(|| hosted.ui.choice(&self.id).map(str::to_string))
            .or_else(|| self.default_value.clone());

        let rows: Vec<&Item> = self
            .entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Item(item) => Some(item),
                Entry::Group(_) => None,
            })
            .collect();
        let enabled: Vec<usize> = (0..rows.len())
            .filter(|i| !self.disabled && !rows[*i].disabled)
            .collect();
        // The one row in the Tab order: the current page's if it is enabled, else the first
        // enabled one, else none.
        let stop = enabled
            .iter()
            .copied()
            .find(|i| current.as_deref() == Some(rows[*i].value.as_str()))
            .or_else(|| enabled.first().copied());

        let mut column = apply(col().key(self.id.clone()), &accessible)
            .focus_group()
            .shrink();
        let (controlled, default) = (self.value.clone(), self.default_value.clone());
        let (mut row_n, mut group_n) = (0, 0);
        for entry in &self.entries {
            let item = match entry {
                Entry::Group(heading) => {
                    let mut header = words(
                        &paint,
                        heading.clone(),
                        list.header_role,
                        list.header,
                        Flow::End,
                    )
                    .px(list.header_padding_x)
                    .mb(list.header_gap);
                    if row_n > 0 || group_n > 0 {
                        header = header.mt(list.group_gap);
                    }
                    column = column.child(header);
                    group_n += 1;
                    continue;
                }
                Entry::Item(item) => item,
            };
            let index = row_n;
            row_n += 1;
            let usable = enabled.contains(&index);
            let on = current.as_deref() == Some(item.value.as_str());
            let s = paint
                .tokens
                .nav_item(density, States::NONE.selected(on).disabled(!usable));
            let g = s.geometry;
            let name = match &item.counter {
                Some(counter) => format!("{} ({counter})", item.label),
                None => item.label.clone(),
            };
            let mut row_accessible = Accessible::new(Role::Link, name)
                .selected(on)
                .disabled(!usable);
            // Rune's tree has no current-page state for a link, so the current row says so in
            // its description, which a screen reader reads.
            if on {
                row_accessible.description = Some(CURRENT_PAGE.to_string());
            }
            let key = format!("{}-{}", self.id, item.value);
            let mut line = apply(row().key(key.clone()), &row_accessible)
                .h(g.height)
                .px(g.padding_x)
                .gap(g.gap)
                .center_y()
                .radius(g.radius)
                .fill(ink(s.colours.default.background))
                .focusable(usable)
                .focus_selected(Some(index) == stop)
                .no_focus_ring();
            if s.responds {
                line = line
                    .hover(ink(s.colours.hover.background))
                    .press(ink(s.colours.pressed.background));
            }
            let foreground = s.colours.default.foreground;
            if let Some(glyph) = item.icon.clone() {
                line = line.child(glyph.slot(g.icon_size, ink(foreground)));
            }
            line = line.child(
                words(
                    &paint,
                    item.label.clone(),
                    g.type_role,
                    foreground,
                    Flow::End,
                )
                .grow(1.0),
            );
            if let Some(counter) = &item.counter {
                line = line.child(words(
                    &paint,
                    counter.clone(),
                    TypeRole::Small,
                    foreground,
                    Flow::End,
                ));
            }
            if let Some(indicator) = s.indicator {
                // Along the leading edge, over the fill: it takes no space.
                line = line.child(
                    el().absolute(0.0, 0.0)
                        .size(s.indicator_width, g.height)
                        .fill(ink(indicator))
                        .a11y_hidden(),
                );
            }
            line = focus_ring(line, &s.focus);
            if usable {
                let (id, value) = (self.id.clone(), item.value.clone());
                let (controlled, default) = (controlled.clone(), default.clone());
                let change = self.on_change.clone();
                let open: Toggle<T> = Rc::new(move |hosted: &mut Hosted<T>| {
                    let current = controlled
                        .clone()
                        .or_else(|| hosted.ui.choice(&id).map(str::to_string))
                        .or_else(|| default.clone());
                    if current.as_deref() == Some(value.as_str()) {
                        return;
                    }
                    if controlled.is_none() {
                        hosted.ui.set_choice(&id, &value);
                    }
                    if let Some(change) = &change {
                        change(&mut hosted.app, &value);
                    }
                });
                line = pressable(line, &key, true, open);
            }
            column = column.child(line);
        }
        column
    }
}

impl<T> Describe for NavList<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Navigation, self.name.clone()).disabled(self.disabled)
    }
}
