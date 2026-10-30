//! A group of items, on component tokens (`docs/spec/odk-component-tokens.md`, `item group`).

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};

use crate::components;
use crate::layout::{StyledExt, v_flex};
use crate::separator::Separator;
use crate::theme::colour;

/// A titled group of `Item`s, as shadcn/ui's `ItemGroup` with an `ItemSeparator` between one item
/// and the next: a title, an optional description, and the items added with `child` under them.
///
/// ```text
/// Title
/// Description
/// Item
/// ────────────────
/// Item
/// ```
///
/// Its title reads as a level 2 heading and its description as secondary body text. A line
/// divides one item from the next, never the first from the description or the last from what
/// follows the group. It has no box and no padding, so its text lines up with the items' text and
/// with the heading over it, and it takes no focus and no keys. It reports a group named by its
/// title, with its description.
#[derive(IntoElement)]
pub struct ItemGroup {
    title: SharedString,
    description: Option<SharedString>,
    items: Vec<AnyElement>,
}

impl ItemGroup {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            items: Vec::new(),
        }
    }

    /// Secondary text under the title that says what the group is for.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }
}

/// What `child` adds: an item, after the ones before it.
impl ParentElement for ItemGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.items.extend(elements);
    }
}

impl Describe for ItemGroup {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Group, self.title.to_string());
        a.description = self.description.as_ref().map(ToString::to_string);
        a
    }
}

impl RenderOnce for ItemGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let s = components().item_group();
        #[cfg(test)]
        crate::drawn::record(&format!("odk-item-group/{}", self.title), s);
        let name = self.title.clone();
        let part = move |p: &str| format!("odk-item-group/{name}/{p}");

        let mut group = v_flex()
            .debug_selector({
                let title = self.title.clone();
                move || format!("odk-item-group/{title}")
            })
            .w_full()
            .gap(px(s.gap))
            .child(
                div()
                    .debug_selector({
                        let part = part.clone();
                        move || part("title")
                    })
                    .text_role(s.title_role)
                    .text_color(colour(s.title))
                    .child(self.title),
            );
        if let Some(text) = self.description {
            group = group.child(
                div()
                    .debug_selector({
                        let part = part.clone();
                        move || part("description")
                    })
                    .text_role(s.description_role)
                    .text_color(colour(s.description))
                    .child(text),
            );
        }
        // The items sit flush, each padded by its own density, with a line between two.
        let mut items = v_flex().w_full();
        for (i, item) in self.items.into_iter().enumerate() {
            if i > 0 {
                items = items.child(
                    div()
                        .debug_selector({
                            let part = part.clone();
                            move || part(&format!("separator-{i}"))
                        })
                        .child(Separator::horizontal()),
                );
            }
            items = items.child(item);
        }
        group.child(items)
    }
}
