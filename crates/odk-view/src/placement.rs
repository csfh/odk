//! Where an overlay goes beside its anchor (`docs/spec/odk-overlays.md`, "Placement").

/// Which side of its anchor an overlay prefers. Rune flips it to the other side when it does not
/// fit, and keeps it inside the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Placement {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Which edge of the anchor an overlay lines up with. Centred alignment is not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Align {
    #[default]
    Start,
    End,
}

impl Placement {
    pub(crate) fn side(self) -> rune::Side {
        match self {
            Self::Top => rune::Side::Top,
            Self::Bottom => rune::Side::Bottom,
            Self::Left => rune::Side::Left,
            Self::Right => rune::Side::Right,
        }
    }
}

impl Align {
    pub(crate) fn rune(self) -> rune::Align {
        match self {
            Self::Start => rune::Align::Start,
            Self::End => rune::Align::End,
        }
    }
}
