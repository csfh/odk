use odk_theme::component::status_mark;
use odk_theme::semantic::Role;

/// The meaning of a notice, which picks its status colour.
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
    /// A one-character mark drawn in the notice's colour, so the tone never shows by colour
    /// alone: its role's status mark. ODK has no icon set yet, so the mark is text in the
    /// monospace face.
    pub(crate) fn mark(self) -> &'static str {
        status_mark(self.into()).expect("every tone is a status role")
    }
}
