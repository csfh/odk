//! Feedback, marks and text: badges, alerts, status marks and dots, progress bars, spinners,
//! skeletons, separators, keys, body text and headings (`docs/spec/odk-component-tokens.md`). A
//! status is never shown by colour alone: each status role has a mark of its own
//! ([`status_mark`]), and a status dot has a label.

use std::time::Duration;

use super::catalogue::{Entry, Measure, Summary, Template};
use super::geometry::{self, ControlGeometry, Shape, SurfaceGeometry};
use super::state::States;
use super::{ComponentTokens, TreatmentRef};
use crate::Color;
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{
    BorderToken as B, ColourRef as C, Emphasis, Part, Role, State, SurfaceToken as S,
    TextToken as T, Treatment,
};

/// What a badge looks like: a role at an emphasis, optionally with an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BadgeAppearance {
    pub role: Role,
    pub emphasis: Emphasis,
    pub outline: bool,
}

impl BadgeAppearance {
    pub const fn new(role: Role, emphasis: Emphasis) -> Self {
        Self {
            role,
            emphasis,
            outline: false,
        }
    }

    pub const fn outlined(mut self) -> Self {
        self.outline = true;
        self
    }
}

/// A badge: as large as its text, not a control's height, with the application's corners. Every
/// look has a border, transparent or not, so a change of look never changes its size. An icon
/// before the label is the text's size, `geometry.gap` from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadgeStyle {
    pub colours: Treatment,
    pub geometry: ControlGeometry,
}

/// An alert in one role and emphasis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlertStyle {
    /// Background its fill, foreground its title, border its edge.
    pub surface: Treatment,
    pub description: Color,
    /// The status mark beside the title, in the role's colour.
    pub mark: Color,
    pub geometry: SurfaceGeometry,
    /// The same alert inline in a dense place: tighter padding and gap.
    pub compact: SurfaceGeometry,
}

/// A status dot and its label, such as "● Online". The dot carries the role's colour and the
/// label says what it means, so the status never rests on the colour, which a monochrome theme
/// does not have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusDotStyle {
    /// The dot's fill, or its edge when it is hollow.
    pub colour: Color,
    /// The neutral role, something that is off or inactive, is a hollow ring; every other is a
    /// filled dot, so on and off differ in shape as well as colour.
    pub filled: bool,
    pub size: f32,
    /// A hollow dot's edge.
    pub border_width: f32,
    /// Round by nature (the user's brief of 2026-10-23).
    pub shape: Shape,
    /// Between the dot and its label.
    pub gap: f32,
    pub label: Color,
    pub label_role: TypeRole,
}

/// A determinate progress bar in one role: a bar on a track, with an optional label above it
/// and its value at the end of that line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressStyle {
    /// The whole length: the role's muted tint, the one fill its focus colour is sure to show on.
    pub track: Color,
    /// How far it has got: the role's focus colour, at least `ui_contrast` against the track.
    pub bar: Color,
    /// The thickness of the track and the bar.
    pub height: f32,
    pub radius: f32,
    pub label: Color,
    /// The value, such as `42%`, at the end of the label's line.
    pub value: Color,
    pub type_role: TypeRole,
    /// Between the label's line and the bar.
    pub gap: f32,
}

/// A skeleton: the shape of content still loading, in a quiet tint. Decoration, held to no
/// contrast. It stands still.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkeletonStyle {
    pub fill: Color,
    pub radius: f32,
    /// One line of body text, which a text skeleton's lines are spaced by.
    pub line_height: f32,
    /// A line's bar: as high as the body text's letters, centred in its line.
    pub bar_height: f32,
}

/// The colour and mark of a status shown inline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusStyle {
    pub colour: Color,
    /// `None` for accent and neutral, which are not statuses.
    pub mark: Option<&'static str>,
}

