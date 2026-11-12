//! A line between groups of content on the Rune host: decoration, never a control's edge
//! (`docs/spec/odk-component-tokens.md`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style;
use odk_a11y::{Accessible, Describe, Role};
use rune::{El, el};

/// Which way a separator runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Horizontal,
    Vertical,
}

/// A line between groups of content, one border width thick in the subtle border colour, quieter
/// than any control's edge. A horizontal one spans the width of its column, a vertical one the
/// height of its row. It takes no focus and no pointer.
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

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let line = style::paint(hosted).tokens.separator();
        let inset = if self.inset { line.inset } else { 0.0 };
        // A box of its thickness, stretched along the group it divides. Rune does not shrink it.
        let bar = apply(el(), &accessible)
            .self_stretch()
            .fill(ink(line.colour));
        match self.orientation {
            Orientation::Horizontal => bar.h(line.thickness).mx(inset),
            Orientation::Vertical => bar.w(line.thickness).my(inset),
        }
    }
}

impl Describe for Separator {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Separator, "")
    }
}
