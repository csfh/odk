//! The component layer of ADR 0040 (`docs/spec/odk-component-tokens.md`): the final style each
//! ODK component draws with, by variant, density and state.
//!
//! It joins the two layers below it and adds nothing of its own:
//!
//! - **Colours come only from the semantic layer.** A component's colour is a [`ColourRef`] into
//!   [`SemanticColours`], fixed by component, variant and state in this module's code, so it is
//!   always a colour the resolver made and checked, and it cannot depend on density or corner
//!   radius.
//! - **Geometry comes only from the foundation.** Every length follows from
//!   [`FoundationTokens`], so none depends on the theme.
//!
//! [`ComponentTokens`] holds one snapshot of each and answers lookups such as
//! [`ComponentTokens::button`]: a reference resolved and a few lengths added, with no colour
//! maths, no file and no allocation. [`validate`] checks every colour pair a component draws over
//! the surfaces it is drawn on; the theme manager runs it with the resolver, so a theme whose
//! component colours fail is a failed reload (ADR 0041) and nothing partial is published.
//! [`ComponentTokens::diagnostics`] lists every style with its sources, geometry and contrast.
//!
//! Every component draws these.
//!
//! [`ColourRef`]: crate::semantic::ColourRef

mod catalogue;
mod controls;
mod feedback;
mod geometry;
mod navigation;
mod overlay;
mod state;

use std::sync::Arc;

use crate::Color;
use crate::foundation::{Density, FoundationTokens};
use crate::semantic::{
    BorderToken, ColourRef, Emphasis, Part, ResolveOptions, Role, SemanticColours, State, Treatment,
};

pub use catalogue::{ContrastResult, Measure, StyleDiagnostic, validate};
pub use controls::{
    ButtonAppearance, ButtonLayout, ButtonStyle, ButtonVariant, CheckboxStyle, FIELD_INVALID_MARK,
    FieldStyle, FormFieldStyle, RadioStyle, SelectStyle, SwitchStyle,
};
pub use feedback::{
    AlertStyle, BadgeAppearance, BadgeStyle, KbdStyle, ProgressStyle, ProseStyle, SeparatorStyle,
    SkeletonStyle, SpinnerStyle, StatusDotStyle, StatusStyle, heading_role, status_mark,
};
pub use geometry::{
    CheckGeometry, ControlGeometry, FocusRing, RadioGeometry, Shape, SurfaceGeometry,
    SwitchGeometry,
};
pub use navigation::{
    LinkStyle, NavItemStyle, NavListStyle, NavLook, SegmentStyle, SegmentedStyle, TabStyle,
};
pub use overlay::{CommandStyle, MenuItemStyle, MenuStyle, OverlayStyle, ToastStyle};
pub use state::{Check, States};

/// The component tokens for one theme and one foundation configuration. Cheap to build and to
/// clone: it holds the two snapshots it reads, and the generation of the theme snapshot the
/// colours came from, so a consumer can tell it never mixes two themes.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentTokens {
    foundation: Arc<FoundationTokens>,
    semantic: Arc<SemanticColours>,
    generation: u64,
}

/// Checks the colours of every component against `semantic` with `options`, then joins them
/// with `foundation`. For tests and tools; an application's tokens are built from a theme
/// snapshot, whose colours were checked when it was made ([`ComponentTokens::new`]).
pub fn resolve(
    foundation: &FoundationTokens,
    semantic: &SemanticColours,
    options: &ResolveOptions,
) -> Result<ComponentTokens, ComponentError> {
    validate(semantic, options)?;
    Ok(ComponentTokens::new(
        Arc::new(foundation.clone()),
        Arc::new(semantic.clone()),
        0,
    ))
}

impl ComponentTokens {
    /// Joins `semantic`, checked already, with `foundation`. `generation` is the theme
    /// snapshot's. Nothing is computed.
    pub fn new(
        foundation: Arc<FoundationTokens>,
        semantic: Arc<SemanticColours>,
        generation: u64,
    ) -> Self {
        Self {
            foundation,
            semantic,
            generation,
        }
    }

    /// The same colours with other foundation tokens: a change of density, corner radius,
    /// motion or fonts. No colour is looked at.
    pub fn with_foundation(&self, foundation: Arc<FoundationTokens>) -> Self {
        Self {
            foundation,
            ..self.clone()
        }
    }

    /// The same foundation with another theme's colours.
    pub fn with_theme(&self, semantic: Arc<SemanticColours>, generation: u64) -> Self {
        Self {
            semantic,
            generation,
            ..self.clone()
        }
    }

