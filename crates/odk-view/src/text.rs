//! Body text and headings on the Rune host: each is a type role in a text colour
//! (`docs/spec/odk-component-tokens.md`), and its flow says what it does when the room is short
//! (ADR 0058).

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, Paint, text_style};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ProseStyle;
use odk_theme::foundation::TypeRole;
use rune::{El, text};

/// What text does when its line is longer than its room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    /// Breaks onto the next line, inside a word only when the word alone is wider than the
    /// room, so it never overflows. A heading's and body text's way.
    #[default]
    Wrap,
    /// One line, cut with "…" at the end. A parent that lays its children in a row must let the
    /// text shrink, or the text takes the width it wants.
    End,
    /// One line, with "…" in the middle: a path, or a file name, whose ends matter.
    Middle,
    /// At most this many lines, "…" ending the last when more would follow.
    Clamp(usize),
}

/// `element` flowing as `flow`. Rune keeps the whole string for copying and for a screen
/// reader whatever is cut.
fn flowed(element: El, flow: Flow) -> El {
    match flow {
        Flow::Wrap => element.wrap_anywhere(),
        Flow::End => element.truncate(),
        Flow::Middle => element.truncate_middle(),
        Flow::Clamp(lines) => element.max_lines(lines),
    }
}

/// `content` in `role` and the style's colour, flowing as `flow`, described as `accessible`.
fn prose(
    paint: &Paint,
    style: ProseStyle,
    role: TypeRole,
    content: String,
    flow: Flow,
    accessible: &Accessible,
) -> El {
    apply(
        flowed(
            text(content)
                .font(text_style(paint.role(role)))
                .color(ink(style.colour)),
            flow,
        ),
        accessible,
    )
}

/// Body text in the primary text colour.
pub struct Text {
    content: String,
    muted: bool,
    mono: bool,
    role: Option<TypeRole>,
    flow: Flow,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            muted: false,
            mono: false,
            role: None,
            flow: Flow::Wrap,
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

    /// Draws in `role` instead of body. The colour stays the text colour.
    pub fn role(mut self, role: TypeRole) -> Self {
        self.role = Some(role);
        self
    }

    /// What the text does when its line is longer than its room. Wrap unless asked.
    pub fn flow(mut self, flow: Flow) -> Self {
        self.flow = flow;
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let style = if self.mono {
            paint.tokens.text_mono(self.muted)
        } else {
            paint.tokens.text(self.muted)
        };
        let role = self.role.unwrap_or(style.role);
        prose(&paint, style, role, self.content, self.flow, &accessible)
    }
}

impl Describe for Text {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.content.clone())
    }
}

/// A heading. Level 1 is the largest.
pub struct Heading {
    content: String,
    level: u8,
    flow: Flow,
}

impl Heading {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            level: 1,
            flow: Flow::Wrap,
        }
    }

    /// 1 is a title, 2 a heading, and 3 and higher a subtitle.
    pub fn level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    /// What the heading does when its line is longer than its room. Wrap unless asked.
    pub fn flow(mut self, flow: Flow) -> Self {
        self.flow = flow;
        self
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let style = paint.tokens.heading(self.level);
        prose(
            &paint,
            style,
            style.role,
            self.content,
            self.flow,
            &accessible,
        )
    }
}

impl Describe for Heading {
    /// Reports the level it was given, between 1 and 6, even where levels 3 and below draw
    /// alike, so the outline a screen reader sees keeps the application's structure.
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Heading, self.content.clone()).level(self.level.clamp(1, 6))
    }
}
