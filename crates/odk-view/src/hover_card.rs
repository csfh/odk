//! A panel of content that opens beside its trigger on hover, on the Rune host
//! (`docs/spec/odk-overlays.md`, `HoverCard`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::hover::{Beside, hover};
use crate::ink::ink;
use crate::placement::{Align, Placement};
use crate::style;
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, col};

/// The widest a hover card grows, in points at the base text size. ODK's own.
const MAX_WIDTH: f32 = 320.0;

/// A panel of content, such as a person's card, that opens beside its trigger when the pointer
/// rests on it, on the popover's component token. It opens, stays and closes as a tooltip does,
/// takes no focus, and closes on Escape. Unlike a tooltip it may hold more than a line of text,
/// but nothing the user needs: it is not reachable by keyboard.
///
/// Not carried over: it does not open when the trigger has keyboard focus (`docs/checks.md`).
pub struct HoverCard {
    id: String,
    name: String,
    content: Option<El>,
    placement: Placement,
    align: Align,
}

impl HoverCard {
    /// A card named `name` for assistive technology, such as "Profile of Ada".
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            content: None,
            placement: Placement::Bottom,
            align: Align::Start,
        }
    }

    /// What the card shows.
    pub fn content(mut self, content: El) -> Self {
        self.content = Some(content);
        self
    }

    /// The side of the trigger it prefers; below by default.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which edge of the trigger it lines up with; the start by default.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// `trigger` with its card: the open card is added to `layers`.
    pub fn wrap<T: 'static>(self, hosted: &Hosted<T>, trigger: El, layers: &mut Vec<El>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.popover();
        let g = s.geometry;
        let mut panel = apply(col(), &accessible)
            .px(g.padding_x)
            .py(g.padding_y)
            .gap(g.gap)
            .max_w(paint.px(MAX_WIDTH))
            .fill(ink(s.surface.background))
            .outline(g.border_width, 0.0, ink(s.surface.border))
            .radius(g.radius);
        if let Some(content) = self.content {
            panel = panel.child(content);
        }
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

impl Describe for HoverCard {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Tooltip, self.name.clone())
    }
}
