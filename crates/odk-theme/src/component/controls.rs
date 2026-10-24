//! Controls: buttons, fields (input, textarea, input group, the box of a select), check boxes and
//! switches (`docs/spec/odk-component-tokens.md`).

use super::catalogue::{Entry, Measure, Summary, Template};
use super::geometry::{
    self, CheckGeometry, ControlGeometry, FocusRing, RadioGeometry, Shape, SwitchGeometry,
};
use super::state::{Check, States};
use super::{ComponentTokens, Interactive, TreatmentRef, focus_of};
use crate::Color;
use crate::foundation::{Density, FoundationTokens, TypeRole};
use crate::semantic::{
    BorderToken as B, ColourRef as C, Emphasis, Part, Role, State, SurfaceToken as S,
    TextToken as T, Treatment,
};

/// A button's variant. Each is a role at an emphasis ([`ButtonVariant::appearance`]), which an
/// application can change, so a destructive button can also be muted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonVariant {
    /// The main action: the accent, solid.
    Primary,
    /// An ordinary action: neutral, muted.
    #[default]
    Secondary,
    /// Neutral, subtle, with an edge.
    Outline,
    /// A quiet action: neutral, subtle.
    Ghost,
    /// An action that destroys something: danger, solid.
    Destructive,
    Success,
    Warning,
    /// The accent as an underlined link, with no fill in any state.
    Link,
}

impl ButtonVariant {
    pub const ALL: [Self; 8] = [
        Self::Primary,
        Self::Secondary,
        Self::Outline,
        Self::Ghost,
        Self::Destructive,
        Self::Success,
        Self::Warning,
        Self::Link,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Outline => "outline",
            Self::Ghost => "ghost",
            Self::Destructive => "destructive",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Link => "link",
        }
    }

    /// The role and emphasis the variant draws with unless the application chooses others.
    pub const fn appearance(self) -> ButtonAppearance {
        use Emphasis::{Muted, Solid, Subtle};
        match self {
            Self::Primary => ButtonAppearance::new(Role::Accent, Solid),
            Self::Secondary => ButtonAppearance::new(Role::Neutral, Muted),
            Self::Outline => ButtonAppearance::new(Role::Neutral, Subtle).outlined(),
            Self::Ghost => ButtonAppearance::new(Role::Neutral, Subtle),
            Self::Destructive => ButtonAppearance::new(Role::Danger, Solid),
            Self::Success => ButtonAppearance::new(Role::Success, Solid),
            Self::Warning => ButtonAppearance::new(Role::Warning, Solid),
            Self::Link => ButtonAppearance::new(Role::Accent, Subtle).linked(),
        }
    }
}

/// What a button looks like: a role at an emphasis, optionally with an edge or drawn as a link.
/// Any role goes with any emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ButtonAppearance {
    pub role: Role,
    pub emphasis: Emphasis,
    /// Drawn with an edge in the role's colour (`border.strong` for a neutral button).
    pub outline: bool,
    /// Drawn as an underlined link: text in the role's colour, no fill and no edge in any state.
    /// The emphasis then does not matter.
    pub link: bool,
}

impl ButtonAppearance {
    pub const fn new(role: Role, emphasis: Emphasis) -> Self {
        Self {
            role,
            emphasis,
            outline: false,
            link: false,
        }
    }

    pub const fn with_emphasis(mut self, emphasis: Emphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    pub const fn with_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    pub const fn outlined(mut self) -> Self {
        self.outline = true;
        self
    }

    pub const fn linked(mut self) -> Self {
        self.link = true;
        self
    }

    /// The name diagnostics list it under: `"danger muted"`, `"neutral subtle outline"`,
    /// `"accent link"`. A link's emphasis and edge do not change how it is drawn, so they are
    /// not in its name. The eight variants are also listed under their own names.
    pub fn name(&self) -> String {
        if self.link {
            return format!("{} link", self.role);
        }
        let edge = if self.outline { " outline" } else { "" };
        format!("{} {}{edge}", self.role, self.emphasis.name())
    }
}

impl From<ButtonVariant> for ButtonAppearance {
    fn from(variant: ButtonVariant) -> Self {
        variant.appearance()
    }
}

/// What a button holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ButtonLayout {
    /// A label: as wide as its content.
    #[default]
    Text,
    /// An icon before the label.
    LeadingIcon,
    /// An icon after the label.
    TrailingIcon,
    /// An icon alone: a square as wide as it is high, which needs an accessible name.
    IconOnly,
}

