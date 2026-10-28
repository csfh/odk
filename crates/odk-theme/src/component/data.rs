//! Data and layout: a resizable group's handle (ADR 0069), a data table (ADR 0070), a chart's
//! inks (ADR 0071) and an image (ADR 0072).

use super::catalogue::{Entry, Measure, Summary, Template};
use super::geometry::{self, ControlGeometry, FocusRing, Shape};
use super::{ComponentTokens, Interactive, States, TreatmentRef, navigation};
use crate::Color;
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{
    BorderToken as B, ColourRef as C, Emphasis, Part, Role, State, SurfaceToken as S,
    TextToken as T, Treatment,
};

/// The handle between two panels of a `Resizable`: a line one border wide, in the separator's
/// colour at rest and the focus colour, at the focus width, while it is hovered or dragged, with
/// a pointer target `target_min` across centred on it that takes no room (ADR 0069).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizeHandleStyle {
    /// The line as it is drawn now.
    pub line: crate::Color,
    /// How thick the line is drawn now: a border at rest, the focus width while active. The
    /// handle takes `thickness` in the layout whatever its state, so nothing moves.
    pub drawn: f32,
    /// The room the handle takes between the panels: one border.
    pub thickness: f32,
    /// How wide the pointer target is, centred on the line, over the panels' edges.
    pub target: f32,
    pub focus: FocusRing,
    pub focus_visible: bool,
}

impl ComponentTokens {
    /// A resizable group's handle. Hovered or pressed (a drag) it is active, and disabled it
    /// stays at rest.
    pub fn resize_handle(&self, states: States) -> ResizeHandleStyle {
        let g = &self.foundation.geometry;
        let active = !states.disabled && (states.hovered || states.pressed);
        ResizeHandleStyle {
            line: self.colour(handle_line(active)),
            drawn: if active {
                g.focus_width
            } else {
                g.border_width
            },
            thickness: g.border_width,
            target: g.target_min,
            focus: self.ring(C::Border(B::Focus)),
            focus_visible: states.shows_focus(),
        }
    }
}

/// The handle's line: the quiet separator colour at rest, the focus colour while active.
const fn handle_line(active: bool) -> C {
    if active {
        C::Border(B::Focus)
    } else {
        C::Border(B::Subtle)
    }
}

/// What a data table draws around its cells (`DataTable`, ADR 0070): rows a control's height at
/// its density, with its padding, a rule under the header, and the empty text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableStyle {
    /// A row's height, its cells' padding and gap, its type role, no border.
    pub geometry: ControlGeometry,
    /// The header's names: the small type role.
    pub header_role: TypeRole,
    /// The cells' text.
    pub text: crate::Color,
    /// The rule under the header: the default border, as wide as a border.
    pub rule: crate::Color,
    pub rule_width: f32,
    /// What an empty table says: secondary text.
    pub empty: crate::Color,
    /// The ring around the selected row while the rows have focus, and around a focused header.
    pub focus: FocusRing,
}

/// One header cell or one row of a data table, under the pointer and selected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableCellStyle {
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    pub responds: bool,
}

impl ComponentTokens {
    /// A data table at `density`.
    pub fn table(&self, density: Density) -> TableStyle {
        let f = &self.foundation;
        TableStyle {
            geometry: geometry::item(f, density, f.controls.for_density(density).height),
            header_role: TypeRole::Small,
            text: self.colour(C::Text(T::Primary)),
            rule: self.colour(TABLE_RULE),
            rule_width: f.geometry.border_width,
            empty: self.colour(TABLE_EMPTY),
            focus: self.ring(C::Border(B::Focus)),
        }
    }

    /// A header cell: secondary text, primary under the pointer when the column sorts.
    pub fn table_header(&self, sortable: bool, states: States) -> TableCellStyle {
        let states = if sortable {
            states
        } else {
            states.disabled(false)
        };
        let (colours, current) = self.interactive(states, |s| table_header(sortable, s));
        TableCellStyle {
            colours,
            current,
            responds: sortable && states.responds(),
        }
    }

    /// A row: no fill, a tint under the pointer, the selected fill when selected (as a
    /// segment's, so a row under the pointer never passes for the selected one).
    pub fn table_row(&self, states: States) -> TableCellStyle {
        let selected = states.selected;
        let (colours, current) = self.interactive(states, |s| navigation::segment(selected, s));
        TableCellStyle {
            colours,
            current,
            responds: states.responds(),
        }
    }
}

