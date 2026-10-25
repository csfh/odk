//! Body text and headings, on component tokens: each is a type role in a text colour
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};

use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ProseStyle;

use crate::components;
use crate::layout::StyledExt;
use crate::theme::colour;

/// Body text in the primary text colour.
#[derive(IntoElement)]
pub struct Text {
    content: SharedString,
    muted: bool,
    mono: bool,
    single_line: bool,
}

impl Text {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            muted: false,
            mono: false,
            single_line: false,
        }
    }

    /// Secondary text, for what says less than the text around it.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// In the monospace type role, for what is read as characters: a command line, a path, an
    /// identifier. The colours are the text's own.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Stays on one line and ends in `…` when it does not fit, in the room its parent gives it.
    /// A parent that lays children out in a row must let this one shrink (`flex_1` and
    /// `min_w_0` on the element around it), or the text takes the width it wants.
    pub fn single_line(mut self) -> Self {
        self.single_line = true;
        self
    }
}

impl Describe for Text {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.content.to_string())
    }
}

impl RenderOnce for Text {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let tokens = components();
        let style = if self.mono {
            tokens.text_mono(self.muted)
        } else {
            tokens.text(self.muted)
        };
        let line = prose(style, self.content);
        if self.single_line {
            crate::layout::single_line(line).min_w_0()
        } else {
            line
        }
    }
}

/// A heading. Level 1 is the largest.
#[derive(IntoElement)]
pub struct Heading {
    content: SharedString,
    level: u8,
}

impl Heading {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            level: 1,
        }
    }

    /// 1 is a title, 2 a heading, and 3 and higher a subtitle.
    pub fn level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }
}

impl Describe for Heading {
    /// Reports the level it was given, between 1 and 6, even where levels 3 and below draw
    /// alike, so the outline a screen reader sees keeps the application's structure.
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Heading, self.content.to_string()).level(self.level.clamp(1, 6))
    }
}

impl RenderOnce for Heading {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        prose(components().heading(self.level), self.content)
    }
}

/// `content` in its style's colour and type role.
fn prose(style: ProseStyle, content: SharedString) -> gpui::Div {
    #[cfg(test)]
    drawn::record(&content, style);
    div()
        .text_color(colour(style.colour))
        .text_role(style.role)
        .child(content)
}

/// What each text drew last, by its content, for the checklist's tests.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use odk_theme::component::ProseStyle;

    thread_local! {
        static DRAWN: RefCell<HashMap<String, ProseStyle>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(content: &str, style: ProseStyle) {
        DRAWN.with(|d| d.borrow_mut().insert(content.to_owned(), style));
    }

    pub(crate) fn last(content: &str) -> ProseStyle {
        DRAWN.with(|d| {
            *d.borrow()
                .get(content)
                .unwrap_or_else(|| panic!("no text {content:?} has been drawn"))
        })
    }
}