/// A button in one appearance, density, layout and set of states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonStyle {
    pub appearance: ButtonAppearance,
    pub layout: ButtonLayout,
    /// At rest, under hover and pressed. All three are the disabled colours while disabled, and
    /// the resting ones while loading.
    pub colours: Interactive<Treatment>,
    /// The colours for the states asked for.
    pub current: Treatment,
    pub focus: FocusRing,
    /// Whether to draw `focus` for the states asked for.
    pub focus_visible: bool,
    pub geometry: ControlGeometry,
    /// `Some` for an icon-only button, which is square; `None` for one as wide as its content.
    /// No text button has a minimum width.
    pub width: Option<f32>,
    /// A link's label is underlined, since colour alone does not set it apart from text.
    pub underline: bool,
    /// The spinner a loading button draws over its content, in its foreground. The content
    /// keeps its place, so loading never changes the button's size.
    pub spinner_size: f32,
    /// Whether hover and pressed colours apply and a press acts.
    pub responds: bool,
}

/// A field: an input (also masked, for a password, and a search or number field), a textarea,
/// an input group, the field of a command palette, and the box of a select.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldStyle {
    /// The box: background its fill, foreground the value's text, border its edge. While the
    /// field is focused or its list is open, all three carry the focused edge, so the pointer
    /// never takes it away.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    /// The edge while focused, which a wrong value keeps as `border.danger`.
    pub focus_border: Color,
    pub placeholder: Color,
    /// Selected text: background the selection, foreground its text.
    pub selection: Treatment,
    pub caret: Color,
    /// The mark beside the label of a field that must be filled in.
    pub required: Color,
    /// A leading or trailing icon, such as a search field's: text tertiary, primary while a
    /// select's list is open.
    pub icon: Color,
    /// Text around the value inside the box, such as an input group's `https://`.
    pub addon: Color,
    /// The colour of [`FIELD_INVALID_MARK`], `Some` while the value is wrong, so a wrong value
    /// never shows by its edge's colour alone (a monochrome theme's danger edge is grey).
    pub invalid_mark: Option<Color>,
    pub focus: FocusRing,
    pub focus_visible: bool,
    /// The density's box, text in `body`. A leading or trailing icon is `icon_size`, `gap` from
    /// the text. A textarea's height is [`ControlGeometry::rows_height`].
    pub geometry: ControlGeometry,
    pub responds: bool,
}

/// The mark of a wrong value: at the end of a field, after a check box's label, after a radio
/// group's first option. Not the danger role's `×`, which a field already shows as its clear
/// button.
pub const FIELD_INVALID_MARK: &str = "!";

/// The box of a select and the list it opens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectStyle {
    pub trigger: FieldStyle,
    /// The `▾` at its end, `trigger.geometry.icon_size` square: the field's icon colour.
    pub indicator: Color,
}

/// A check box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckboxStyle {
    /// The box: background its fill, foreground its mark, border its edge.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    pub check: Check,
    pub label: Color,
    /// The colour of [`FIELD_INVALID_MARK`] after the label, `Some` while the choice is wrong,
    /// so it never shows by its edge's colour alone.
    pub invalid_mark: Option<Color>,
    pub focus: FocusRing,
    pub focus_visible: bool,
    pub geometry: CheckGeometry,
    pub responds: bool,
}

/// A radio: a check box's colours on a circle, the chosen one (`states.checked`) filled with
/// a dot of its mark colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioStyle {
    /// The circle: background its fill, foreground the dot, border its edge.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    pub checked: bool,
    pub label: Color,
    /// As a check box's: `Some` while the group's choice is wrong.
    pub invalid_mark: Option<Color>,
    pub focus: FocusRing,
    pub focus_visible: bool,
    pub geometry: RadioGeometry,
    pub responds: bool,
}

/// What surrounds a control in a form: its label, a description, the mark of a field that must
/// be filled in or may be left empty, and the gaps between them. A message under the control is
/// a [`super::StatusStyle`] from [`ComponentTokens::form_message`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormFieldStyle {
    pub label: Color,
    pub label_role: TypeRole,
    pub description: Color,
    pub description_role: TypeRole,
    pub message_role: TypeRole,
    /// The `*` after the label of a required field.
    pub required: Color,
    /// The word after the label of an optional one.
    pub optional: Color,
    /// Between the label and the control, and the control and its message: `field_gap`.
    pub gap: f32,
    /// Between the fields of a form: `form_gap`.
    pub form_gap: f32,
    /// The mark of a message is `icon_size` wide, `gap` from its text.
    pub icon_size: f32,
}

