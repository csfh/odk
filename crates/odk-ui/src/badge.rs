//! A short label in a small box, on component tokens: a Role at an Emphasis
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{BadgeAppearance, BadgeStyle};
use odk_theme::semantic::{self, Emphasis};

use crate::decoration::{self, Applied, FlowKind, TextDecoration};
use crate::icon::{self, Glyph};
use crate::layout::StyledExt;
use crate::stack::Row;
use crate::{Icon, Tone, components};

/// A short label, such as a count or a state, in a small box: as high as its text, as wide as its
/// label until the parent is narrower, then one line ending in `…` (ADR 0058), with the
/// application's corners. Any of the six Roles at any Emphasis, with or without an edge; every
/// look has the same border, so a change of status never changes its size. A status is said by
/// its label, and `marked` adds the Role's status mark so it reads without its colour, as in a
/// monochrome theme. A truncated label shows the full text in a tooltip on hover; a badge is not
/// a tab stop. Two badges with the same label share that measurement.
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    appearance: BadgeAppearance,
    icon: Option<Glyph>,
    marked: bool,
    applied: Applied,
}

impl Badge {
    /// A neutral, muted badge.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            appearance: BadgeAppearance::new(semantic::Role::Neutral, Emphasis::Muted),
            icon: None,
            marked: false,
            applied: Applied::default(),
        }
    }

    /// Attaches a text decoration. The default is one line ending in `…` when the parent is
    /// narrower than the label; [`crate::Wrap`] lets it wrap.
    pub fn decorate(mut self, decoration: impl TextDecoration) -> Self {
        decoration.apply_text(&mut self.applied);
        self
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

    /// An icon before the label, at the label's size: one of ODK's icons or a character.
    pub fn icon(mut self, icon: impl Into<Glyph>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Puts the Role's status icon (`Info`, `Success`, `Warning`, `Danger`) before the label, as
    /// an alert has, so the status reads without its colour. Accent and neutral have none.
    pub fn marked(mut self) -> Self {
        self.marked = true;
        self
    }

    /// Its look and the glyph before its label: an icon, else the status mark when `marked`.
    fn glyph(&self) -> Option<Glyph> {
        self.icon.clone().or_else(|| {
            self.marked
                .then(|| Icon::status(self.appearance.role).map(Glyph::Icon))
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s: BadgeStyle = components().badge(self.appearance);
        #[cfg(test)]
        crate::drawn::record(&format!("odk-badge/{}", self.label), s);
        let g = s.geometry;
        let flow = self.applied.flow(FlowKind::End);
        let id = ElementId::Name(SharedString::from(format!("odk-badge-{}", self.label)));
        let size = px(components().foundation().typography.style(g.type_role).size);
        let lines = decoration::flow_lines(flow).unwrap_or(1);
        let (was, probe) =
            decoration::truncation_probe(&id, self.label.clone(), size, lines, window, cx);
        let mut badge = Row::new()
            .debug_selector(|| "odk-badge".into())
            .max_w_full()
            .min_w_0()
            .control_geometry(&g)
            .treatment(&s.colours);
        if let Some(glyph) = self.glyph() {
            let ink = s.colours.foreground;
            badge = badge.child(
                icon::slot(glyph, g.icon_size, [ink; 3], None)
                    .debug_selector(|| "odk-badge/icon".into()),
            );
        }
        // As wide as its label, not stretched across a column, until the column is narrower.
        badge.style().align_self = Some(gpui::AlignSelf::Start);
        badge = badge.child(
            decoration::apply_flow(div().min_w_0(), flow)
                .child(self.label.clone())
                .child(probe),
        );
        decoration::overflow_tooltip(
            &id,
            self.label,
            was && flow != FlowKind::Wrap,
            decoration::ClipFit::Fill,
            false,
            badge,
        )
    }
}
