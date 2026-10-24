//! The semantic layer of ADR 0040: colours by role and emphasis, made from a [`Palette`] by the
//! adaptive resolver, defined in `docs/spec/odk-semantic-tokens.md`.
//!
//! [`resolve`] is a pure function of a Palette and [`ResolveOptions`]: no window, no file, no
//! theme name. It returns the [`SemanticColours`] and a [`Report`] of how each colour was
//! chosen. [`crate::ThemeManager`] runs it once per theme and per change of options and
//! publishes both in its [`crate::ThemeSnapshot`]; nothing runs it while drawing. Components draw
//! them through the component layer.
//!
//! [`Palette`]: crate::Palette

use std::collections::BTreeMap;
use std::fmt;

use toml::{Table, Value};

use crate::{Color, Mode, NON_TEXT_CONTRAST, PaletteKey, TEXT_CONTRAST};

pub use crate::resolve::{
    BorderToken, Considered, Decision, Note, Reason, Report, Requirement, Resolution, ResolveError,
    Source, Subject, SurfaceToken, TextToken, resolve,
};

/// What a colour means. The palette's own names (`red`, `bright_red`) say nothing about this;
/// the resolver decides which of them plays each role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    /// The theme's own colour: primary actions, selection, links.
    Accent,
    /// No meaning beyond "a control": secondary actions, chips, plain surfaces.
    Neutral,
    /// Destructive actions, errors.
    Danger,
    /// Completed actions, valid input.
    Success,
    /// Something to look at before going on.
    Warning,
    /// Neutral information that still wants attention.
    Info,
}

impl Role {
    pub const ALL: [Self; 6] = [
        Self::Accent,
        Self::Neutral,
        Self::Danger,
        Self::Success,
        Self::Warning,
        Self::Info,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Accent => "accent",
            Self::Neutral => "neutral",
            Self::Danger => "danger",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.name() == name)
    }

    /// The palette keys that may play this role, most preferred first, before overrides. A
    /// role's colour is never taken from outside this list, except the accent's relatives
    /// (`docs/spec/odk-semantic-tokens.md`, "Candidates").
    pub const fn candidates(self) -> &'static [PaletteKey] {
        use PaletteKey as K;
        match self {
            Self::Accent => &[K::Accent],
            Self::Neutral => &[K::Foreground, K::BrightForeground, K::LightForeground],
            Self::Danger => &[K::Red, K::BrightRed],
            Self::Success => &[K::Green, K::BrightGreen],
            Self::Warning => &[K::Yellow, K::BrightYellow, K::Orange],
            Self::Info => &[K::Cyan, K::BrightCyan, K::Blue, K::BrightBlue],
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How prominently a role shows. Independent of which palette colour plays the role: one
/// source colour gives all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Emphasis {
    /// A filled background in the role's colour with a label that contrasts with it: primary
    /// and prominent actions.
    Solid,
    /// A tint of the role's colour, its text in the role's colour and a faint border:
    /// secondary actions, badges, alerts.
    Muted,
    /// No fill until hovered, text in the role's colour, no border: text actions, ghost
    /// buttons, quiet feedback.
    Subtle,
}

impl Emphasis {
    pub const ALL: [Self; 3] = [Self::Solid, Self::Muted, Self::Subtle];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Muted => "muted",
            Self::Subtle => "subtle",
        }
    }
}

/// The interaction states a treatment has colours for. Focus is not one of them: it is drawn
/// over any state, with [`RoleColours::focus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum State {
    Default,
    Hover,
    Pressed,
    Disabled,
}

impl State {
    pub const ALL: [Self; 4] = [Self::Default, Self::Hover, Self::Pressed, Self::Disabled];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Hover => "hover",
            Self::Pressed => "pressed",
            Self::Disabled => "disabled",
        }
    }
}

/// The three colours a control is drawn with in one state. Any of them may be translucent;
/// the resolver has checked the foreground against the background as drawn over every surface
/// the control may sit on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Treatment {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
}

/// One emphasis of a role, in every state.
#[derive(Debug, Clone, PartialEq)]
pub struct EmphasisColours {
    pub default: Treatment,
    pub hover: Treatment,
    pub pressed: Treatment,
    pub disabled: Treatment,
}

impl EmphasisColours {
    pub fn state(&self, state: State) -> &Treatment {
        match state {
            State::Default => &self.default,
            State::Hover => &self.hover,
            State::Pressed => &self.pressed,
            State::Disabled => &self.disabled,
        }
    }
}