/// A switch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchStyle {
    /// The track and thumb: background the track's fill, foreground the thumb, border the
    /// track's edge.
    pub colours: Interactive<Treatment>,
    pub current: Treatment,
    pub on: bool,
    pub label: Color,
    pub focus: FocusRing,
    pub focus_visible: bool,
    pub geometry: SwitchGeometry,
    pub responds: bool,
}

impl ComponentTokens {
    /// A button. `density` is the button's own or the application's ([`Self::density`]).
    pub fn button(
        &self,
        appearance: impl Into<ButtonAppearance>,
        density: Density,
        layout: ButtonLayout,
        states: States,
    ) -> ButtonStyle {
        let appearance = appearance.into();
        let (colours, current) = self.interactive(states, |s| button(appearance, s));
        let (geometry, width) = button_geometry(&self.foundation, density, layout);
        ButtonStyle {
            appearance,
            layout,
            colours,
            current,
            focus: self.ring(focus_of(appearance.role)),
            focus_visible: states.shows_focus(),
            geometry,
            width,
            underline: appearance.link,
            spinner_size: geometry.icon_size,
            responds: states.responds(),
        }
    }

    /// A one-line field: an input, a masked input, a search or number field, an input group or a
    /// command palette's field.
    pub fn field(&self, density: Density, states: States) -> FieldStyle {
        let (mut colours, mut current) = self.interactive(states, |s| field(states, s));
        let focus_border = self.colour(field_focus_border(states));
        if !states.disabled && (states.focused || states.expanded) {
            // The focused edge holds under the pointer too: hover and press change nothing of it.
            for t in [
                &mut colours.default,
                &mut colours.hover,
                &mut colours.pressed,
                &mut current,
            ] {
                t.border = focus_border;
            }
        }
        let ring = if states.invalid {
            C::Border(B::Danger)
        } else {
            C::Border(B::Focus)
        };
        FieldStyle {
            colours,
            current,
            focus_border,
            placeholder: self.colour(if states.disabled {
                C::Text(T::Disabled)
            } else {
                C::Text(T::Placeholder)
            }),
            selection: self.paint(SELECTION),
            caret: self.colour(C::Text(T::Primary)),
            required: self.colour(REQUIRED),
            icon: self.colour(field_icon(states)),
            addon: self.colour(field_addon(states)),
            invalid_mark: self.invalid_mark(states),
            focus: self.ring(ring),
            focus_visible: states.shows_focus(),
            geometry: geometry::control(&self.foundation, density, TypeRole::Body),
            responds: states.responds() && !states.read_only,
        }
    }

    /// A button inside a field's box: a search field's clear button, a password field's show
    /// button, a number field's steppers, an input group's action. It is `appearance` at
    /// `layout`, as high as the field at `density` less its borders and the thumb gap on each
    /// side, so it never makes the field taller; an icon alone is a square of that height.
    pub fn field_action(
        &self,
        appearance: impl Into<ButtonAppearance>,
        density: Density,
        layout: ButtonLayout,
        states: States,
    ) -> ButtonStyle {
        let mut style = self.button(appearance, density, layout, states);
        let (geometry, width) = field_action_geometry(&self.foundation, density, layout);
        style.geometry = geometry;
        style.width = width;
        style
    }

    /// A field `rows` lines high: the field's colours, with its height made of rows of text.
    pub fn textarea(&self, density: Density, rows: usize, states: States) -> FieldStyle {
        let mut style = self.field(density, states);
        style.geometry.height = style.geometry.rows_height(rows);
        style
    }

    /// A select's box. Its list is a menu ([`Self::menu`], [`Self::menu_item`]). An open select
    /// keeps the edge of a focused field.
    pub fn select(&self, density: Density, states: States) -> SelectStyle {
        let trigger = self.field(density, states);
        SelectStyle {
            indicator: trigger.icon,
            trigger,
        }
    }

