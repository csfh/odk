//! A group of items on the Rune host (`docs/spec/odk-component-tokens.md`, `item group`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::separator::Separator;
use crate::style;
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ItemGroupVariant;
use rune::{El, col};

/// A titled group of items with a line between one item and the next: a title, an optional
/// description, and the items added with `child` under them.
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
/// follows the group. By default it has no box and no padding, so its text lines up with the
/// items' text and with the heading over it; `ItemGroupVariant::Outline` puts its title, its
/// description and its items inside a border with no fill. It takes no focus and no keys. It
/// reports a group named by its title, with its description.
pub struct ItemGroup {
    title: String,
    description: Option<String>,
    items: Vec<El>,
    variant: ItemGroupVariant,
}

impl ItemGroup {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            items: Vec::new(),
            variant: ItemGroupVariant::Default,
        }
    }

    /// How it is set off from the page: `Outline` draws it in a box, for a group that lists
    /// things, such as devices or accounts.
    pub fn variant(mut self, variant: ItemGroupVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Secondary text under the title that says what the group is for.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// An item, after the ones before it.
    pub fn child(mut self, item: El) -> Self {
        self.items.push(item);
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.item_group(self.variant);

        let mut group = apply(col(), &accessible)
            .w_full()
            .gap(s.gap)
            .padding(s.padding)
            .child(words(&paint, self.title, s.title_role, s.title, Flow::Wrap));
        if let Some(description) = self.description {
            group = group.child(words(
                &paint,
                description,
                s.description_role,
                s.description,
                Flow::Wrap,
            ));
        }
        // The items sit flush, each padded by its own density, with a line between two.
        let mut items = col().w_full();
        for (index, item) in self.items.into_iter().enumerate() {
            if index > 0 {
                items = items.child(Separator::horizontal().show(hosted));
            }
            items = items.child(item);
        }
        let group = group.child(items);
        match s.border {
            Some(edge) => group
                .radius(s.radius)
                .outline(s.border_width, 0.0, ink(edge)),
            None => group,
        }
    }
}

impl Describe for ItemGroup {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Group, self.title.clone());
        a.description = self.description.clone();
        a
    }
}
