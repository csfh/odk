//! A status indicator, on component tokens: a dot in a Role's colour with a label that says what
//! it means (`docs/spec/odk-component-tokens.md`).

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::semantic;

use crate::components;
use crate::decoration::{self, ClipFit};
use crate::layout::{StyledExt, h_flex, truncate_end};
use crate::theme::colour;

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
/// always shows and says what the status is, so it never rests on the dot's colour, which a
/// monochrome theme does not have; an inactive status is also a hollow ring rather than a dot.
/// Decoration and text: it takes no focus. A truncated label shows the full text in a tooltip
/// on hover only; nothing is added to the tab order to carry it.
#[derive(IntoElement)]
pub struct StatusIndicator {
    status: Status,
    label: SharedString,
}

impl StatusIndicator {
    /// `label` is what the status means, in words: "Online", "Build failed", "Syncing".
    pub fn new(status: Status, label: impl Into<SharedString>) -> Self {
        Self {
            status,
            label: label.into(),
        }
    }
}

impl Describe for StatusIndicator {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.label.to_string())
    }
}

impl RenderOnce for StatusIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = components().status_dot(self.status.into());
        #[cfg(test)]
        crate::drawn::record(&format!("odk-status/{}", self.label), s);
        let dot_size = px(s.size);
        let mut dot = div()
            .debug_selector(|| "odk-status/dot".into())
            .flex_shrink_0()
            .size(dot_size)
            .rounded(px(s.shape.corner_radius(s.size, s.size)));
        dot = if s.filled {
            dot.bg(colour(s.colour))
        } else {
            dot.border(px(s.border_width))
                .border_color(colour(s.colour))
        };
        let id = ElementId::Name(SharedString::from(format!("odk-status-{}", self.label)));
        let text_size = px(components()
            .foundation()
            .typography
            .style(s.label_role)
            .size);
        let (was, probe) =
            decoration::truncation_probe(&id, self.label.clone(), text_size, 1, window, cx);
        // The label box stays the flex item that shrinks. The tooltip hugs the text inside it,
        // so the measure keeps seeing the box and not the full string.
        let text = decoration::overflow_tooltip(
            &id,
            self.label.clone(),
            was,
            ClipFit::Hug,
            false,
            div().child(self.label),
        );
        let mut row = h_flex()
            .debug_selector(|| "odk-status".into())
            .gap(px(s.gap))
            .min_w_0()
            .text_role(s.label_role)
            .text_color(colour(s.label))
            .child(dot)
            .child(truncate_end(div().min_w_0().child(text).child(probe)));
        // As wide as its label, not stretched across a column.
        row.style().align_self = Some(gpui::AlignSelf::Start);
        row.max_w_full()
    }
}
