//! Data and layout: a resizable group's handle (ADR 0069) and a data table (ADR 0070).

use super::catalogue::{Entry, Measure, Summary, Template};
use super::geometry::{self, ControlGeometry, FocusRing, Shape};
use super::{ComponentTokens, Interactive, States, TreatmentRef, navigation};
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{BorderToken as B, ColourRef as C, State, TextToken as T, Treatment};

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
}
