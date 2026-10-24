//! A line between groups of content, on component tokens: decoration, never a control's edge
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{App, InteractiveElement, IntoElement, RenderOnce, Styled, Window, div, px};
use odk_a11y::{Accessible, Describe, Role};

use crate::components;
use crate::theme::colour;

/// Which way a separator runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Horizontal,
    Vertical,
}

/// A line between groups of content, one border width thick in the subtle border colour, quieter
/// than any control's edge. A horizontal one spans the width of its column, a vertical one the
/// height of its row. It is never squeezed thinner, even in a column too short for its content.
/// It is decoration: it takes no focus and no pointer.
#[derive(IntoElement)]
pub struct Separator {
    orientation: Orientation,
    inset: bool,
}

impl Separator {
    /// A line across a column, between rows of content.
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            inset: false,
        }
    }

    /// A line down a row, between items side by side.
    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
            inset: false,
        }
    }

    /// Stops short of each end by a control's padding at the application's density, so it lines
    /// up with the text of the controls it divides, as between the groups of a sidebar.
    pub fn inset(mut self) -> Self {
        self.inset = true;
        self
    }
}

impl Describe for Separator {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Separator, "")
    }
}

impl RenderOnce for Separator {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let s = components().separator();
        #[cfg(test)]
        crate::drawn::record("odk-separator", s);
        let (thickness, inset) = (px(s.thickness), px(if self.inset { s.inset } else { 0.0 }));
        // A box of fixed thickness, which flex layout would otherwise shrink.
        let mut line = div()
            .debug_selector(|| "odk-separator".into())
            .flex_shrink_0()
            .bg(colour(s.colour));
        line.style().align_self = Some(gpui::AlignSelf::Stretch);
        match self.orientation {
            Orientation::Horizontal => line.h(thickness).mx(inset),
            Orientation::Vertical => line.w(thickness).my(inset),
        }
    }
}