/// A spinner standing alone. Inside a control it takes the control's foreground instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinnerStyle {
    pub colour: Color,
    pub size: f32,
    /// Whether it stands still: reduced motion. It stays visible, so the work still reads as
    /// busy.
    pub still: bool,
    /// One turn: the foundation's `cycle`.
    pub turn: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorStyle {
    pub colour: Color,
    pub thickness: f32,
    /// How far an inset separator stops short of each end: a control's padding at the
    /// application's density, so it lines up with the text of the controls it divides.
    pub inset: f32,
}

/// A keyboard key, such as `Ctrl+K`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KbdStyle {
    pub colours: Treatment,
    pub geometry: ControlGeometry,
}

/// Body text or a heading: a type role in a text colour, with no box of its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProseStyle {
    pub colour: Color,
    pub role: TypeRole,
}

/// The type role a heading of `level` draws: 1 (and 0) `Title`, 2 `Heading`, 3 and below
/// `Subtitle`, so levels past 3 look alike while keeping the level they were given.
pub const fn heading_role(level: u8) -> TypeRole {
    match level {
        0 | 1 => TypeRole::Title,
        2 => TypeRole::Heading,
        _ => TypeRole::Subtitle,
    }
}

/// Body text's colour: primary, or secondary for text that says less than what is around it.
const fn prose(secondary: bool) -> C {
    if secondary {
        C::Text(T::Secondary)
    } else {
        C::Text(T::Primary)
    }
}

/// The levels the catalogue lists: one for each type role a heading draws.
const HEADING_LEVELS: [u8; 3] = [1, 2, 3];

/// The mark of a status, so it reads without its colour: `i`, `✓`, `!` and `×` for info,
/// success, warning and danger. ODK has no icon set yet, so marks are characters.
pub const fn status_mark(role: Role) -> Option<&'static str> {
    match role {
        Role::Info => Some("i"),
        Role::Success => Some("✓"),
        Role::Warning => Some("!"),
        Role::Danger => Some("×"),
        Role::Accent | Role::Neutral => None,
    }
}

/// A role's colour for text and marks on any surface.
pub(crate) const fn status_ink(role: Role) -> C {
    C::Treatment(role, Emphasis::Subtle, State::Default, Part::Foreground)
}

impl ComponentTokens {
    pub fn badge(&self, appearance: BadgeAppearance) -> BadgeStyle {
        BadgeStyle {
            colours: self.paint(badge(appearance)),
            geometry: badge_geometry(&self.foundation),
        }
    }

    /// An alert. The default emphasis for an alert is muted.
    pub fn alert(&self, role: Role, emphasis: Emphasis) -> AlertStyle {
        let (surface, description, mark) = alert(role, emphasis);
        AlertStyle {
            surface: self.paint(surface),
            description: self.colour(description),
            mark: self.colour(mark),
            geometry: alert_geometry(&self.foundation),
            compact: compact_alert_geometry(&self.foundation),
        }
    }

    pub fn status(&self, role: Role) -> StatusStyle {
        StatusStyle {
            colour: self.colour(status_ink(role)),
            mark: status_mark(role),
        }
    }

    /// A status dot of `role` with its label: accent for something active, neutral (a hollow
    /// ring) for something inactive, a status role for how something went.
    pub fn status_dot(&self, role: Role) -> StatusDotStyle {
        let f = &self.foundation;
        StatusDotStyle {
            colour: self.colour(status_dot(role)),
            filled: role != Role::Neutral,
            size: status_dot_size(f),
            border_width: f.geometry.border_width,
            shape: Shape::Circle,
            gap: f.spacing.s2,
            label: self.colour(STATUS_LABEL),
            label_role: TypeRole::Body,
        }
    }

    /// A progress bar in `role`. `states.disabled` greys it; nothing else applies.
    pub fn progress(&self, role: Role, states: States) -> ProgressStyle {
        let f = &self.foundation;
        let (track, bar, label, value) = progress(role, states.disabled);
        ProgressStyle {
            track: self.colour(track),
            bar: self.colour(bar),
            height: progress_height(f),
            radius: f.geometry.radius.default,
            label: self.colour(label),
            value: self.colour(value),
            type_role: TypeRole::Small,
            gap: f.layout.field_gap,
        }
    }

