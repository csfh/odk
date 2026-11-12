//! A short label in a small box on the Rune host: a Role at an Emphasis
//! (`docs/spec/odk-component-tokens.md`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::{Glyph, Icon};
use crate::ink::ink;
use crate::layout::inline;
use crate::style;
use crate::text::{Flow, words};
use crate::tone::Tone;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::BadgeAppearance;
use odk_theme::semantic::{self, Emphasis};
use rune::{El, row};

/// A short label, such as a count or a state, in a small box: as high as its text, as wide as its
/// label until the parent is narrower, then one line ending in `…` (ADR 0058), with the
/// application's corners. Any of the six Roles at any Emphasis, with or without an edge. A
/// status is said by its label, and `marked` adds the Role's status mark so it reads without its
/// colour. A badge is not a tab stop.
pub struct Badge {
    label: String,
    appearance: BadgeAppearance,
    icon: Option<Glyph>,
    marked: bool,
    flow: Flow,
}

impl Badge {
    /// A neutral, muted badge.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            appearance: BadgeAppearance::new(semantic::Role::Neutral, Emphasis::Muted),
            icon: None,
            marked: false,
            flow: Flow::End,
        }
    }

    /// What the label does when the parent is narrower than it: one line ending in `…` unless
    /// asked.
    pub fn flow(mut self, flow: Flow) -> Self {
        self.flow = flow;
        self
    }

    /// A status badge: the tone's Role, muted.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.appearance = BadgeAppearance::new(tone.into(), Emphasis::Muted);
        self
    }

    /// Any of the six Roles, keeping the emphasis and the edge.
    pub fn role(mut self, role: impl Into<semantic::Role>) -> Self {
        self.appearance.role = role.into();
        self
    }

    /// Solid, muted or subtle, keeping the Role.
    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.appearance.emphasis = emphasis;
        self
    }

    /// The whole look at once.
    pub fn appearance(mut self, appearance: BadgeAppearance) -> Self {
        self.appearance = appearance;
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

    /// Puts the Role's status icon (`Info`, `Success`, `Warning`, `Danger`) before the label, so
    /// the status reads without its colour. Accent and neutral have none.
    pub fn marked(mut self) -> Self {
        self.marked = true;
        self
    }

    /// The glyph before its label: an icon, else the status mark when `marked`.
    fn glyph(&self) -> Option<Glyph> {
        self.icon.clone().or_else(|| {
            self.marked
                .then(|| Icon::status(self.appearance.role).map(Glyph::Icon))
                .flatten()
        })
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.badge(self.appearance);
        let g = s.geometry;
        let mut badge = apply(row(), &accessible)
            .min_h(g.height)
            .px(g.padding_x)
            .gap(g.gap)
            .center_y()
            .radius(g.radius)
            .fill(ink(s.colours.background))
            .outline(g.border_width, 0.0, ink(s.colours.border))
            .shrink();
        if let Some(glyph) = self.glyph() {
            badge = badge.child(glyph.slot(g.icon_size, ink(s.colours.foreground)));
        }
        // As wide as its label, not stretched across a column, until the column is narrower.
        inline(badge.child(words(
            &paint,
            self.label,
            g.type_role,
            s.colours.foreground,
            self.flow,
        )))
    }
}

impl Describe for Badge {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Text, self.label.clone())
    }
}