/// Every treatment of one role, and its focus colour.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleColours {
    pub solid: EmphasisColours,
    pub muted: EmphasisColours,
    pub subtle: EmphasisColours,
    /// The focus indicator around a control of this role, in any state. At least
    /// [`ResolveOptions::ui_contrast`] against every surface and every muted and subtle fill
    /// of the role; around a solid fill it needs a gap of the surface between them
    /// (`docs/spec/odk-semantic-tokens.md`, "Focus").
    pub focus: Color,
}

impl RoleColours {
    pub fn emphasis(&self, emphasis: Emphasis) -> &EmphasisColours {
        match emphasis {
            Emphasis::Solid => &self.solid,
            Emphasis::Muted => &self.muted,
            Emphasis::Subtle => &self.subtle,
        }
    }
}

/// The six roles.
#[derive(Debug, Clone, PartialEq)]
pub struct Roles {
    pub accent: RoleColours,
    pub neutral: RoleColours,
    pub danger: RoleColours,
    pub success: RoleColours,
    pub warning: RoleColours,
    pub info: RoleColours,
}

impl Roles {
    pub fn get(&self, role: Role) -> &RoleColours {
        match role {
            Role::Accent => &self.accent,
            Role::Neutral => &self.neutral,
            Role::Danger => &self.danger,
            Role::Success => &self.success,
            Role::Warning => &self.warning,
            Role::Info => &self.info,
        }
    }
}

/// Where things are drawn. All opaque except `hover`, `pressed` and `scrim`, and `selection`
/// when the theme has none, which are drawn over whatever surface is below.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceColours {
    /// The window: `background`.
    pub canvas: Color,
    /// A well one step below the canvas, such as a sidebar or a field: `dark_background`.
    pub recessed: Color,
    /// Two steps below: `darker_background`, or one more of the canvas-to-recessed step.
    pub sunken: Color,
    /// A panel or card above the canvas: `lighter_background`.
    pub raised: Color,
    /// Menus, popovers and dialogs: `lighter_background`, set apart by their border.
    pub overlay: Color,
    /// Selected text and rows: `selection_background`, else `selection`, else the accent made
    /// translucent.
    pub selection: Color,
    /// A row or item under the pointer: a translucent tint of the foreground.
    pub hover: Color,
    /// A row or item being pressed: a stronger tint of the foreground.
    pub pressed: Color,
    /// Dims the window behind a dialog or a sheet: `sunken`, translucent.
    pub scrim: Color,
}

impl SurfaceColours {
    pub fn get(&self, token: SurfaceToken) -> Color {
        match token {
            SurfaceToken::Canvas => self.canvas,
            SurfaceToken::Recessed => self.recessed,
            SurfaceToken::Sunken => self.sunken,
            SurfaceToken::Raised => self.raised,
            SurfaceToken::Overlay => self.overlay,
            SurfaceToken::Selection => self.selection,
            SurfaceToken::Hover => self.hover,
            SurfaceToken::Pressed => self.pressed,
            SurfaceToken::Scrim => self.scrim,
        }
    }

    /// The opaque surfaces text and controls are drawn on, which every text colour and
    /// treatment is checked against.
    pub fn grounds(&self) -> [Color; 5] {
        [
            self.canvas,
            self.recessed,
            self.sunken,
            self.raised,
            self.overlay,
        ]
    }
}

/// Text, each colour at least [`ResolveOptions::text_contrast`] against every surface it is
/// for, except `disabled`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextColours {
    pub primary: Color,
    /// Less prominent than `primary`, more than `tertiary`.
    pub secondary: Color,
    /// The quietest text that is still readable.
    pub tertiary: Color,
    /// The label of a control that cannot be used: between 2:1 and 3:1 against the canvas, so
    /// it can be read but never passes for active text or an active control.
    pub disabled: Color,
    /// Text on a fill of `primary`, such as a tooltip.
    pub inverse: Color,
    /// A link. Draw it underlined: colour alone does not set it apart from text in every
    /// theme.
    pub link: Color,
    /// A field's placeholder: as readable as `tertiary`.
    pub placeholder: Color,
    /// Text on the accent's solid fill.
    pub on_accent: Color,
    /// Text on `surface.selection`.
    pub on_selection: Color,
}

