//! A short label in a small box, on component tokens: a Role at an Emphasis
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{BadgeAppearance, BadgeStyle, status_mark};
use odk_theme::semantic::{self, Emphasis};

use crate::layout::{StyledExt, h_flex};
use crate::theme::mono;
use crate::{Tone, components};

/// A short label, such as a count or a state, in a small box: as high as its text, as wide as its
/// label, never wrapped, with the application's corners. Any of the six Roles at any Emphasis,
/// with or without an edge; every look has the same border, so a change of status never changes
/// its size. A status is said by its label, and `marked` adds the Role's status mark so it reads
/// without its colour, as in a monochrome theme.
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    appearance: BadgeAppearance,
    icon: Option<SharedString>,
    marked: bool,
}

impl Badge {
    /// A neutral, muted badge.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            appearance: BadgeAppearance::new(semantic::Role::Neutral, Emphasis::Muted),
            icon: None,
            marked: false,
        }
    }

    /// A status badge: the tone's Role, muted.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.appearance = tone.into();
        self
    }

    /// Any of the six Roles, keeping the emphasis and the edge: `role(Role::Accent)` for
    /// something new, `role(Tone::Warning)` for a status.
    pub fn role(mut self, role: impl Into<semantic::Role>) -> Self {
        self.appearance.role = role.into();
        self
    }

    /// Solid, muted or subtle, keeping the Role. Quieter than a primary button at any of them.
    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.appearance.emphasis = emphasis;
        self
    }

    /// The whole look at once: `BadgeAppearance::new(role, emphasis)`, optionally `outlined()`.
    pub fn appearance(mut self, appearance: impl Into<BadgeAppearance>) -> Self {
        self.appearance = appearance.into();
        self
    }

    /// An edge in the Role's colour (strong grey for neutral), as for a badge with no fill.
    pub fn outlined(mut self) -> Self {
        self.appearance.outline = true;
        self
    }

    /// A glyph before the label, at the label's size. ODK has no icon set yet, so an icon is a
    /// character.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Puts the Role's status mark (`i`, `✓`, `!`, `×`) before the label, as an alert has, so the
    /// status reads without its colour. Accent and neutral have none.
    pub fn marked(mut self) -> Self {
        self.marked = true;
        self
    }

    /// Its look and the glyph before its label: an icon, else the status mark when `marked`.
    fn glyph(&self) -> Option<SharedString> {
        self.icon.clone().or_else(|| {
            self.marked
                .then(|| status_mark(self.appearance.role).map(SharedString::from))
                .flatten()
        })
    }
}

/// A tone is its Role, muted.
impl From<Tone> for BadgeAppearance {
    fn from(tone: Tone) -> Self {
        Self::new(tone.into(), Emphasis::Muted)
    }
}

impl Describe for Badge {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.label.to_string())
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let s: BadgeStyle = components().badge(self.appearance);
        #[cfg(test)]
        crate::drawn::record(&format!("odk-badge/{}", self.label), s);
        let g = s.geometry;
        let mut badge = h_flex()
            .debug_selector(|| "odk-badge".into())
            .flex_shrink_0()
            .whitespace_nowrap()
            .control_geometry(&g)
            .treatment(&s.colours);
        if let Some(glyph) = self.glyph() {
            let size = px(g.icon_size);
            badge = badge.child(
                div()
                    .debug_selector(|| "odk-badge/icon".into())
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .size(size)
                    .font_family(mono())
                    .text_size(size)
                    .line_height(size)
                    .child(glyph),
            );
        }
        // As wide as its label, not stretched across a column.
        badge.style().align_self = Some(gpui::AlignSelf::Start);
        badge.child(self.label)
    }
}