    /// A check box: `states.checked` and `states.indeterminate` give its mark.
    pub fn checkbox(&self, density: Density, states: States) -> CheckboxStyle {
        let check = states.check();
        let (colours, current) = self.interactive(states, |s| checkbox(check, states.invalid, s));
        CheckboxStyle {
            colours,
            current,
            check,
            label: self.colour(label(states)),
            invalid_mark: self.invalid_mark(states),
            focus: self.ring(if states.invalid {
                C::Border(B::Danger)
            } else {
                C::Border(B::Focus)
            }),
            focus_visible: states.shows_focus(),
            geometry: geometry::check(&self.foundation, density),
            responds: states.responds(),
        }
    }

    /// A radio: `states.checked` is the chosen one. It takes the check box's mapping, a chosen
    /// radio being a ticked box, and its geometry, on a circle.
    pub fn radio(&self, density: Density, states: States) -> RadioStyle {
        let check = if states.checked {
            Check::Checked
        } else {
            Check::Unchecked
        };
        let (colours, current) = self.interactive(states, |s| checkbox(check, states.invalid, s));
        RadioStyle {
            colours,
            current,
            checked: states.checked,
            label: self.colour(label(states)),
            invalid_mark: self.invalid_mark(states),
            focus: self.ring(if states.invalid {
                C::Border(B::Danger)
            } else {
                C::Border(B::Focus)
            }),
            focus_visible: states.shows_focus(),
            geometry: geometry::radio(&self.foundation, density),
            responds: states.responds(),
        }
    }

    /// The label, description and marks around a control in a form. Only `disabled` is read:
    /// a disabled control's label and description are `text.disabled`.
    pub fn form_field(&self, states: States) -> FormFieldStyle {
        let f = &self.foundation;
        let quiet = |c: C| {
            if states.disabled {
                C::Text(T::Disabled)
            } else {
                c
            }
        };
        FormFieldStyle {
            label: self.colour(label(states)),
            label_role: TypeRole::BodyStrong,
            description: self.colour(quiet(C::Text(T::Secondary))),
            description_role: TypeRole::Small,
            message_role: TypeRole::Small,
            required: self.colour(quiet(REQUIRED)),
            optional: self.colour(quiet(C::Text(T::Tertiary))),
            gap: f.layout.field_gap,
            form_gap: f.layout.form_gap,
            icon_size: f.typography.style(TypeRole::Small).size,
        }
    }

    /// A message under a control: an error (`Danger`), a warning, a confirmation (`Success`) in
    /// its role's text colour with its status mark, or helper text (`Neutral`) in secondary text
    /// with no mark. Validation is a role, never a colour of its own.
    pub fn form_message(&self, role: Role) -> super::StatusStyle {
        super::StatusStyle {
            colour: self.colour(form_message(role)),
            mark: super::status_mark(role),
        }
    }

    /// The colour of the mark of a wrong value: the danger role's text colour, while the value is
    /// wrong and the control enabled (a disabled control is not judged).
    fn invalid_mark(&self, states: States) -> Option<Color> {
        (states.invalid && !states.disabled).then(|| self.colour(REQUIRED))
    }

    /// A switch: `states.checked` is on.
    pub fn switch(&self, density: Density, states: States) -> SwitchStyle {
        let (colours, current) = self.interactive(states, |s| switch(states.checked, s));
        SwitchStyle {
            colours,
            current,
            on: states.checked,
            label: self.colour(label(states)),
            focus: self.ring(C::Border(B::Focus)),
            focus_visible: states.shows_focus(),
            geometry: geometry::switch(&self.foundation, density),
            responds: states.responds(),
        }
    }
}

/// Selected text in a field.
const SELECTION: TreatmentRef =
    TreatmentRef::new(C::Surface(S::Selection), C::Text(T::OnSelection), C::None);
/// The required mark: the danger role's text colour.
const REQUIRED: C = C::Treatment(
    Role::Danger,
    Emphasis::Subtle,
    State::Default,
    Part::Foreground,
);

const fn label(states: States) -> C {
    if states.disabled {
        C::Text(T::Disabled)
    } else {
        C::Text(T::Primary)
    }
}

pub(crate) fn button(a: ButtonAppearance, state: State) -> TreatmentRef {
    if a.link {
        let text = match state {
            State::Disabled => C::Text(T::Disabled),
            _ if a.role == Role::Accent => C::Text(T::Link),
            _ => C::Treatment(a.role, Emphasis::Subtle, State::Default, Part::Foreground),
        };
        return TreatmentRef::new(C::None, text, C::None);
    }
    let base = TreatmentRef::role(a.role, a.emphasis, state);
    if !a.outline {
        return base;
    }
    base.with_border(match (state, a.role) {
        (State::Disabled, _) => C::Border(B::Disabled),
        (State::Default, Role::Neutral) => C::Border(B::Strong),
        (_, Role::Neutral) => C::Border(B::Hover),
        (_, role) => C::Focus(role),
    })
}