/// The rule under a table's header.
const TABLE_RULE: C = C::Border(B::Default);
/// What an empty table says.
const TABLE_EMPTY: C = C::Text(T::Secondary);

/// A header cell's label: a column that sorts reads as a link does, secondary and primary under
/// the pointer; one that does not stays secondary.
fn table_header(sortable: bool, state: State) -> TreatmentRef {
    if sortable {
        navigation::link(false, state)
    } else {
        TreatmentRef::new(C::None, C::Text(T::Secondary), C::None)
    }
}

fn table_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = geometry::item(f, d, f.controls.for_density(d).height);
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

/// The Roles a chart's series take, in order: the accent first, then the statuses, then the
/// neutral grey. A seventh series takes the first's colour again.
pub const SERIES_ROLES: [Role; 6] = [
    Role::Accent,
    Role::Info,
    Role::Success,
    Role::Warning,
    Role::Danger,
    Role::Neutral,
];

/// How many steps a chart's sequential ramp has.
pub const HEAT_STEPS: usize = 5;

/// What a chart draws with (`Chart`, ADR 0071): every colour an application can name, the
/// widths of its strokes and its text. Resolved once per theme, so drawing a chart does no
/// colour maths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartStyle {
    /// Each series' line and mark, by [`SERIES_ROLES`]: the Role's focus colour, which reaches
    /// `ui_contrast` over every surface.
    pub series: [Color; 6],
    /// Each series' tint, for an area or a bar: the Role's muted fill.
    pub series_fill: [Color; 6],
    /// A sequential ramp, low to high: the neutral muted fill, the accent's muted fill, its
    /// muted fill under the pointer, its solid fill and its focus colour.
    pub heat: [Color; HEAT_STEPS],
    pub text: Color,
    pub text_muted: Color,
    /// Grid lines: the subtle border. The axis: the default border.
    pub grid: Color,
    pub axis: Color,
    /// The surface the chart is on, for a gap between marks or a halo behind text.
    pub canvas: Color,
    /// Stroke widths: a border, the focus width and twice that.
    pub thin: f32,
    pub regular: f32,
    pub thick: f32,
    /// The heights a chart can ask for (ADR 0075): compact (`s8`), regular (twice `s12`) and
    /// tall (four times `s12`), so they grow with the text size as the spacing does.
    pub heights: [f32; 3],
    pub type_role: TypeRole,
    pub focus: FocusRing,
}

impl ComponentTokens {
    pub fn chart(&self) -> ChartStyle {
        let g = &self.foundation.geometry;
        let s = &self.foundation.spacing;
        ChartStyle {
            series: SERIES_ROLES.map(|r| self.colour(series_line(r))),
            series_fill: SERIES_ROLES.map(|r| self.colour(series_fill(r))),
            heat: HEAT.map(|c| self.colour(c)),
            text: self.colour(C::Text(T::Primary)),
            text_muted: self.colour(C::Text(T::Secondary)),
            grid: self.colour(C::Border(B::Subtle)),
            axis: self.colour(C::Border(B::Default)),
            canvas: self.colour(C::Surface(S::Canvas)),
            thin: g.border_width,
            regular: g.focus_width,
            thick: g.focus_width * 2.0,
            heights: [s.s8, s.s12 * 2.0, s.s12 * 4.0],
            type_role: TypeRole::Small,
            focus: self.ring(C::Border(B::Focus)),
        }
    }
}

/// A series' line: its Role's focus colour.
const fn series_line(role: Role) -> C {
    C::Focus(role)
}

/// A series' tint: its Role's muted fill.
const fn series_fill(role: Role) -> C {
    C::Treatment(role, Emphasis::Muted, State::Default, Part::Background)
}

/// The sequential ramp, low to high.
const HEAT: [C; HEAT_STEPS] = [
    C::Treatment(
        Role::Neutral,
        Emphasis::Muted,
        State::Default,
        Part::Background,
    ),
    C::Treatment(
        Role::Accent,
        Emphasis::Muted,
        State::Default,
        Part::Background,
    ),
    C::Treatment(
        Role::Accent,
        Emphasis::Muted,
        State::Hover,
        Part::Background,
    ),
    C::Treatment(
        Role::Accent,
        Emphasis::Solid,
        State::Default,
        Part::Background,
    ),
    C::Focus(Role::Accent),
];

