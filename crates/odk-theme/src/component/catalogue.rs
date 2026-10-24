//! Every style the components draw, by reference, with what each must reach: the list
//! [`validate`] checks and [`ComponentTokens::diagnostics`] reports. Each component module adds
//! its own entries from the same mapping its lookups use, so what is checked is what is drawn.

use std::fmt;

use super::geometry::Shape;
use super::{
    ComponentError, ComponentTokens, TreatmentRef, controls, feedback, navigation, overlay,
};
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{
    ColourRef, Emphasis, ResolveOptions, Role, SemanticColours, State, SurfaceToken, Treatment,
};
use crate::{Color, drawn_contrast};

/// What a colour pair must reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Measure {
    /// The foreground is text on the background: `text_contrast`.
    Text,
    /// The foreground is a mark that shows a state, such as a switch's thumb, a tab's indicator
    /// or a focus ring: `ui_contrast`.
    Mark,
    /// The border shows where the control is, against the surface around it: `ui_contrast`.
    Edge,
}

impl Measure {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Mark => "mark",
            Self::Edge => "edge",
        }
    }

    fn required(self, options: &ResolveOptions) -> f32 {
        match self {
            Self::Text => options.text_contrast,
            Self::Mark | Self::Edge => options.ui_contrast,
        }
    }
}

/// The surfaces a style may be drawn on, which it is measured over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grounds {
    /// Any of them: a control can be placed on the canvas, a panel or an overlay.
    Surfaces,
    /// Only inside an overlay: menu items, a toast's or a tooltip's text.
    Overlay,
}

impl Grounds {
    fn tokens(self) -> &'static [SurfaceToken] {
        use SurfaceToken as S;
        match self {
            Self::Surfaces => &[S::Canvas, S::Recessed, S::Sunken, S::Raised, S::Overlay],
            Self::Overlay => &[S::Overlay],
        }
    }
}

/// The shape of a style at one density, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Summary {
    pub height: Option<f32>,
    pub shape: Shape,
    pub type_role: Option<TypeRole>,
}

/// What a style's geometry is at a density.
pub(crate) type Geometry = fn(&FoundationTokens, Density) -> Summary;

/// One style, by reference.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub template: Template,
    pub state: &'static str,
    pub colours: TreatmentRef,
    pub measures: &'static [Measure],
}

/// What the entries of one component variant share.
#[derive(Debug, Clone)]
pub(crate) struct Template {
    pub component: &'static str,
    pub variant: String,
    pub role: Option<Role>,
    pub emphasis: Option<Emphasis>,
    pub grounds: Grounds,
    pub geometry: Geometry,
    /// Whether the geometry changes with density, so diagnostics give a row for each.
    pub follows_density: bool,
}

impl Template {
    pub fn new(component: &'static str, variant: impl Into<String>, geometry: Geometry) -> Self {
        Self {
            component,
            variant: variant.into(),
            role: None,
            emphasis: None,
            grounds: Grounds::Surfaces,
            geometry,
            follows_density: true,
        }
    }

    pub fn role(mut self, role: Role, emphasis: Option<Emphasis>) -> Self {
        self.role = Some(role);
        self.emphasis = emphasis;
        self
    }

    pub fn in_overlay(mut self) -> Self {
        self.grounds = Grounds::Overlay;
        self
    }

    pub fn fixed_size(mut self) -> Self {
        self.follows_density = false;
        self
    }

    /// One entry.
    pub fn entry(
        &self,
        out: &mut Vec<Entry>,
        state: &'static str,
        measures: &'static [Measure],
        colours: TreatmentRef,
    ) {
        out.push(Entry {
            template: self.clone(),
            state,
            colours,
            measures,
        });
    }

    /// The default, hover and pressed entries, which must reach `measures`, and the disabled
    /// one, which WCAG does not hold to a minimum.
    pub fn states(
        &self,
        out: &mut Vec<Entry>,
        measures: &'static [Measure],
        map: impl Fn(State) -> TreatmentRef,
    ) {
        for state in [State::Default, State::Hover, State::Pressed] {
            self.entry(out, state.name(), measures, map(state));
        }
        self.entry(out, State::Disabled.name(), &[], map(State::Disabled));
    }

    /// The focus ring, which is drawn outside the control, on the surface around it.
    pub fn focus(&self, out: &mut Vec<Entry>, colour: ColourRef) {
        let mut on_surfaces = self.clone();
        on_surfaces.grounds = Grounds::Surfaces;
        on_surfaces.entry(
            out,
            "focus",
            &[Measure::Mark],
            TreatmentRef::new(ColourRef::None, colour, ColourRef::None),
        );
    }
}

/// Every style of every component, in a fixed order.
pub(crate) fn catalogue() -> Vec<Entry> {
    let mut out = Vec::new();
    controls::catalogue(&mut out);
    navigation::catalogue(&mut out);
    overlay::catalogue(&mut out);
    feedback::catalogue(&mut out);
    out
}