fn button_geometry(
    f: &FoundationTokens,
    density: Density,
    layout: ButtonLayout,
) -> (ControlGeometry, Option<f32>) {
    let mut g = geometry::control(f, density, TypeRole::Button);
    if layout != ButtonLayout::IconOnly {
        return (g, None);
    }
    g.padding_x = ((g.height - g.icon_size) / 2.0 - g.border_width).max(0.0);
    (g, Some(g.height))
}

/// A field's box. Its edge, by precedence: disabled, then a wrong value (which keeps its edge
/// under the pointer and in focus), then read-only, then the pointer.
pub(crate) fn field(states: States, state: State) -> TreatmentRef {
    let disabled = TreatmentRef::role(Role::Neutral, Emphasis::Muted, State::Disabled);
    let border = match state {
        State::Disabled => return disabled,
        _ if states.invalid => C::Border(B::Danger),
        _ if states.read_only => C::Border(B::Default),
        State::Default => C::Border(B::Strong),
        State::Hover | State::Pressed => C::Border(B::Hover),
    };
    TreatmentRef::new(C::Surface(S::Recessed), C::Text(T::Primary), border)
}

const fn field_focus_border(states: States) -> C {
    if states.disabled {
        C::Treatment(
            Role::Neutral,
            Emphasis::Muted,
            State::Disabled,
            Part::Border,
        )
    } else if states.invalid {
        C::Border(B::Danger)
    } else {
        C::Border(B::Focus)
    }
}

/// A field's icon, and a select's indicator: tertiary, primary while its list is open.
const fn field_icon(states: States) -> C {
    if states.disabled {
        C::Text(T::Disabled)
    } else if states.expanded {
        C::Text(T::Primary)
    } else {
        C::Text(T::Tertiary)
    }
}

/// Text beside the value inside a field: quieter than the value, readable as text.
const fn field_addon(states: States) -> C {
    if states.disabled {
        C::Text(T::Disabled)
    } else {
        C::Text(T::Tertiary)
    }
}

/// A message's colour: its role's text colour, or secondary text for helper text.
const fn form_message(role: Role) -> C {
    match role {
        Role::Neutral => C::Text(T::Secondary),
        role => super::feedback::status_ink(role),
    }
}

/// A field action's box: the field's height less its borders and the thumb gap on each side,
/// with the button's own type centred in it, and the padding of the density one step tighter,
/// whose controls are about its height. Its icon is the field's density's.
fn field_action_geometry(
    f: &FoundationTokens,
    density: Density,
    layout: ButtonLayout,
) -> (ControlGeometry, Option<f32>) {
    let field = geometry::control(f, density, TypeRole::Body);
    let mut g = geometry::control(f, density, TypeRole::Button);
    g.height = field.height - 2.0 * (field.border_width + f.geometry.thumb_gap);
    g.padding_x = f.controls.for_density(density.smaller()).padding_x;
    g.padding_y = geometry::centred(g.height, g.line_height, g.border_width);
    if layout != ButtonLayout::IconOnly {
        return (g, None);
    }
    g.padding_x = ((g.height - g.icon_size) / 2.0 - g.border_width).max(0.0);
    (g, Some(g.height))
}

/// A check box's box. Ticked or indeterminate, it is the accent's solid fill with its label as
/// the mark and the accent's focus colour as its edge, so it shows against any surface even where
/// the fill is close to it.
pub(crate) fn checkbox(check: Check, invalid: bool, state: State) -> TreatmentRef {
    let edge = |normal: C| {
        if invalid && state != State::Disabled {
            C::Border(B::Danger)
        } else {
            normal
        }
    };
    match (check, state) {
        (Check::Unchecked, State::Disabled) => {
            let d = TreatmentRef::role(Role::Neutral, Emphasis::Muted, State::Disabled);
            TreatmentRef::new(d.background, C::None, d.border)
        }
        (Check::Unchecked, State::Default) => {
            TreatmentRef::new(C::Surface(S::Recessed), C::None, edge(C::Border(B::Strong)))
        }
        (Check::Unchecked, _) => {
            TreatmentRef::new(C::Surface(S::Recessed), C::None, edge(C::Border(B::Hover)))
        }
        (_, State::Disabled) => {
            let d = TreatmentRef::role(Role::Accent, Emphasis::Solid, State::Disabled);
            TreatmentRef::new(d.background, C::Text(T::Disabled), d.border)
        }
        (_, state) => TreatmentRef::role(Role::Accent, Emphasis::Solid, state)
            .with_border(edge(C::Focus(Role::Accent))),
    }
}

