//! A row of content on the Rune host (`docs/spec/odk-component-tokens.md`, `item`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::style;
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::foundation::Density;
use rune::{El, col, row};

/// A row of content: a title, an optional description and the lines added with `child` under
/// them, the `action`s at its end, and an optional `footer` across it under its words:
///
/// ```text
/// Title                          [ action ] [ action ]
/// Description
/// line
/// [ footer                                           ]
/// ```
///
/// Its title and description read as a form field's label and description. Each action keeps its
/// own width, and the words wrap in the room the actions leave (ADR 0058). An action can be
/// anything, a switch, a select, a button, a badge. It is padded above and below by its density
/// and not at its sides, so its text lines up with the heading over it. It is not a tab stop and
/// takes no keys: Tab reaches its actions. It reports a group named by its title, with its
/// description.
pub struct Item {
    title: String,
    description: Option<String>,
    lines: Vec<El>,
    actions: Vec<El>,
    footer: Option<El>,
    density: Option<Density>,
}

impl Item {
    pub fn new(title: impl Into<String>) -> Self {
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
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// A line under the description, such as a status or a path.
    pub fn child(mut self, line: El) -> Self {
        self.lines.push(line);
        self
    }

    /// Something at the item's end, after the actions before it: a control, a button, a badge.
    pub fn action(mut self, action: El) -> Self {
        self.actions.push(action);
        self
    }

    /// Something across the item under its words, as wide as the item: a control that needs the
    /// room, such as a number field or a line of text.
    pub fn footer(mut self, footer: El) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Its own density, in place of the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.item(paint.tokens.density(self.density));

        let mut text = col().grow(1.0).shrink().gap(s.line_gap).child(words(
            &paint,
            self.title,
            s.title_role,
            s.title,
            Flow::Wrap,
        ));
        if let Some(description) = self.description {
            text = text.child(words(
                &paint,
                description,
                s.description_role,
                s.description,
                Flow::Wrap,
            ));
        }
        // Centred across, as a Row is: an action keeps its own height beside the words.
        let mut line = row()
            .w_full()
            .center_y()
            .gap(s.gap)
            .child(text.children(self.lines));
        for action in self.actions {
            line = line.child(row().child(action));
        }
        let mut item = apply(col(), &accessible)
            .w_full()
            .py(s.padding_y)
            .gap(s.footer_gap)
            .child(line);
        if let Some(footer) = self.footer {
            item = item.child(col().w_full().child(footer));
        }
        item
    }
}

impl Describe for Item {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Group, self.title.clone());
        a.description = self.description.clone();
        a
    }
}