    pub fn foundation(&self) -> &Arc<FoundationTokens> {
        &self.foundation
    }

    pub fn semantic(&self) -> &Arc<SemanticColours> {
        &self.semantic
    }

    /// The generation of the theme snapshot the colours came from.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// A control's density: its own if it has one, else the application's.
    pub fn density(&self, own: Option<Density>) -> Density {
        own.unwrap_or(self.foundation.controls.density)
    }

    fn colour(&self, reference: ColourRef) -> Color {
        self.semantic.colour(reference)
    }

    fn paint(&self, reference: TreatmentRef) -> Treatment {
        reference.resolve(&self.semantic)
    }

    /// The colours of a control under each pointer state, given its other flags, from the
    /// component's one mapping from a state to colours.
    fn interactive(
        &self,
        states: States,
        map: impl Fn(State) -> TreatmentRef,
    ) -> (Interactive<Treatment>, Treatment) {
        let colours = Interactive::by_state(states, |state| self.paint(map(state)));
        (colours, colours.get(states.state()))
    }

    /// The focus ring of a control whose ring is `colour`.
    fn ring(&self, colour: ColourRef) -> FocusRing {
        let g = &self.foundation.geometry;
        FocusRing {
            colour: self.colour(colour),
            width: g.focus_width,
            offset: g.focus_offset,
        }
    }
}

/// The three colours of a treatment, by reference. Resolved against the semantic colours only
/// when a component asks for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreatmentRef {
    pub background: ColourRef,
    pub foreground: ColourRef,
    pub border: ColourRef,
}

impl TreatmentRef {
    pub const fn new(background: ColourRef, foreground: ColourRef, border: ColourRef) -> Self {
        Self {
            background,
            foreground,
            border,
        }
    }

    /// A semantic treatment as it is: `roles.<role>.<emphasis>.<state>`.
    pub const fn role(role: Role, emphasis: Emphasis, state: State) -> Self {
        Self {
            background: ColourRef::Treatment(role, emphasis, state, Part::Background),
            foreground: ColourRef::Treatment(role, emphasis, state, Part::Foreground),
            border: ColourRef::Treatment(role, emphasis, state, Part::Border),
        }
    }

    pub const fn with_border(self, border: ColourRef) -> Self {
        Self { border, ..self }
    }

    pub const fn with_background(self, background: ColourRef) -> Self {
        Self { background, ..self }
    }

    pub const fn with_foreground(self, foreground: ColourRef) -> Self {
        Self { foreground, ..self }
    }

    pub fn resolve(&self, semantic: &SemanticColours) -> Treatment {
        Treatment {
            background: semantic.colour(self.background),
            foreground: semantic.colour(self.foreground),
            border: semantic.colour(self.border),
        }
    }
}

/// One value for each pointer state a GPUI element styles itself in: at rest, under `.hover()`
/// and under `.active()`. A disabled control has its disabled value in all three, and a loading
/// one its resting value, so wiring all three is always safe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interactive<T> {
    pub default: T,
    pub hover: T,
    pub pressed: T,
}

impl<T: Copy> Interactive<T> {
    /// The value under `state`. A disabled control's every value is its disabled one.
    pub fn get(&self, state: State) -> T {
        match state {
            State::Hover => self.hover,
            State::Pressed => self.pressed,
            State::Default | State::Disabled => self.default,
        }
    }

    /// `value` at each pointer state, given the other flags in `states`.
    fn by_state(states: States, value: impl Fn(State) -> T) -> Self {
        Self {
            default: value(states.at(State::Default)),
            hover: value(states.at(State::Hover)),
            pressed: value(states.at(State::Pressed)),
        }
    }
}

/// The focus colour of a control of `role`: a neutral control's is the accent's
/// (`border.focus`), any other's its role's own.
const fn focus_of(role: Role) -> ColourRef {
    match role {
        Role::Neutral => ColourRef::Border(BorderToken::Focus),
        role => ColourRef::Focus(role),
    }
}

/// A component's colour that does not reach the contrast it needs, over the surface named.
/// Nothing is published then.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error(
    "{component} {variant} {state}: {what} is {found:.2}:1 against {against}, below the {required}:1 it needs"
)]
pub struct ComponentError {
    pub component: &'static str,
    pub variant: String,
    pub state: &'static str,
    /// What was measured: text, a mark or an edge.
    pub what: &'static str,
    pub found: f32,
    pub required: f32,
    pub against: &'static str,
}