/// A switch's track and thumb. Off, the track is the neutral muted fill with a strong edge and
/// the thumb its text colour; on, the accent's solid fill with its label as the thumb.
pub(crate) fn switch(on: bool, state: State) -> TreatmentRef {
    match (on, state) {
        (false, State::Disabled) => {
            let d = TreatmentRef::role(Role::Neutral, Emphasis::Muted, State::Disabled);
            TreatmentRef::new(d.background, C::Text(T::Disabled), d.border)
        }
        (true, State::Disabled) => {
            let d = TreatmentRef::role(Role::Accent, Emphasis::Solid, State::Disabled);
            TreatmentRef::new(d.background, C::Text(T::Disabled), d.border)
        }
        (false, state) => {
            let edge = if state == State::Default {
                C::Border(B::Strong)
            } else {
                C::Border(B::Hover)
            };
            TreatmentRef::role(Role::Neutral, Emphasis::Muted, state).with_border(edge)
        }
        (true, state) => TreatmentRef::role(Role::Accent, Emphasis::Solid, state)
            .with_border(C::Focus(Role::Accent)),
    }
}

fn summary(g: ControlGeometry) -> Summary {
    Summary {
        height: Some(g.height),
        shape: g.shape(),
        type_role: Some(g.type_role),
    }
}

fn button_summary(f: &FoundationTokens, d: Density) -> Summary {
    summary(button_geometry(f, d, ButtonLayout::Text).0)
}

fn field_summary(f: &FoundationTokens, d: Density) -> Summary {
    summary(geometry::control(f, d, TypeRole::Body))
}

fn field_action_summary(f: &FoundationTokens, d: Density) -> Summary {
    summary(field_action_geometry(f, d, ButtonLayout::IconOnly).0)
}

fn check_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = geometry::check(f, d);
    Summary {
        height: Some(g.size),
        shape: g.shape,
        type_role: Some(g.label),
    }
}

fn radio_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = geometry::radio(f, d);
    Summary {
        height: Some(g.size),
        shape: g.shape,
        type_role: Some(g.label),
    }
}

fn form_summary(f: &FoundationTokens, _: Density) -> Summary {
    Summary {
        height: None,
        shape: Shape::Rect {
            radius: f.geometry.radius.none,
        },
        type_role: Some(TypeRole::BodyStrong),
    }
}

fn switch_summary(f: &FoundationTokens, d: Density) -> Summary {
    let g = geometry::switch(f, d);
    Summary {
        height: Some(g.height),
        shape: Shape::Pill,
        type_role: Some(g.label),
    }
}

/// Every button appearance: the eight variants, then every role at every emphasis, with and
/// without an edge, and every role as a link.
fn appearances() -> Vec<(String, ButtonAppearance)> {
    let mut all: Vec<_> = ButtonVariant::ALL
        .iter()
        .map(|v| (v.name().to_owned(), v.appearance()))
        .collect();
    for role in Role::ALL {
        for emphasis in Emphasis::ALL {
            let plain = ButtonAppearance::new(role, emphasis);
            all.push((plain.name(), plain));
            all.push((plain.outlined().name(), plain.outlined()));
        }
        let link = ButtonAppearance::new(role, Emphasis::Subtle).linked();
        all.push((link.name(), link));
    }
    all
}

