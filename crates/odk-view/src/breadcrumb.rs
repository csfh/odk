//! A path of links ending in the current page, on the Rune host, on the nav link's component
//! tokens (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::{Glyph, Icon};
use crate::ink::ink;
use crate::menu::{Menu, MenuItem, below_trigger};
use crate::style::{self, text_style};
use crate::text::{Flow, flowed};
use crate::toggle::{Toggle, pressable};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use rune::{El, Key, KeyEvent, Link, Tx, row, text};

type Handler<T> = Rc<dyn Fn(&mut T)>;

struct Crumb<T> {
    label: String,
    on_click: Option<Handler<T>>,
}

/// A path of links ending in the current page, which is not a link: links in the nav link's
/// colour, underlined, the current page in primary text, quiet separators between them, and no
/// box or fill.
///
/// Tab reaches each link, and Enter or Space follows it. A path too long for its width truncates
/// its labels with `…`. With [`Breadcrumb::max_items`] the middle folds into a `…` that Tab also
/// reaches; Enter, Space or Down opens a menu of the hidden items, in which the arrow keys move and
/// Enter follows one.
pub struct Breadcrumb<T> {
    id: String,
    items: Vec<Crumb<T>>,
    separator: Glyph,
    max_items: Option<usize>,
}

impl<T: 'static> Breadcrumb<T> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            separator: Icon::ChevronRight.into(),
            max_items: None,
        }
    }

    /// Adds a step; `on_click` goes there. Ignored on the last item, the current page.
    pub fn item(mut self, label: impl Into<String>, on_click: impl Fn(&mut T) + 'static) -> Self {
        self.items.push(Crumb {
            label: label.into(),
            on_click: Some(Rc::new(on_click)),
        });
        self
    }

    /// Adds a step that is not a link, usually the current page.
    pub fn current(mut self, label: impl Into<String>) -> Self {
        self.items.push(Crumb {
            label: label.into(),
            on_click: None,
        });
        self
    }

    /// What stands between two items: ODK's chevron by default, as another icon or a character.
    pub fn separator(mut self, separator: impl Into<Glyph>) -> Self {
        self.separator = separator.into();
        self
    }

    /// The most items shown, at least 2; the middle ones fold into a `…` menu.
    pub fn max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max.max(2));
        self
    }

    /// What each part reports, in order: links, the current page, plain steps, and the folded
    /// middle as a button.
    pub fn item_accessibles(&self) -> Vec<Accessible> {
        let last = self.items.len().saturating_sub(1);
        let shown = visible(self.items.len(), self.max_items);
        let hidden = self.items.len() + 1 - shown.len();
        shown
            .into_iter()
            .map(|slot| match slot {
                None => Accessible::new(Role::Button, fold_name(hidden)),
                Some(i) if i == last => {
                    Accessible::new(Role::CurrentPage, self.items[i].label.clone())
                }
                Some(i) => {
                    let role = if self.items[i].on_click.is_some() {
                        Role::Link
                    } else {
                        Role::Text
                    };
                    Accessible::new(role, self.items[i].label.clone())
                }
            })
            .collect()
    }
}

/// The accessible name of the folded middle.
fn fold_name(hidden: usize) -> String {
    if hidden == 1 {
        "Show 1 hidden item".into()
    } else {
        format!("Show {hidden} hidden items")
    }
}

/// The items to show: an index, or `None` for the `…` that stands for the folded middle.
fn visible(len: usize, max: Option<usize>) -> Vec<Option<usize>> {
    match max {
        Some(max) if len > max => {
            // One item before the fold, the rest after it.
            let tail = max - 1;
            let mut shown = vec![Some(0), None];
            shown.extend((len - tail..len).map(Some));
            shown
        }
        _ => (0..len).map(Some).collect(),
    }
}

impl<T> Describe for Breadcrumb<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Navigation, "Breadcrumb")
    }
}