impl TextColours {
    pub fn get(&self, token: TextToken) -> Color {
        match token {
            TextToken::Primary => self.primary,
            TextToken::Secondary => self.secondary,
            TextToken::Tertiary => self.tertiary,
            TextToken::Disabled => self.disabled,
            TextToken::Inverse => self.inverse,
            TextToken::Link => self.link,
            TextToken::Placeholder => self.placeholder,
            TextToken::OnAccent => self.on_accent,
            TextToken::OnSelection => self.on_selection,
        }
    }
}

/// Borders. `subtle`, `default` and `disabled` are decoration and may be faint; `strong`,
/// `hover`, `focus` and `danger` mark something the user needs to see and reach
/// [`ResolveOptions::ui_contrast`] against every surface.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderColours {
    /// Separators.
    pub subtle: Color,
    /// Cards, panels and overlays.
    pub default: Color,
    /// The edge of a field or a check box, which shows where the control is.
    pub strong: Color,
    /// `strong` under the pointer.
    pub hover: Color,
    /// The focus indicator of a neutral control: the accent's focus colour.
    pub focus: Color,
    pub disabled: Color,
    /// The edge of a field whose value is wrong.
    pub danger: Color,
}

impl BorderColours {
    pub fn get(&self, token: BorderToken) -> Color {
        match token {
            BorderToken::Subtle => self.subtle,
            BorderToken::Default => self.default,
            BorderToken::Strong => self.strong,
            BorderToken::Hover => self.hover,
            BorderToken::Focus => self.focus,
            BorderToken::Disabled => self.disabled,
            BorderToken::Danger => self.danger,
        }
    }
}

/// One of a treatment's three colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Background,
    Foreground,
    Border,
}

impl Part {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Foreground => "foreground",
            Self::Border => "border",
        }
    }
}

/// A name for one semantic colour. The component layer stores these rather than colours, so a
/// component's colour is always one the resolver made and checked, and diagnostics can say
/// which (`docs/spec/odk-component-tokens.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColourRef {
    /// Nothing is drawn: fully transparent.
    None,
    Surface(SurfaceToken),
    Text(TextToken),
    Border(BorderToken),
    /// One colour of one treatment: `roles.<role>.<emphasis>.<state>.<part>`.
    Treatment(Role, Emphasis, State, Part),
    /// `roles.<role>.focus`.
    Focus(Role),
}

impl fmt::Display for ColourRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::Surface(token) => write!(f, "surface.{}", token.name()),
            Self::Text(token) => write!(f, "text.{}", token.name()),
            Self::Border(token) => write!(f, "border.{}", token.name()),
            Self::Treatment(role, emphasis, state, part) => write!(
                f,
                "roles.{role}.{}.{}.{}",
                emphasis.name(),
                state.name(),
                part.name()
            ),
            Self::Focus(role) => write!(f, "roles.{role}.focus"),
        }
    }
}

/// The whole semantic layer for one theme.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticColours {
    pub mode: Mode,
    pub surface: SurfaceColours,
    pub text: TextColours,
    pub border: BorderColours,
    pub roles: Roles,
}

impl SemanticColours {
    pub fn role(&self, role: Role) -> &RoleColours {
        self.roles.get(role)
    }

    pub fn treatment(&self, role: Role, emphasis: Emphasis, state: State) -> &Treatment {
        self.roles.get(role).emphasis(emphasis).state(state)
    }

    /// The colour `reference` names. [`ColourRef::None`] is the canvas made fully transparent.
    pub fn colour(&self, reference: ColourRef) -> Color {
        match reference {
            ColourRef::None => self.surface.canvas.with_alpha(0.0),
            ColourRef::Surface(token) => self.surface.get(token),
            ColourRef::Text(token) => self.text.get(token),
            ColourRef::Border(token) => self.border.get(token),
            ColourRef::Treatment(role, emphasis, state, part) => {
                let t = self.treatment(role, emphasis, state);
                match part {
                    Part::Background => t.background,
                    Part::Foreground => t.foreground,
                    Part::Border => t.border,
                }
            }
            ColourRef::Focus(role) => self.role(role).focus,
        }
    }
}

/// How the resolver ranks candidates (`docs/spec/odk-semantic-tokens.md`, "Strategies").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strategy {
    /// Prefer the role's own hue family, then the colour that needs the least change, then
    /// the earlier candidate.
    #[default]
    Adaptive,
    /// Prefer the first candidate that passes as the theme wrote it; adjust the first
    /// candidate only when none does.
    Strict,
    /// Like `Strict`, but a role with an override considers only the override's keys.
    Manual,
}

