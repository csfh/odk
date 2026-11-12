//! The meaning of a notice, which picks its status colour (ADR 0083).

use crate::icon::Icon;
use odk_theme::semantic::Role;

/// What a notice means: `Info` (the default), `Success`, `Warning` or `Danger`. An `Alert` or a
/// `Toast` takes one with `variant`, and it picks the status colour and the icon that goes with
/// it, so the meaning never shows by colour alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Info,
    Success,
    Warning,
    Danger,
}

/// The semantic role of a tone (`docs/spec/odk-semantic-tokens.md`).
impl From<Tone> for Role {
    fn from(tone: Tone) -> Self {
        match tone {
            Tone::Info => Role::Info,
            Tone::Success => Role::Success,
            Tone::Warning => Role::Warning,
            Tone::Danger => Role::Danger,
        }
    }
}

impl Tone {
    /// The icon drawn in the notice's colour, so the tone never shows by colour alone: its
    /// role's status icon.
    pub fn icon(self) -> Icon {
        Icon::status(self.into()).expect("every tone is a status role")
    }
}
