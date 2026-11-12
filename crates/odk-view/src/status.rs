//! A status indicator on the Rune host: a dot in a Role's colour with a label that says what it
//! means (`docs/spec/odk-component-tokens.md`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::layout::inline;
use crate::style;
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::semantic;
use rune::{El, el, row};

/// What a status indicator says about something: whether it is on, or how it went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// On, running, online: the accent, filled.
    Active,
    /// Off, stopped, offline: a hollow grey ring.
    Inactive,
    Success,
    Warning,
    Error,
    Info,
}

/// The Role each status is drawn in, kept here only.
impl From<Status> for semantic::Role {
    fn from(status: Status) -> Self {
        match status {
            Status::Active => Self::Accent,
            Status::Inactive => Self::Neutral,
            Status::Success => Self::Success,
            Status::Warning => Self::Warning,
            Status::Error => Self::Danger,
            Status::Info => Self::Info,
        }
    }
}

/// A small dot in its status's colour with a label beside it, such as "● Online". The label
/// always shows and says what the status is, so it never rests on the dot's colour; an inactive
/// status is also a hollow ring rather than a dot. Decoration and text: it takes no focus.
pub struct StatusIndicator {
    status: Status,
    label: String,
}

impl StatusIndicator {
    /// `label` is what the status means, in words: "Online", "Build failed", "Syncing".
    pub fn new(status: Status, label: impl Into<String>) -> Self {
        Self {
            status,
            label: label.into(),
        }
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.status_dot(self.status.into());
        let dot = el()
            .size(s.size, s.size)
            .radius(s.shape.corner_radius(s.size, s.size))
            .a11y_hidden();
        // A hollow dot's edge is drawn inside its box, so the ring keeps the dot's size.
        let dot = if s.filled {
            dot.fill(ink(s.colour))
        } else {
            dot.outline(s.border_width, -s.border_width, ink(s.colour))
        };
        inline(
            apply(row(), &accessible)
                .gap(s.gap)
                .center_y()
                .shrink()
                .child(dot)
                .child(words(&paint, self.label, s.label_role, s.label, Flow::End)),
        )
    }
}

impl Describe for StatusIndicator {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.label.clone())
    }
}
