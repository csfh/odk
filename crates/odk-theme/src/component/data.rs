//! Data and layout: a resizable group's handle (ADR 0069).

use super::catalogue::{Entry, Measure, Summary, Template};
use super::geometry::{FocusRing, Shape};
use super::{ComponentTokens, States, TreatmentRef};
use crate::semantic::{BorderToken as B, ColourRef as C};

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
}