    pub fn skeleton(&self) -> SkeletonStyle {
        let f = &self.foundation;
        let body = f.typography.style(TypeRole::Body);
        SkeletonStyle {
            fill: self.colour(SKELETON),
            radius: f.geometry.radius.default,
            line_height: body.line_height,
            bar_height: body.size,
        }
    }

    pub fn spinner(&self, density: Density) -> SpinnerStyle {
        let f = &self.foundation;
        SpinnerStyle {
            colour: self.colour(C::Text(T::Secondary)),
            size: f.controls.for_density(density).icon_size,
            still: f.motion.reduced,
            turn: f.motion.cycle,
        }
    }

    /// A separator: decoration, never the edge of a control.
    pub fn separator(&self) -> SeparatorStyle {
        let f = &self.foundation;
        SeparatorStyle {
            colour: self.colour(C::Border(B::Subtle)),
            thickness: f.geometry.border_width,
            inset: f.controls.metrics().padding_x,
        }
    }

    pub fn kbd(&self) -> KbdStyle {
        KbdStyle {
            colours: self.paint(KBD),
            geometry: kbd_geometry(&self.foundation),
        }
    }

    /// Body text, in the secondary text colour when `secondary`.
    pub fn text(&self, secondary: bool) -> ProseStyle {
        ProseStyle {
            colour: self.colour(prose(secondary)),
            role: TypeRole::Body,
        }
    }

    /// A heading at `level` ([`heading_role`]), in the primary text colour.
    pub fn heading(&self, level: u8) -> ProseStyle {
        ProseStyle {
            colour: self.colour(prose(false)),
            role: heading_role(level),
        }
    }
}

const KBD: TreatmentRef = TreatmentRef::new(
    C::Surface(S::Sunken),
    C::Text(T::Primary),
    C::Border(B::Default),
);

/// A status dot's label: quieter than the text around it, since the dot draws the eye.
const STATUS_LABEL: C = C::Text(T::Secondary);

/// A skeleton's tint: the neutral muted fill, which a secondary button rests on.
const SKELETON: C = C::Treatment(
    Role::Neutral,
    Emphasis::Muted,
    State::Default,
    Part::Background,
);

/// A status dot's colour: the role's text colour, which reads on every surface, or tertiary
/// text for the hollow ring of something inactive.
const fn status_dot(role: Role) -> C {
    match role {
        Role::Neutral => C::Text(T::Tertiary),
        role => status_ink(role),
    }
}

/// A progress bar's track, bar, label and value colours. The bar is the role's focus colour on
/// the role's own muted tint, a pair the semantic layer guarantees at `ui_contrast` over every
/// surface; disabled, both are grey and the text is disabled.
const fn progress(role: Role, disabled: bool) -> (C, C, C, C) {
    if disabled {
        let track = C::Treatment(
            Role::Neutral,
            Emphasis::Muted,
            State::Disabled,
            Part::Background,
        );
        let off = C::Text(T::Disabled);
        return (track, off, off, off);
    }
    (
        C::Treatment(role, Emphasis::Muted, State::Default, Part::Background),
        C::Focus(role),
        C::Text(T::Primary),
        C::Text(T::Secondary),
    )
}

/// A status dot: the grid's 8 px step, the smallest circle that reads as one at scale 1.
fn status_dot_size(f: &FoundationTokens) -> f32 {
    f.spacing.s2
}

/// A progress bar: the grid's 4 px step, thick enough to read at a glance and thin beside text.
fn progress_height(f: &FoundationTokens) -> f32 {
    f.spacing.s1
}

pub(crate) fn badge(a: BadgeAppearance) -> TreatmentRef {
    let base = TreatmentRef::role(a.role, a.emphasis, State::Default);
    if !a.outline {
        return base;
    }
    base.with_border(match a.role {
        Role::Neutral => C::Border(B::Strong),
        role => C::Focus(role),
    })
}