/// An image (`Image`, ADR 0072): the application's corners, an avatar's size and shape, and the
/// placeholder it draws before its bytes arrive or when they are not an image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageStyle {
    pub radius: f32,
    /// An avatar's width and height: the foundation's `s12`, which grows with the text size.
    pub avatar: f32,
    pub avatar_shape: Shape,
    /// A skeleton's quiet fill.
    pub placeholder: Color,
    /// The mark on a placeholder for bytes that are not an image: tertiary text.
    pub mark: Color,
    pub type_role: TypeRole,
}

impl ComponentTokens {
    pub fn image(&self) -> ImageStyle {
        let f = &self.foundation;
        ImageStyle {
            radius: f.geometry.radius.default,
            avatar: f.spacing.s12,
            avatar_shape: Shape::Circle,
            placeholder: self.colour(IMAGE_PLACEHOLDER),
            mark: self.colour(C::Text(T::Tertiary)),
            type_role: TypeRole::Small,
        }
    }
}

/// An image's placeholder: the skeleton's fill, the neutral muted tint.
const IMAGE_PLACEHOLDER: C = C::Treatment(
    Role::Neutral,
    Emphasis::Muted,
    State::Default,
    Part::Background,
);

pub(super) fn catalogue(out: &mut Vec<Entry>) {
    let t = Template::new("resize handle", "", |f, _| Summary {
        height: Some(f.geometry.border_width),
        shape: Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: None,
    })
    .fixed_size();
    // At rest it is a separator: decoration, held to no contrast.
    t.entry(
        out,
        "default",
        &[],
        TreatmentRef::new(C::None, C::None, handle_line(false)),
    );
    t.entry(
        out,
        "active",
        &[Measure::Mark],
        TreatmentRef::new(C::None, handle_line(true), C::None),
    );
    t.focus(out, C::Border(B::Focus));
    for selected in [false, true] {
        let variant = if selected { "selected" } else { "" };
        Template::new("table row", variant, table_summary)
            .states(out, &[Measure::Text], |s| navigation::segment(selected, s));
    }
    Template::new("table header", "sortable", table_summary)
        .states(out, &[Measure::Text], |s| table_header(true, s));
    let header = Template::new("table header", "", table_summary);
    header.entry(
        out,
        "default",
        &[Measure::Text],
        table_header(false, State::Default),
    );
    header.focus(out, C::Border(B::Focus));
    let table = Template::new("table", "", table_summary);
    table.entry(
        out,
        "rule",
        &[],
        TreatmentRef::new(C::None, C::None, TABLE_RULE),
    );
    table.entry(
        out,
        "empty",
        &[Measure::Text],
        TreatmentRef::new(C::None, TABLE_EMPTY, C::None),
    );
    let series = Template::new("chart", "series", |f, _| Summary {
        height: None,
        shape: Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: Some(TypeRole::Small),
    })
    .fixed_size();
    for role in SERIES_ROLES {
        series.clone().role(role, None).entry(
            out,
            "line",
            &[Measure::Mark],
            TreatmentRef::new(C::None, series_line(role), C::None),
        );
        series.clone().role(role, None).entry(
            out,
            "fill",
            &[],
            TreatmentRef::new(series_fill(role), C::None, C::None),
        );
    }
    let text = Template::new("chart", "text", |f, _| Summary {
        height: None,
        shape: Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: Some(TypeRole::Small),
    })
    .fixed_size();
    text.entry(
        out,
        "label",
        &[Measure::Text],
        TreatmentRef::new(C::None, C::Text(T::Primary), C::None),
    );
    text.entry(
        out,
        "muted",
        &[Measure::Text],
        TreatmentRef::new(C::None, C::Text(T::Secondary), C::None),
    );
    for (n, heat) in HEAT.iter().enumerate() {
        let state: &'static str = ["heat 0", "heat 1", "heat 2", "heat 3", "heat 4"][n];
        text.entry(out, state, &[], TreatmentRef::new(*heat, C::None, C::None));
    }
    let image = Template::new("image", "", |f, _| Summary {
        height: None,
        shape: Shape::Rect {
            radius: f.geometry.radius.default,
        },
        type_role: Some(TypeRole::Small),
    })
    .fixed_size();
    image.entry(
        out,
        "placeholder",
        &[Measure::Text],
        TreatmentRef::new(IMAGE_PLACEHOLDER, C::Text(T::Tertiary), C::None),
    );
}
