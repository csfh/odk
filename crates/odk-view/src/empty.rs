//! An empty state on the Rune host (`docs/spec/odk-component-tokens.md`, `empty`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Icon;
use crate::ink::ink;
use crate::style;
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use rune::{Align, El, col, el, row};

/// What a list or a page shows when there is nothing in it: an optional icon in a small muted
/// square, a title, an optional description, and the actions that lead out of it, centred in the
/// room it is given:
///
/// ```text
///            [ icon ]
///       No matching processes
///    Nothing matches that search.
///        [ action ] [ action ]
/// ```
///
/// Its words wrap at about thirty ems, so a description stays a short paragraph on a wide page.
/// It takes no focus or keys of its own; Tab reaches its actions. It reports a status named by
/// its title, with its description, so an assistive technology says it when a list empties.
pub struct Empty {
    title: String,
    description: Option<String>,
    icon: Option<Icon>,
    actions: Vec<El>,
}

impl Empty {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            icon: None,
            actions: Vec::new(),
        }
    }

    /// Secondary text under the title that says why it is empty or what to do.
    pub fn description(mut self, text: impl Into<String>) -> Self {
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
    pub fn action(mut self, action: El) -> Self {
        self.actions.push(action);
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.empty();

        let mut text = col()
            .items_center()
            .w_full()
            .max_w(s.measure)
            .gap(s.gap)
            .child(
                words(&paint, self.title, s.title_role, s.title, Flow::Wrap).align(Align::Center),
            );
        if let Some(description) = self.description {
            text = text.child(
                words(
                    &paint,
                    description,
                    s.description_role,
                    s.description,
                    Flow::Wrap,
                )
                .align(Align::Center),
            );
        }
        let media = self.icon.map(|icon| {
            el().size(s.media_size, s.media_size)
                .mb(s.media_gap - s.gap)
                .center()
                .radius(s.radius)
                .fill(ink(s.media))
                .child(icon.mark(s.icon_size, ink(s.icon)))
        });
        let actions = (!self.actions.is_empty()).then(|| {
            row()
                .flex_wrap()
                .center_y()
                .justify_center()
                .mt(s.actions_gap - s.gap)
                .gap(s.gap)
                .children(self.actions)
        });
        let mut empty = apply(col(), &accessible)
            .w_full()
            .items_center()
            .padding(s.padding)
            .gap(s.gap);
        if let Some(media) = media {
            empty = empty.child(media);
        }
        empty = empty.child(text);
        if let Some(actions) = actions {
            empty = empty.child(actions);
        }
        empty
    }
}

impl Describe for Empty {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Status, self.title.clone());
        a.description = self.description.clone();
        a
    }
}
