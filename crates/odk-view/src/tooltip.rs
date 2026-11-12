//! A short text that explains its trigger, on the Rune host
//! (`docs/spec/odk-overlays.md`, `Tooltip`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::hover::{Beside, hover};
use crate::ink::ink;
use crate::placement::{Align, Placement};
use crate::style;
use crate::text::{Flow, words};
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, col};

/// The widest a tooltip grows before its text wraps, in points at the base text size. ODK's own
/// (ADR 0035).
const MAX_WIDTH: f32 = 240.0;

/// A short text that explains its trigger, shown above it (below when there is no room), on the
/// tooltip's component token.
///
/// It opens after the pointer rests on the trigger for the foundation's `hover_open`; Escape
/// closes it. It stays open while the pointer is over it, and closes `hover_close` after the
/// pointer leaves both (WCAG 1.4.13). It never takes focus and adds no tab stop. It fades in over
/// Rune's own transition. A tooltip says what a control does; anything the user needs to finish a
/// task belongs in visible text or a popover.
///
/// Not carried over: it does not open when the trigger has keyboard focus, since Rune tells an
/// element nothing of its focus (`docs/checks.md`).
pub struct Tooltip {
    id: String,
    text: String,
    placement: Placement,
    align: Align,
}

impl Tooltip {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            placement: Placement::Top,
            align: Align::Start,
        }
    }

    /// The side of the trigger it prefers; above by default.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which edge of the trigger it lines up with; the start by default.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// `trigger` with its tooltip: the open tooltip is added to `layers`.
    pub fn wrap<T: 'static>(self, hosted: &Hosted<T>, trigger: El, layers: &mut Vec<El>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let s = paint.tokens.tooltip();
        let g = s.geometry;
        let panel = apply(col(), &accessible)
            .px(g.padding_x)
            .py(g.padding_y)
            .max_w(paint.px(MAX_WIDTH))
            .fill(ink(s.surface.background))
            .outline(g.border_width, 0.0, ink(s.surface.border))
            .radius(g.radius)
            .child(words(
                &paint,
                self.text,
                g.body,
                s.surface.foreground,
                Flow::Wrap,
            ));
        hover(
            hosted,
            &paint,
            Beside {
                id: &self.id,
                placement: self.placement,
                align: self.align,
                panel,
            },
            trigger,
            layers,
        )
    }
}

impl Describe for Tooltip {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Tooltip, self.text.clone())
    }
}