/// An alert's surface, description and mark. Inside a fill every text is the fill's own
/// foreground; without one, the title and description are ordinary text and the edge and mark
/// carry the role.
pub(crate) fn alert(role: Role, emphasis: Emphasis) -> (TreatmentRef, C, C) {
    let fill = TreatmentRef::role(role, emphasis, State::Default);
    match emphasis {
        Emphasis::Solid | Emphasis::Muted => (fill, fill.foreground, fill.foreground),
        Emphasis::Subtle => (
            TreatmentRef::new(C::None, C::Text(T::Primary), C::Focus(role)),
            C::Text(T::Secondary),
            status_ink(role),
        ),
    }
}

/// A badge takes the application's corners, as every rectangle does (the brief for navigation
/// and feedback, 2026-10-23; until then `radius.small`).
fn badge_geometry(f: &FoundationTokens) -> ControlGeometry {
    ControlGeometry {
        radius: f.geometry.radius.default,
        ..geometry::inline(f, TypeRole::Small, f.spacing.s2)
    }
}

fn kbd_geometry(f: &FoundationTokens) -> ControlGeometry {
    geometry::inline(f, TypeRole::Mono, f.spacing.s1)
}

fn alert_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    SurfaceGeometry {
        radius: f.geometry.radius.default,
        ..geometry::surface(
            f,
            f.layout.panel_padding,
            f.layout.panel_padding,
            f.spacing.s3,
            TypeRole::BodyStrong,
            TypeRole::Body,
        )
    }
}

/// An alert in a dense place: the grid's 12 px across, 8 px down and between its parts.
fn compact_alert_geometry(f: &FoundationTokens) -> SurfaceGeometry {
    SurfaceGeometry {
        radius: f.geometry.radius.default,
        ..geometry::surface(
            f,
            f.spacing.s3,
            f.spacing.s2,
            f.spacing.s2,
            TypeRole::BodyStrong,
            TypeRole::Body,
        )
    }
}