impl Strategy {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::Strict => "strict",
            Self::Manual => "manual",
        }
    }
}

/// A role's preferred palette keys, tried before its defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleOverride {
    pub preferred: PaletteKey,
    pub fallback: Vec<PaletteKey>,
}

impl RoleOverride {
    pub fn keys(&self) -> impl Iterator<Item = PaletteKey> + '_ {
        std::iter::once(self.preferred).chain(self.fallback.iter().copied())
    }
}

/// Which palette keys each role prefers, an application's own choice. Overrides change the
/// order candidates are tried in, never the checks a colour must pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Overrides(BTreeMap<Role, RoleOverride>);

impl Overrides {
    pub fn get(&self, role: Role) -> Option<&RoleOverride> {
        self.0.get(&role)
    }

    pub fn set(&mut self, role: Role, choice: RoleOverride) {
        self.0.insert(role, choice);
    }

    pub fn remove(&mut self, role: Role) {
        self.0.remove(&role);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The highest contrast there is: black on white.
const MAX_CONTRAST: f32 = 21.0;

/// What the resolver must reach and how it ranks. [`ResolveOptions::validate`] refuses values
/// that would weaken WCAG's minimums or cannot be met at all.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveOptions {
    /// The least contrast of every text colour, from 4.5 (WCAG AA) to 21.
    pub text_contrast: f32,
    /// The least contrast of focus indicators and essential borders, from 3 (WCAG AA) to
    /// `text_contrast`.
    pub ui_contrast: f32,
    pub strategy: Strategy,
    pub overrides: Overrides,
}

impl Default for ResolveOptions {
    fn default() -> Self {
        Self {
            text_contrast: TEXT_CONTRAST,
            ui_contrast: NON_TEXT_CONTRAST,
            strategy: Strategy::Adaptive,
            overrides: Overrides::default(),
        }
    }
}

impl ResolveOptions {
    pub fn validate(&self) -> Result<(), OptionsError> {
        let check = |field: &'static str, value: f32, min: f32| {
            if value.is_finite() && (min..=MAX_CONTRAST).contains(&value) {
                Ok(())
            } else {
                Err(OptionsError::Contrast { field, value, min })
            }
        };
        check("text_contrast", self.text_contrast, TEXT_CONTRAST)?;
        check("ui_contrast", self.ui_contrast, NON_TEXT_CONTRAST)?;
        if self.ui_contrast > self.text_contrast {
            return Err(OptionsError::UiAboveText {
                ui: self.ui_contrast,
                text: self.text_contrast,
            });
        }
        Ok(())
    }

    /// Reads options from TOML text, an application's own configuration (no file is read
    /// here). Everything is under `[colors]`, and anything left out keeps its default:
    ///
    /// ```toml
    /// [colors]
    /// strategy = "strict"   # "adaptive", "strict" or "manual"
    /// text_contrast = 7.0
    /// ui_contrast = 3.0
    ///
    /// [colors.warning]
    /// preferred = "yellow"
    /// fallback = "bright_yellow"   # or a list: ["bright_yellow", "orange"]
    /// ```
    ///
    /// Other top-level tables are ignored. Inside `[colors]`, a key or a role it does not know
    /// and a palette key that does not exist are errors that name them.
    pub fn from_toml(text: &str) -> Result<Self, OptionsError> {
        let table: Table =
            toml::from_str(text).map_err(|e| OptionsError::Toml(e.message().to_owned()))?;
        let mut options = Self::default();
        let Some(colors) = table.get("colors") else {
            return Ok(options);
        };
        let colors = colors
            .as_table()
            .ok_or_else(|| wrong("colors", "a table", colors))?;
        for (key, value) in colors {
            match key.as_str() {
                "strategy" => {
                    let name = value
                        .as_str()
                        .ok_or_else(|| wrong(key, "a string", value))?;
                    options.strategy = match name {
                        "adaptive" => Strategy::Adaptive,
                        "strict" => Strategy::Strict,
                        "manual" => Strategy::Manual,
                        other => return Err(OptionsError::BadStrategy(other.to_owned())),
                    };
                }
                "text_contrast" => options.text_contrast = number(key, value)?,
                "ui_contrast" => options.ui_contrast = number(key, value)?,
                name => {
                    let role = Role::from_name(name)
                        .ok_or_else(|| OptionsError::UnknownOption(name.to_owned()))?;
                    options.overrides.set(role, role_override(role, value)?);
                }
            }
        }
        options.validate()?;
        Ok(options)
    }
}

