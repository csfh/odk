//! A row of content, on component tokens (`docs/spec/odk-component-tokens.md`, `item`).

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::foundation::Density;

use crate::components;
use crate::layout::StyledExt;
use crate::stack::{Row, Stack};
use crate::theme::colour;

/// A row of content, as shadcn/ui's `Item`: a title, an optional description and the lines
/// added with `child` under them, the `action`s at its end, and an optional `footer` across it
/// under its words:
///
/// ```text
/// Title                          [ action ] [ action ]
/// Description
/// line
/// [ footer                                           ]
/// ```
///
/// Its title and description read as a `FormField`'s label and description. Each action keeps
/// its own width, and the words wrap in the room the actions leave (ADR 0058). An action can be
/// anything, a switch, a select, a button, a badge, so a settings page can choose a row's control
/// as it runs, which a `FormField`, holding only ODK's form controls, cannot. It is padded above
/// and below by its density and not at its sides, so its text lines up with the heading over it. It is not a tab stop and takes no keys:
/// Tab reaches its actions, and each works as it does anywhere. It reports a group named by its
/// title, with its description.
#[derive(IntoElement)]
pub struct Item {
    title: SharedString,
    description: Option<SharedString>,
    lines: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    footer: Option<AnyElement>,
    density: Option<Density>,
}

impl Item {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            lines: Vec::new(),
            actions: Vec::new(),
            footer: None,
            density: None,
        }
    }

    /// Secondary text under the title that says more about it.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Something at the item's end, after the actions before it: a control, a button, a badge.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Something across the item under its words, as wide as the item: a control that needs
    /// the room, such as a number field or a line of text.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Its own density, in place of the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }
}

/// What `child` adds: lines under the description, such as a status or a path.
impl ParentElement for Item {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.lines.extend(elements);
    }
}

impl Describe for Item {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Group, self.title.to_string());
        a.description = self.description.as_ref().map(ToString::to_string);
        a
    }
}

impl RenderOnce for Item {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let c = components();
        let s = c.item(c.density(self.density));
        #[cfg(test)]
        crate::drawn::record(&format!("odk-item/{}", self.title), s);
        let name = self.title.clone();
        let part = move |p: &str| format!("odk-item/{name}/{p}");

        let mut words = Stack::new()
            .debug_selector({
                let part = part.clone();
                move || part("words")
            })
            .flex_1()
            .min_w_0()
            .gap(px(s.line_gap))
            .child(
                div()
                    .debug_selector({
                        let part = part.clone();
                        move || part("title")
                    })
                    .min_w_0()
                    .text_role(s.title_role)
                    .text_color(colour(s.title))
                    .child(self.title.clone()),
            );
        if let Some(text) = self.description {
            words = words.child(
                div()
                    .debug_selector({
                        let part = part.clone();
                        move || part("description")
                    })
                    .min_w_0()
                    .text_role(s.description_role)
                    .text_color(colour(s.description))
                    .child(text),
            );
        }
        let mut row = Row::new()
            .w_full()
            .gap(px(s.gap))
            .child(words.children(self.lines));
        for action in self.actions {
            row = row.child(
                Row::new()
                    .debug_selector({
                        let part = part.clone();
                        move || part("action")
                    })
                    .child(action),
            );
        }
        let title = self.title;
        let mut item = Stack::new()
            .debug_selector(move || format!("odk-item/{title}"))
            .w_full()
            .min_w_0()
            .py(px(s.padding_y))
            .gap(px(s.footer_gap))
            .child(row);
        if let Some(footer) = self.footer {
            item = item.child(
                Stack::new()
                    .debug_selector(move || part("footer"))
                    .w_full()
                    .child(footer),
            );
        }
        item
    }
}