/// The least contrast of one measure of `colours` over `grounds`, and the surface it is least
/// on.
fn measure(
    semantic: &SemanticColours,
    measure: Measure,
    colours: &Treatment,
    grounds: Grounds,
) -> (f32, SurfaceToken) {
    grounds
        .tokens()
        .iter()
        .map(|&token| {
            let ground = semantic.surface.get(token);
            let found = match measure {
                Measure::Text | Measure::Mark => {
                    drawn_contrast(colours.foreground, colours.background, ground)
                }
                Measure::Edge => colours.border.over(ground).contrast(ground),
            };
            (found, token)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .expect("every set of grounds has one")
}

/// Checks every colour pair a component draws, over every surface it may be drawn on, with the
/// contrast minimums of `options`: text at `text_contrast`; a mark that shows a state, a focus
/// ring and an essential edge at `ui_contrast`. Disabled styles and decorative borders are not
/// held to a minimum. Fails on the first pair that falls short, naming it.
pub fn validate(
    semantic: &SemanticColours,
    options: &ResolveOptions,
) -> Result<(), ComponentError> {
    for entry in catalogue() {
        let colours = entry.colours.resolve(semantic);
        for &m in entry.measures {
            let (found, against) = measure(semantic, m, &colours, entry.template.grounds);
            let required = m.required(options);
            if found < required {
                return Err(ComponentError {
                    component: entry.template.component,
                    variant: entry.template.variant,
                    state: entry.state,
                    what: m.name(),
                    found,
                    required,
                    against: against.name(),
                });
            }
        }
    }
    Ok(())
}

/// One style as it is drawn: what it is, its colours and where each comes from, its geometry,
/// and the contrast it reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDiagnostic {
    pub component: &'static str,
    pub variant: String,
    pub state: &'static str,
    /// `None` for a component whose geometry does not follow density.
    pub density: Option<Density>,
    pub role: Option<Role>,
    pub emphasis: Option<Emphasis>,
    pub colours: Treatment,
    pub sources: TreatmentRef,
    /// `None` for a style whose height is its content's.
    pub height: Option<f32>,
    pub shape: Shape,
    pub type_role: Option<TypeRole>,
    pub contrast: Vec<ContrastResult>,
}

/// One measure of one style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastResult {
    pub measure: Measure,
    /// The least contrast over the surfaces the style may be drawn on.
    pub found: f32,
    pub required: f32,
    /// The surface it is least on.
    pub against: &'static str,
}

impl ContrastResult {
    pub fn passes(&self) -> bool {
        self.found >= self.required
    }
}

impl ComponentTokens {
    /// Every style of every component, with its sources, geometry at each density it has, and
    /// the contrast it reaches against `options`' minimums. Built on request, never while
    /// drawing; for a tool such as the gallery.
    pub fn diagnostics(&self, options: &ResolveOptions) -> Vec<StyleDiagnostic> {
        let mut rows = Vec::new();
        for entry in catalogue() {
            let t = &entry.template;
            let colours = entry.colours.resolve(&self.semantic);
            let contrast: Vec<_> = entry
                .measures
                .iter()
                .map(|&m| {
                    let (found, against) = measure(&self.semantic, m, &colours, t.grounds);
                    ContrastResult {
                        measure: m,
                        found,
                        required: m.required(options),
                        against: against.name(),
                    }
                })
                .collect();
            let densities: Vec<Option<Density>> = if t.follows_density {
                Density::ALL.into_iter().map(Some).collect()
            } else {
                vec![None]
            };
            for density in densities {
                let summary = (t.geometry)(
                    &self.foundation,
                    density.unwrap_or(self.foundation.controls.density),
                );
                rows.push(StyleDiagnostic {
                    component: t.component,
                    variant: t.variant.clone(),
                    state: entry.state,
                    density,
                    role: t.role,
                    emphasis: t.emphasis,
                    colours,
                    sources: entry.colours,
                    height: summary.height,
                    shape: summary.shape,
                    type_role: summary.type_role,
                    contrast: contrast.clone(),
                });
            }
        }
        rows
    }
}

fn hex(c: Color) -> String {
    c.to_hex()
}

impl fmt::Display for StyleDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.component)?;
        if !self.variant.is_empty() {
            write!(f, " {}", self.variant)?;
        }
        write!(f, " {}", self.state)?;
        if let Some(d) = self.density {
            write!(f, ", {}", d.name())?;
        }
        if let Some(role) = self.role {
            write!(f, " ({role}")?;
            if let Some(e) = self.emphasis {
                write!(f, " {}", e.name())?;
            }
            write!(f, ")")?;
        }
        let c = &self.colours;
        let s = &self.sources;
        write!(
            f,
            ": background {} ({}), foreground {} ({}), border {} ({})",
            hex(c.background),
            s.background,
            hex(c.foreground),
            s.foreground,
            hex(c.border),
            s.border
        )?;
        match self.height {
            Some(h) => write!(f, "; height {h}")?,
            None => write!(f, "; height of its content")?,
        }
        match self.shape {
            Shape::Rect { radius } => write!(f, ", radius {radius}")?,
            Shape::Pill => write!(f, ", pill")?,
            Shape::Circle => write!(f, ", circle")?,
        }
        if let Some(role) = self.type_role {
            write!(f, ", type {}", role.name())?;
        }
        for r in &self.contrast {
            write!(
                f,
                "; {} {:.2}:1 (needs {}:1, least on surface.{}){}",
                r.measure.name(),
                r.found,
                r.required,
                r.against,
                if r.passes() { "" } else { ", FAILS" }
            )?;
        }
        Ok(())
    }
}