fn role_override(role: Role, value: &Value) -> Result<RoleOverride, OptionsError> {
    let section = format!("colors.{}", role.name());
    let table = value
        .as_table()
        .ok_or_else(|| wrong(&section, "a table", value))?;
    let key = |name: &str| {
        PaletteKey::from_name(name).ok_or_else(|| OptionsError::UnknownKey {
            role: role.name(),
            key: name.to_owned(),
        })
    };
    let mut preferred = None;
    let mut fallback = Vec::new();
    for (field, value) in table {
        match field.as_str() {
            "preferred" => {
                let name = value
                    .as_str()
                    .ok_or_else(|| wrong(&format!("{section}.preferred"), "a string", value))?;
                preferred = Some(key(name)?);
            }
            "fallback" => {
                let names: Vec<&Value> = match value {
                    Value::Array(items) => items.iter().collect(),
                    other => vec![other],
                };
                for name in names {
                    let name = name.as_str().ok_or_else(|| {
                        wrong(
                            &format!("{section}.fallback"),
                            "a string or a list of strings",
                            name,
                        )
                    })?;
                    fallback.push(key(name)?);
                }
            }
            other => {
                return Err(OptionsError::UnknownField {
                    role: role.name(),
                    field: other.to_owned(),
                });
            }
        }
    }
    Ok(RoleOverride {
        preferred: preferred.ok_or(OptionsError::NoPreferred(role.name()))?,
        fallback,
    })
}

fn number(key: &str, value: &Value) -> Result<f32, OptionsError> {
    match value {
        Value::Float(f) => Ok(*f as f32),
        Value::Integer(i) => Ok(*i as f32),
        other => Err(wrong(key, "a number", other)),
    }
}

fn wrong(key: &str, expected: &'static str, found: &Value) -> OptionsError {
    OptionsError::WrongType {
        key: key.to_owned(),
        expected,
        found: found.type_str(),
    }
}