impl<T: 'static> Breadcrumb<T> {
    /// The path, with the folded middle's menu drawn into `layers` while it is open.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El {
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.nav_link(States::NONE);
        let current_page = paint
            .tokens
            .nav_link(States::NONE.selected(true))
            .current
            .foreground;
        let type_style = paint.role(s.type_role);
        let len = self.items.len();
        let last = len.saturating_sub(1);
        let shown = visible(len, self.max_items);
        let folded: Vec<usize> = (0..len).filter(|i| !shown.contains(&Some(*i))).collect();
        let mut crumbs: Vec<Option<Crumb<T>>> = self.items.into_iter().map(Some).collect();
        let mut path = apply(
            row().key(self.id.clone()),
            &Accessible::new(Role::Navigation, "Breadcrumb"),
        )
        .gap(s.gap)
        .center_y()
        .min_w(0.0);
        for (n, slot) in shown.into_iter().enumerate() {
            if n > 0 {
                // On the items' line, as tall as it.
                path = path.child(
                    row()
                        .key(format!("{}-sep-{n}", self.id))
                        .h(type_style.line_height)
                        .center_y()
                        .child(
                            self.separator
                                .clone()
                                .slot(type_style.size, ink(s.separator)),
                        ),
                );
            }
            let Some(i) = slot else {
                // The folded middle: a menu of the hidden items.
                let mut menu = Menu::<T>::new();
                for i in &folded {
                    let crumb = crumbs[*i].take().expect("each item is folded once");
                    let mut item = MenuItem::new(crumb.label);
                    if let Some(go) = crumb.on_click {
                        item = item.on_select(move |app: &mut T| go(app));
                    }
                    menu = menu.item(item);
                }
                let name = fold_name(folded.len());
                let more = format!("{}-more", self.id);
                below_trigger(hosted, &more, &name, menu, layers);
                path = path.child(fold_trigger::<T>(
                    &more,
                    &name,
                    s.colours.default.foreground,
                    &paint,
                    s.type_role,
                ));
                continue;
            };
            let crumb = crumbs[i].take().expect("each item is shown once");
            let key = format!("{}-item-{i}", self.id);
            let label = crumb.label;
            let go = crumb.on_click.filter(|_| i != last);
            let words = text(label.clone()).font(text_style(type_style));
            path = path.child(match go {
                Some(go) => flowed(
                    words
                        .color(ink(s.colours.default.foreground))
                        .link(Link::new(0..label.len(), key.clone()))
                        .on_link(move |hosted: &mut Hosted<T>, _: &str, _: &mut Tx| {
                            go(&mut hosted.app)
                        }),
                    Flow::End,
                )
                .key(key)
                .shrink(),
                // The current page in primary text; a step that is not a link in the text of a
                // link at rest.
                None => flowed(
                    words.color(ink(if i == last {
                        current_page
                    } else {
                        s.current.foreground
                    })),
                    Flow::End,
                )
                .key(key)
                .shrink(),
            });
        }
        path
    }
}

/// The folded middle's `…`: a link-looking button that opens the menu of the hidden items on a
/// press, Enter, Space or Down.
fn fold_trigger<T: 'static>(
    id: &str,
    name: &str,
    colour: odk_theme::Color,
    paint: &style::Paint,
    role: odk_theme::foundation::TypeRole,
) -> El {
    let key = id.to_string();
    let open: Toggle<T> = {
        let key = key.clone();
        Rc::new(move |hosted: &mut Hosted<T>| {
            let was = hosted.ui.is_open(&key, false);
            hosted.ui.set_open(&key, !was);
        })
    };
    // Down opens it too; the trigger takes Enter and Space, and a Down it does not take rises here.
    let down = {
        let key = key.clone();
        move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
            if ev.key == Key::Down && !hosted.ui.is_open(&key, false) {
                hosted.ui.set_open(&key, true);
                return true;
            }
            false
        }
    };
    let accessible = Accessible::new(Role::Button, name);
    let trigger = apply(
        text("…".to_string())
            .font(text_style(paint.role(role)))
            .color(ink(colour))
            .key(key.clone()),
        &accessible,
    )
    .focusable(true);
    row()
        .key(format!("{key}-wrap"))
        .child(pressable(trigger, &key, true, open))
        .on_key(down)
}