pub(crate) fn catalogue(out: &mut Vec<Entry>) {
    use Measure::{Edge, Mark, Text};
    for (name, a) in appearances() {
        let t = Template::new("button", name, button_summary).role(a.role, Some(a.emphasis));
        let measures: &'static [Measure] = if a.outline { &[Text, Edge] } else { &[Text] };
        t.states(out, measures, |s| button(a, s));
        t.focus(out, focus_of(a.role));
    }

    // A read-only field's edge is decoration: it shows the value cannot be changed.
    let fields: [(&str, States, &'static [Measure]); 3] = [
        ("normal", States::NONE, &[Text, Edge]),
        ("invalid", States::NONE.invalid(true), &[Text, Edge]),
        ("read-only", States::NONE.read_only(true), &[Text]),
    ];
    for (name, states, measures) in fields {
        let t = Template::new("field", name, field_summary);
        t.states(out, measures, |s| field(states, s));
        let placeholder =
            TreatmentRef::new(C::Surface(S::Recessed), C::Text(T::Placeholder), C::None);
        t.entry(out, "placeholder", &[Text], placeholder);
        t.focus(out, field_focus_border(states.focused(true)));
    }
    let t = Template::new("field", "normal", field_summary);
    t.entry(out, "selection", &[Text], SELECTION);
    t.entry(
        out,
        "required",
        &[Text],
        TreatmentRef::new(C::None, REQUIRED, C::None),
    );
    t.entry(
        out,
        "icon",
        &[Mark],
        TreatmentRef::new(C::Surface(S::Recessed), field_icon(States::NONE), C::None),
    );
    t.entry(
        out,
        "addon",
        &[Text],
        TreatmentRef::new(C::Surface(S::Recessed), field_addon(States::NONE), C::None),
    );
    t.entry(
        out,
        "invalid mark",
        &[Text],
        TreatmentRef::new(C::Surface(S::Recessed), REQUIRED, C::None),
    );
    let t = Template::new("select", "indicator", field_summary);
    for (state, states) in [
        ("default", States::NONE),
        ("expanded", States::NONE.expanded(true)),
    ] {
        let indicator = TreatmentRef::new(C::Surface(S::Recessed), field_icon(states), C::None);
        t.entry(out, state, &[Mark], indicator);
    }

    // A field action is a ghost button drawn on the field's own fill.
    let ghost = ButtonVariant::Ghost.appearance();
    let t = Template::new("field action", "ghost", field_action_summary)
        .role(ghost.role, Some(ghost.emphasis));
    t.states(out, &[Text], |s| button(ghost, s));
    t.focus(out, focus_of(ghost.role));

    for (name, checked, invalid) in [
        ("unselected", false, false),
        ("selected", true, false),
        ("unselected invalid", false, true),
        ("selected invalid", true, true),
    ] {
        let check = if checked {
            Check::Checked
        } else {
            Check::Unchecked
        };
        let t = Template::new("radio", name, radio_summary);
        let measures: &'static [Measure] = if checked { &[Mark, Edge] } else { &[Edge] };
        t.states(out, measures, |s| checkbox(check, invalid, s));
        t.focus(
            out,
            if invalid {
                C::Border(B::Danger)
            } else {
                C::Border(B::Focus)
            },
        );
    }

    let t = Template::new("form field", "", form_summary).fixed_size();
    let text = |c: C| TreatmentRef::new(C::None, c, C::None);
    t.entry(out, "label", &[Text], text(label(States::NONE)));
    t.entry(out, "description", &[Text], text(C::Text(T::Secondary)));
    t.entry(out, "required", &[Text], text(REQUIRED));
    t.entry(out, "optional", &[Text], text(C::Text(T::Tertiary)));
    t.entry(out, "disabled", &[], text(C::Text(T::Disabled)));
    for role in [Role::Neutral, Role::Success, Role::Warning, Role::Danger] {
        Template::new("form message", role.to_string(), form_summary)
            .role(role, None)
            .fixed_size()
            .entry(out, "default", &[Text], text(form_message(role)));
    }

    for (name, check, invalid) in [
        ("unchecked", Check::Unchecked, false),
        ("checked", Check::Checked, false),
        ("indeterminate", Check::Indeterminate, false),
        ("unchecked invalid", Check::Unchecked, true),
        ("checked invalid", Check::Checked, true),
    ] {
        let t = Template::new("checkbox", name, check_summary);
        let measures: &'static [Measure] = if check == Check::Unchecked {
            &[Edge]
        } else {
            &[Text, Edge]
        };
        t.states(out, measures, |s| checkbox(check, invalid, s));
        t.focus(
            out,
            if invalid {
                C::Border(B::Danger)
            } else {
                C::Border(B::Focus)
            },
        );
    }

    for (name, on) in [("off", false), ("on", true)] {
        let t = Template::new("switch", name, switch_summary);
        t.states(out, &[Mark, Edge], |s| switch(on, s));
        t.focus(out, C::Border(B::Focus));
    }
}
