//! An empty state, on component tokens (`docs/spec/odk-component-tokens.md`, `empty`).

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};

use crate::components;
use crate::icon::{Icon, still};
use crate::layout::{StyledExt, h_flex, v_flex};
use crate::theme::colour;

/// What a list or a page shows when there is nothing in it, as shadcn/ui's `Empty`: an optional
/// icon in a small muted square, a title, an optional description, and the actions that lead
/// out of it, centred in the room it is given:
///
/// ```text
///            [ icon ]
///       No matching processes
///    Nothing matches that search.
///        [ action ] [ action ]
/// ```
///
/// Its words wrap at about thirty ems, so a description stays a short paragraph on a wide page.
/// It has no border and takes no focus or keys of its own; Tab reaches its actions. It reports a
/// status named by its title, with its description, so an assistive technology says it when a
/// list empties.
#[derive(IntoElement)]
pub struct Empty {
    title: SharedString,
    description: Option<SharedString>,
    icon: Option<Icon>,
    actions: Vec<AnyElement>,
}

impl Empty {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            icon: None,
            actions: Vec::new(),
        }
    }

    /// Secondary text under the title that says why it is empty or what to do.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// An icon over the title that says what kind of nothing this is: `Search` for no matches,
    /// `Info` while waiting.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A way out under the words, after the actions before it: a button that clears a filter or
    /// reads again.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl Describe for Empty {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Status, self.title.to_string());
        a.description = self.description.as_ref().map(ToString::to_string);
        a
    }
}

impl RenderOnce for Empty {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let s = components().empty();
        #[cfg(test)]
        crate::drawn::record(&format!("odk-empty/{}", self.title), s);
        let name = self.title.clone();
        let part = move |p: &'static str| {
            let name = name.clone();
            move || format!("odk-empty/{name}/{p}")
        };

        let mut words = v_flex()
            .items_center()
            .w_full()
            .max_w(px(s.measure))
            .gap(px(s.gap))
            .child(
                div()
                    .debug_selector(part("title"))
                    .text_center()
                    .text_role(s.title_role)
                    .text_color(colour(s.title))
                    .child(self.title.clone()),
            );
        if let Some(text) = self.description {
            words = words.child(
                div()
                    .debug_selector(part("description"))
                    .text_center()
                    .text_role(s.description_role)
                    .text_color(colour(s.description))
                    .child(text),
            );
        }
        let media = self.icon.map(|icon| {
            div()
                .debug_selector(part("media"))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(s.media_size))
                .mb(px(s.media_gap - s.gap))
                .rounded(px(s.radius))
                .bg(colour(s.media))
                .child(still(icon, s.icon_size, s.icon))
        });
        let actions = (!self.actions.is_empty()).then(|| {
            h_flex()
                .debug_selector(part("actions"))
                .flex_wrap()
                .justify_center()
                .mt(px(s.actions_gap - s.gap))
                .gap(px(s.gap))
                .children(self.actions)
        });
        let title = self.title;
        v_flex()
            .debug_selector(move || format!("odk-empty/{title}"))
            .w_full()
            .items_center()
            .p(px(s.padding))
            .gap(px(s.gap))
            .children(media)
            .child(words)
            .children(actions)
    }
}