/// Why options were refused. Each names what to fix.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OptionsError {
    #[error("`{field}` is {value}; it must be a contrast ratio from {min} to 21")]
    Contrast {
        field: &'static str,
        value: f32,
        min: f32,
    },
    #[error(
        "`ui_contrast` ({ui}) is above `text_contrast` ({text}); text needs at least what a control's edge needs"
    )]
    UiAboveText { ui: f32, text: f32 },
    #[error("the colour options are not valid TOML: {0}")]
    Toml(String),
    #[error("`{key}` must be {expected}, not a TOML {found}")]
    WrongType {
        key: String,
        expected: &'static str,
        found: &'static str,
    },
    #[error("`strategy` is {0:?}; it must be \"adaptive\", \"strict\" or \"manual\"")]
    BadStrategy(String),
    #[error(
        "`colors.{0}` is not a colour option; the options are `strategy`, `text_contrast`, `ui_contrast` and the roles accent, neutral, danger, success, warning and info"
    )]
    UnknownOption(String),
    #[error("`colors.{role}` has no `{field}`; it takes `preferred` and `fallback`")]
    UnknownField { role: &'static str, field: String },
    #[error(
        "`colors.{role}` names {key:?}, which is not a colors.toml colour; use a name such as \"yellow\" or \"bright_yellow\""
    )]
    UnknownKey { role: &'static str, key: String },
    #[error("`colors.{0}` needs a `preferred` palette key")]
    NoPreferred(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_wcag_aa_and_valid() {
        let o = ResolveOptions::default();
        assert_eq!((o.text_contrast, o.ui_contrast), (4.5, 3.0));
        assert_eq!(o.strategy, Strategy::Adaptive);
        assert!(o.overrides.is_empty());
        assert_eq!(o.validate(), Ok(()));
    }

    #[test]
    fn options_that_weaken_wcag_or_cannot_be_met_are_refused() {
        let with = |text: f32, ui: f32| ResolveOptions {
            text_contrast: text,
            ui_contrast: ui,
            ..Default::default()
        };
        for (text, ui) in [
            (4.4, 3.0),
            (4.5, 2.9),
            (21.5, 3.0),
            (f32::NAN, 3.0),
            (4.5, f32::NAN),
            (f32::INFINITY, 3.0),
            (-4.5, 3.0),
        ] {
            assert!(
                matches!(
                    with(text, ui).validate(),
                    Err(OptionsError::Contrast { .. })
                ),
                "{text} {ui}"
            );
        }
        assert!(matches!(
            with(4.5, 5.0).validate(),
            Err(OptionsError::UiAboveText { .. })
        ));
        assert_eq!(with(7.0, 4.5).validate(), Ok(()));
        assert_eq!(with(21.0, 21.0).validate(), Ok(()));
    }

    #[test]
    fn options_are_read_from_toml() {
        let o = ResolveOptions::from_toml(
            r#"
[colors]
strategy = "manual"
text_contrast = 7
ui_contrast = 3.5

[colors.warning]
preferred = "yellow"
fallback = "bright_yellow"

[colors.info]
preferred = "blue"
fallback = ["bright_blue", "cyan"]

[unrelated]
anything = true
"#,
        )
        .unwrap();
        assert_eq!(o.strategy, Strategy::Manual);
        assert_eq!((o.text_contrast, o.ui_contrast), (7.0, 3.5));
        let warning = o.overrides.get(Role::Warning).unwrap();
        assert_eq!(warning.preferred, PaletteKey::Yellow);
        assert_eq!(warning.fallback, [PaletteKey::BrightYellow]);
        let info: Vec<_> = o.overrides.get(Role::Info).unwrap().keys().collect();
        assert_eq!(
            info,
            [PaletteKey::Blue, PaletteKey::BrightBlue, PaletteKey::Cyan]
        );
        assert_eq!(o.overrides.get(Role::Danger), None);
        assert_eq!(
            ResolveOptions::from_toml("").unwrap(),
            ResolveOptions::default()
        );
    }

    #[test]
    fn bad_toml_options_say_what_is_wrong() {
        let err = |text: &str| ResolveOptions::from_toml(text).unwrap_err();
        assert!(matches!(
            err("[colors.warning]\npreferred = \"purple\"\n"),
            OptionsError::UnknownKey { role: "warning", ref key } if key == "purple"
        ));
        assert!(matches!(
            err("[colors.warning]\npreferred = \"yellow\"\nfallback = [\"bright_yelow\"]\n"),
            OptionsError::UnknownKey { ref key, .. } if key == "bright_yelow"
        ));
        assert!(matches!(
            err("[colors.danger]\nfallback = \"red\"\n"),
            OptionsError::NoPreferred("danger")
        ));
        assert!(matches!(
            err("[colors.error]\npreferred = \"red\"\n"),
            OptionsError::UnknownOption(ref k) if k == "error"
        ));
        let field = err("[colors.danger]\npreferred = \"red\"\nweight = 2\n");
        assert!(
            matches!(field, OptionsError::UnknownField { role: "danger", ref field } if field == "weight")
        );
        assert_eq!(
            field.to_string(),
            "`colors.danger` has no `weight`; it takes `preferred` and `fallback`"
        );
        assert_eq!(
            err("[colors.error]\npreferred = \"red\"\n").to_string(),
            "`colors.error` is not a colour option; the options are `strategy`, `text_contrast`, `ui_contrast` and the roles accent, neutral, danger, success, warning and info"
        );
        assert!(matches!(
            err("[colors]\nstrategy = \"clever\"\n"),
            OptionsError::BadStrategy(_)
        ));
        assert!(matches!(
            err("[colors]\ntext_contrast = \"high\"\n"),
            OptionsError::WrongType { .. }
        ));
        assert!(matches!(
            err("[colors]\ntext_contrast = 3\n"),
            OptionsError::Contrast {
                field: "text_contrast",
                ..
            }
        ));
        assert!(matches!(err("[colors"), OptionsError::Toml(_)));
        let message = err("[colors.warning]\npreferred = \"purple\"\n").to_string();
        assert!(
            message.contains("`colors.warning`") && message.contains("purple"),
            "{message}"
        );
    }

    #[test]
    fn every_role_has_candidates_and_a_name() {
        for role in Role::ALL {
            assert!(!role.candidates().is_empty(), "{role}");
            assert_eq!(Role::from_name(role.name()), Some(role));
        }
        assert_eq!(Role::from_name("error"), None);
    }
}
