//! A path of links ending in the current page, on the nav link's component tokens
//! (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;

use crate::focus::{child_id, part_name};
use crate::interaction::RingTarget;
use crate::layout::{StyledExt, h_flex, single_line, text_width};
use crate::link::link;
use crate::theme::colour;
use crate::{DropdownMenu, MenuItem, components};

type Handler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// How much faster the middle of a long path gives up width than its ends: enough that the
/// ends keep their full labels until the middle has given up nearly all of its width. ODK's
/// own.
const MIDDLE_SHRINK: f32 = 1000.0;

struct Crumb {
    label: SharedString,
    on_click: Option<Handler>,
}

/// A path of links ending in the current page, which is not a link: ancestors in secondary
/// text, primary and underlined under the pointer, the current page in primary text, quiet
/// separators between them, and no box or fill.
///
/// Tab reaches each link, and Enter or Space follows it. A path too long for its width
/// truncates its labels with `…`, the middle first. With [`Breadcrumb::max_items`] the middle
/// folds into a `…` that Tab also reaches; Enter, Space or Down opens a menu of the hidden
/// items, in which the arrow keys move and Enter follows one.
#[derive(IntoElement)]
pub struct Breadcrumb {
    id: ElementId,
    items: Vec<Crumb>,
    separator: SharedString,
    max_items: Option<usize>,
}

impl Breadcrumb {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            separator: "/".into(),
            max_items: None,
        }
    }

    /// Adds a step; `on_click` goes there. Ignored on the last item, the current page.
    pub fn item(
        mut self,
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.items.push(Crumb {
            label: label.into(),
            on_click: Some(Rc::new(on_click)),
        });
        self
    }

    /// Adds a step that is not a link, usually the current page.
    pub fn current(mut self, label: impl Into<SharedString>) -> Self {
        self.items.push(Crumb {
            label: label.into(),
            on_click: None,
        });
        self
    }

    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = separator.into();
        self
    }

    /// The most items shown, at least 2; the middle ones fold into a `…` menu.
    pub fn max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max.max(2));
        self
    }

    /// What each part reports, in order: links, the current page, plain steps, and the
    /// folded middle as a button.
    pub fn item_accessibles(&self) -> Vec<Accessible> {
        let last = self.items.len().saturating_sub(1);
        let shown = visible(self.items.len(), self.max_items);
        let hidden = self.items.len() + 1 - shown.len();
        shown
            .into_iter()
            .map(|slot| match slot {
                None => Accessible::new(Role::Button, fold_name(hidden)),
                Some(i) if i == last => {
                    Accessible::new(Role::CurrentPage, self.items[i].label.to_string())
                }
                Some(i) => {
                    let role = if self.items[i].on_click.is_some() {
                        Role::Link
                    } else {
                        Role::Text
                    };
                    Accessible::new(role, self.items[i].label.to_string())
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

impl Describe for Breadcrumb {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Navigation, "Breadcrumb")
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let s = c.nav_link(States::NONE);
        let current_page = c.nav_link(States::NONE.selected(true)).current.foreground;
        let f = c.foundation();
        let size = px(f.typography.style(s.type_role).size);
        let floor_cap = px(f.spacing.s6);
        let len = self.items.len();
        let last = len.saturating_sub(1);
        let shown = visible(len, self.max_items);
        let folded: Vec<usize> = (0..len).filter(|i| !shown.contains(&Some(*i))).collect();
        let mut items: Vec<Option<Crumb>> = self.items.into_iter().map(Some).collect();
        // A focused link's ring is painted from outside the row, which clips its labels.
        let ring = RingTarget::default();
        let mut row = h_flex()
            .debug_selector(|| part_name(&self.id, "row"))
            .flex_1()
            .gap(px(s.gap))
            .text_role(s.type_role)
            .min_w_0()
            // Clip sideways only: a row that clipped both ways could be squeezed to no height
            // at all by a column that is too short for its content.
            .overflow_x_hidden();
        for (n, slot) in shown.into_iter().enumerate() {
            if n > 0 {
                let part = part_name(&self.id, &format!("sep-{n}"));
                row = row.child(
                    div()
                        .debug_selector(|| part)
                        .flex_shrink_0()
                        .text_color(colour(s.separator))
                        .child(self.separator.clone()),
                );
            }
            let Some(i) = slot else {
                // The folded middle: a menu of the hidden items.
                let hidden: Vec<(SharedString, Option<Handler>)> = folded
                    .iter()
                    .map(|i| {
                        let crumb = items[*i].take().expect("each item is folded once");
                        (crumb.label, crumb.on_click)
                    })
                    .collect();
                let name = fold_name(hidden.len());
                row = row.child(div().flex_shrink_0().child(
                    DropdownMenu::link(child_id(&self.id, "more"), "…", name, ring.clone()).menu(
                        move |mut menu| {
                            for (label, on_click) in hidden {
                                let mut item = MenuItem::new(label);
                                if let Some(f) = on_click {
                                    item = item.on_select(move |w, cx| f(w, cx));
                                }
                                menu = menu.item(item);
                            }
                            menu
                        },
                    ),
                ));
                continue;
            };
            let crumb = items[i].take().expect("each item is shown once");
            let part = part_name(&self.id, &format!("item-{i}"));
            // Labels truncate rather than overflow; the middle gives up width first.
            let shrink = if i == 0 || i == last {
                1.0
            } else {
                MIDDLE_SHRINK
            };
            // Never narrower than room for a letter and its `…`, or than the label itself when
            // that is shorter, so a label shrinks to a stub that still says which step it is,
            // and never vanishes between two separators.
            let floor = text_width(window, &crumb.label, size).min(floor_cap);
            let mut cell = single_line(div().debug_selector(|| part).min_w(floor));
            cell.style().flex_shrink = Some(shrink);
            match crumb.on_click.filter(|_| i != last) {
                Some(handler) => {
                    let id = child_id(&self.id, format!("item-{i}"));
                    row = row.child(
                        cell.child(
                            single_line(link(&id, crumb.label, &ring, window, cx))
                                .on_click(move |_, w, cx| handler(w, cx)),
                        ),
                    );
                }
                None => {
                    // The current page in primary text; a step that is not a link in the
                    // secondary text of a link at rest.
                    let ink = if i == last {
                        current_page
                    } else {
                        s.current.foreground
                    };
                    row = row.child(cell.text_color(colour(ink)).child(crumb.label));
                }
            }
        }
        div()
            .debug_selector(|| part_name(&self.id, "breadcrumb"))
            .relative()
            .flex()
            .min_w_0()
            .child(row)
            .child(ring.ring(s.focus, f.geometry.radius.none, &self.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_the_middle_of_a_long_path() {
        assert_eq!(visible(3, None), [Some(0), Some(1), Some(2)]);
        assert_eq!(visible(3, Some(3)), [Some(0), Some(1), Some(2)]);
        assert_eq!(visible(6, Some(3)), [Some(0), None, Some(4), Some(5)]);
        assert_eq!(visible(6, Some(2)), [Some(0), None, Some(5)]);
    }
}