fn inline_summary(g: ControlGeometry) -> Summary {
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

pub(crate) fn catalogue(out: &mut Vec<Entry>) {
    use Measure::{Edge, Text};
    for role in Role::ALL {
        for emphasis in Emphasis::ALL {
            for outline in [false, true] {
                let mut a = BadgeAppearance::new(role, emphasis);
                if outline {
                    a = a.outlined();
                }
                let name = if outline { "outline" } else { "plain" };
                let t = Template::new("badge", name, |f, _| inline_summary(badge_geometry(f)))
                    .role(role, Some(emphasis))
                    .fixed_size();
                let measures: &'static [Measure] = if outline { &[Text, Edge] } else { &[Text] };
                t.entry(out, "default", measures, badge(a));
            }
            let t = Template::new("alert", "", |f, _| {
                let g = alert_geometry(f);
                Summary {
                    height: None,
                    shape: Shape::Rect { radius: g.radius },
                    type_role: Some(g.body),
                }
            })
            .role(role, Some(emphasis))
            .fixed_size();
            let (surface, description, mark) = alert(role, emphasis);
            t.entry(out, "title", &[Text], surface.with_border(C::None));
            t.entry(
                out,
                "description",
                &[Text],
                surface.with_foreground(description).with_border(C::None),
            );
            t.entry(
                out,
                "mark",
                &[Text],
                surface.with_foreground(mark).with_border(C::None),
            );
            if emphasis == Emphasis::Subtle {
                t.entry(out, "edge", &[Edge], surface);
            }
        }
        Template::new("status", "", |f, _| Summary {
            height: None,
            shape: Shape::Rect {
                radius: f.geometry.radius.none,
            },
            type_role: None,
        })
        .role(role, None)
        .fixed_size()
        .entry(
            out,
            "default",
            &[Text],
            TreatmentRef::new(C::None, status_ink(role), C::None),
        );
        Template::new("status dot", "", |f, _| Summary {
            height: Some(status_dot_size(f)),
            shape: Shape::Circle,
            type_role: Some(TypeRole::Body),
        })
        .role(role, None)
        .fixed_size()
        .entry(
            out,
            "dot",
            &[Measure::Mark],
            TreatmentRef::new(C::None, status_dot(role), C::None),
        );
        let t = Template::new("progress", "", progress_summary)
            .role(role, None)
            .fixed_size();
        let (track, bar, _, _) = progress(role, false);
        t.entry(
            out,
            "bar",
            &[Measure::Mark],
            TreatmentRef::new(track, bar, C::None),
        );
    }
    Template::new("status dot", "label", |f, _| {
        prose_summary(f, TypeRole::Body)
    })
    .fixed_size()
    .entry(
        out,
        "default",
        &[Text],
        TreatmentRef::new(C::None, STATUS_LABEL, C::None),
    );
    let t = Template::new("progress", "text", progress_summary).fixed_size();
    let (_, _, label, value) = progress(Role::Accent, false);
    t.entry(
        out,
        "label",
        &[Text],
        TreatmentRef::new(C::None, label, C::None),
    );
    t.entry(
        out,
        "value",
        &[Text],
        TreatmentRef::new(C::None, value, C::None),
    );
    let (track, bar, _, _) = progress(Role::Accent, true);
    Template::new("progress", "", progress_summary)
        .fixed_size()
        .entry(out, "disabled", &[], TreatmentRef::new(track, bar, C::None));
    Template::new("skeleton", "", |f, _| Summary {
        height: None,
        shape: Shape::Rect {
            radius: f.geometry.radius.default,
        },
        type_role: Some(TypeRole::Body),
    })
    .fixed_size()
    .entry(
        out,
        "default",
        &[],
        TreatmentRef::new(SKELETON, C::None, C::None),
    );
    Template::new("spinner", "standalone", |f, d| Summary {
        height: Some(f.controls.for_density(d).icon_size),
        shape: Shape::Circle,
        type_role: None,
    })
    .entry(
        out,
        "default",
        &[Measure::Mark],
        TreatmentRef::new(C::None, C::Text(T::Secondary), C::None),
    );
    Template::new("separator", "", |f, _| Summary {
        height: Some(f.geometry.border_width),
        shape: Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: None,
    })
    .fixed_size()
    .entry(
        out,
        "default",
        &[],
        TreatmentRef::new(C::None, C::None, C::Border(B::Subtle)),
    );
    Template::new("kbd", "", |f, _| inline_summary(kbd_geometry(f)))
        .fixed_size()
        .entry(out, "default", &[Text], KBD);
    for secondary in [false, true] {
        let name = if secondary { "secondary" } else { "primary" };
        Template::new("text", name, |f, _| prose_summary(f, TypeRole::Body))
            .fixed_size()
            .entry(
                out,
                "default",
                &[Text],
                TreatmentRef::new(C::None, prose(secondary), C::None),
            );
    }
    for level in HEADING_LEVELS {
        let geometry: super::catalogue::Geometry = match heading_role(level) {
            TypeRole::Title => |f, _| prose_summary(f, TypeRole::Title),
            TypeRole::Heading => |f, _| prose_summary(f, TypeRole::Heading),
            _ => |f, _| prose_summary(f, TypeRole::Subtitle),
        };
        Template::new("heading", level.to_string(), geometry)
            .fixed_size()
            .entry(
                out,
                "default",
                &[Text],
                TreatmentRef::new(C::None, prose(false), C::None),
            );
    }
}

/// A progress bar's track: its thickness, with the application's corners.
fn progress_summary(f: &FoundationTokens, _: Density) -> Summary {
    Summary {
        height: Some(progress_height(f)),
        shape: Shape::Rect {
            radius: f.geometry.radius.default,
        },
        type_role: Some(TypeRole::Small),
    }
}

/// Text has no box: as high as its lines, square, in its type role.
fn prose_summary(f: &FoundationTokens, role: TypeRole) -> Summary {
    Summary {
        height: None,
        shape: Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: Some(role),
    }
}
