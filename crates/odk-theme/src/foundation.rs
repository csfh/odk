//! The foundation layer of ADR 0040: ODK's own geometry, density, spacing, type and motion,
//! defined in `docs/spec/odk-foundation-tokens.md`. Nothing here comes from the Omarchy theme
//! and nothing here is a colour, so density or corner radius can never change one. The one
//! thing the user adds is their text size (ADR 0043): a [`TextScale`] that multiplies the type
//! sizes, line heights, control metrics and spacing, and nothing else.
//!
//! An application chooses a [`FoundationConfig`]; [`FoundationTokens::from_config`] turns it
//! into the values components draw with, at the scale of 12 px text, which is what the spec's
//! numbers are; [`FoundationTokens::scaled`] at the user's. Every component draws them through the component tokens.

use std::time::Duration;

use crate::TextScale;

// The primitive values. Every token below is built from these, so a number appears once.

/// The step of the spacing scale: `space.N` is `N` steps.
const GRID: f32 = 4.0;
/// The gap between a field's label and its control: an optical exception to the grid.
const FIELD_GAP: f32 = 6.0;
/// The least radius that reads as a corner at scale 1 (ADR 0042).
const RADIUS_SUBTLE: f32 = 2.0;
const BORDER_WIDTH: f32 = 1.0;
const FOCUS_WIDTH: f32 = 2.0;
/// The gap of the surface between a control and its focus indicator.
const FOCUS_OFFSET: f32 = 2.0;
/// The gap between a track's edge and the thumb that moves inside it.
const THUMB_GAP: f32 = 2.0;

const COMPACT: ControlMetrics = ControlMetrics {
    height: 28.0,
    padding_x: 10.0,
    icon_size: 14.0,
    gap: 6.0,
};
const DEFAULT: ControlMetrics = ControlMetrics {
    height: 34.0,
    padding_x: 12.0,
    icon_size: 16.0,
    gap: 8.0,
};
const COMFORTABLE: ControlMetrics = ControlMetrics {
    height: 40.0,
    padding_x: 16.0,
    icon_size: 18.0,
    gap: 8.0,
};

const FAST_MS: u64 = 140;
const NORMAL_MS: u64 = 180;
const SLOW_MS: u64 = 240;
const ENTER_MS: u64 = 240;
const EXIT_MS: u64 = 160;
/// One turn of a loop, the user's choice of 2026-10-23 (*provisional*).
const CYCLE_MS: u64 = 800;
/// How long the pointer rests on a tooltip's or hover card's trigger before it opens, and how long
/// after it leaves before it closes; how long a toast stays. Carried over from the legacy
/// tokens ADR 0047 removed (the overlays' brief, 2026-10-23; *provisional*). They are delays, not
/// transitions: reduced motion does not shorten them.
const HOVER_OPEN_MS: u64 = 500;
const HOVER_CLOSE_MS: u64 = 300;
const TOAST_MS: u64 = 5000;

/// What an application chooses. Everything else in this module follows from it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FoundationConfig {
    pub density: Density,
    pub corner_radius: CornerRadius,
    /// The application's own reduced-motion choice. It adds to the system's preference and
    /// never overrides it.
    pub reduced_motion: bool,
    pub fonts: FontConfig,
}

/// The font families an application asks for. `None` keeps Omarchy's: the face fontconfig's
/// `monospace` alias names, which `omarchy font set` chooses, for both families (ADR 0043). An
/// application that wants GPUI's own UI face names it, `.SystemUIFont`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FontConfig {
    pub ui: Option<String>,
    pub mono: Option<String>,
}

/// The whole foundation layer, built from one [`FoundationConfig`].
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationTokens {
    pub geometry: GeometryTokens,
    pub spacing: SpacingTokens,
    pub layout: LayoutTokens,
    pub typography: TypographyTokens,
    pub motion: MotionTokens,
    pub controls: ControlTokens,
    /// The user's text size these were built at.
    pub text_scale: TextScale,
}

impl FoundationTokens {
    /// The tokens at the spec's own sizes: 12 px text, a scale of 1.
    pub fn from_config(config: &FoundationConfig) -> Self {
        Self::scaled(config, TextScale::default())
    }

    /// The tokens with the type, controls and spacing at the user's text size (ADR 0043).
    /// Borders, the focus ring, the thumb gap, radii and durations are the same at every scale.
    pub fn scaled(config: &FoundationConfig, text: TextScale) -> Self {
        let corner = config.corner_radius.px();
        let spacing = SpacingTokens::SCALE.scaled(text);
        Self {
            geometry: GeometryTokens {
                corner_radius: config.corner_radius,
                radius: RadiusTokens {
                    none: 0.0,
                    default: corner,
                    small: 0.0,
                    overlay: corner,
                },
                border_width: BORDER_WIDTH,
                focus_width: FOCUS_WIDTH,
                focus_offset: FOCUS_OFFSET,
                thumb_gap: THUMB_GAP,
            },
            layout: LayoutTokens {
                panel_padding: spacing.s4,
                dialog_padding: spacing.s6,
                section_gap: spacing.s6,
                form_gap: spacing.s4,
                field_gap: text.px(FIELD_GAP),
                menu_item_gap: spacing.s2,
            },
            spacing,
            typography: TypographyTokens {
                ui_family: config.fonts.ui.clone(),
                mono_family: config.fonts.mono.clone(),
                scale: text,
            },
            motion: MotionTokens {
                instant: Duration::ZERO,
                fast: Duration::from_millis(FAST_MS),
                normal: Duration::from_millis(NORMAL_MS),
                slow: Duration::from_millis(SLOW_MS),
                enter: Duration::from_millis(ENTER_MS),
                exit: Duration::from_millis(EXIT_MS),
                cycle: Duration::from_millis(CYCLE_MS),
                hover_open: Duration::from_millis(HOVER_OPEN_MS),
                hover_close: Duration::from_millis(HOVER_CLOSE_MS),
                toast: Duration::from_millis(TOAST_MS),
                easing: MotionEasing::Standard,
                reduced: config.reduced_motion,
            },
            controls: ControlTokens {
                density: config.density,
                scale: text,
            },
            text_scale: text,
        }
    }
}

impl Default for FoundationTokens {
    fn default() -> Self {
        Self::from_config(&FoundationConfig::default())
    }
}

/// How round corners are, for the whole application. A component never picks its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CornerRadius {
    /// 0 px.
    Square,
    /// 2 px (ADR 0042).
    #[default]
    Subtle,
}

impl CornerRadius {
    pub const fn px(self) -> f32 {
        match self {
            Self::Square => 0.0,
            Self::Subtle => RADIUS_SUBTLE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryTokens {
    /// The application's choice that `radius.default` and `radius.overlay` follow.
    pub corner_radius: CornerRadius,
    pub radius: RadiusTokens,
    pub border_width: f32,
    /// The width of the keyboard focus indicator.
    pub focus_width: f32,
    /// The gap of the surface between a control and its focus indicator, which is drawn
    /// outside the control so a solid fill never meets it.
    pub focus_offset: f32,
    /// The gap between a track's edge and the thumb inside it, as in a switch.
    pub thumb_gap: f32,
}

/// Corner radii, in logical pixels. None is larger than [`CornerRadius::Subtle`].
#[derive(Debug, Clone, PartialEq)]
pub struct RadiusTokens {
    /// Always square.
    pub none: f32,
    /// Controls: buttons, inputs, selects. Follows [`CornerRadius`].
    pub default: f32,
    /// Small marks such as a checkbox or a badge: always square.
    pub small: f32,
    /// Surfaces that float: menus, popovers, tooltips, dialogs. Follows [`CornerRadius`].
    pub overlay: f32,
}

/// The spacing scale, on a 4 px grid. `sN` is the spec's `space.N`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpacingTokens {
    pub s0: f32,
    pub s1: f32,
    pub s2: f32,
    pub s3: f32,
    pub s4: f32,
    pub s5: f32,
    pub s6: f32,
    pub s8: f32,
    pub s12: f32,
}

impl SpacingTokens {
    const SCALE: Self = Self {
        s0: 0.0,
        s1: GRID,
        s2: GRID * 2.0,
        s3: GRID * 3.0,
        s4: GRID * 4.0,
        s5: GRID * 5.0,
        s6: GRID * 6.0,
        s8: GRID * 8.0,
        s12: GRID * 12.0,
    };

    /// Each step at the user's text size, rounded to a whole pixel on its own, so the steps keep
    /// their proportion to the text rather than to each other.
    fn scaled(&self, text: TextScale) -> Self {
        Self {
            s0: self.s0,
            s1: text.px(self.s1),
            s2: text.px(self.s2),
            s3: text.px(self.s3),
            s4: text.px(self.s4),
            s5: text.px(self.s5),
            s6: text.px(self.s6),
            s8: text.px(self.s8),
            s12: text.px(self.s12),
        }
    }
}

/// Spacing by purpose, so a component asks for a dialog's padding rather than a step.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutTokens {
    pub panel_padding: f32,
    pub dialog_padding: f32,
    pub section_gap: f32,
    pub form_gap: f32,
    /// Between a field's label and its control: 6 px, off the grid on purpose.
    pub field_gap: f32,
    pub menu_item_gap: f32,
}

/// How tightly controls are packed. It changes their size and padding, never a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    Compact,
    #[default]
    Default,
    Comfortable,
}

impl Density {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Default, Self::Comfortable];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Default => "default",
            Self::Comfortable => "comfortable",
        }
    }

    pub const fn metrics(self) -> ControlMetrics {
        match self {
            Self::Compact => COMPACT,
            Self::Default => DEFAULT,
            Self::Comfortable => COMFORTABLE,
        }
    }

    /// One step tighter, or `Compact` when already there. A control's `Size::Sm` is this step
    /// below the density it would otherwise take.
    pub const fn smaller(self) -> Self {
        match self {
            Self::Compact | Self::Default => Self::Compact,
            Self::Comfortable => Self::Default,
        }
    }

    /// One step roomier, or `Comfortable` when already there. A control's `Size::Lg` is this
    /// step above the density it would otherwise take.
    pub const fn larger(self) -> Self {
        match self {
            Self::Compact => Self::Default,
            Self::Default | Self::Comfortable => Self::Comfortable,
        }
    }
}

/// The dimensions of one control at one density, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMetrics {
    pub height: f32,
    pub padding_x: f32,
    pub icon_size: f32,
    /// Between an icon and a label inside the control.
    pub gap: f32,
}

impl ControlMetrics {
    /// The metrics at the user's text size, each a whole pixel.
    fn scaled(self, text: TextScale) -> Self {
        Self {
            height: text.px(self.height),
            padding_x: text.px(self.padding_x),
            icon_size: text.px(self.icon_size),
            gap: text.px(self.gap),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlTokens {
    /// The application's density, which a control without its own uses.
    pub density: Density,
    /// The user's text size, which the metrics follow.
    pub scale: TextScale,
}

impl ControlTokens {
    /// The metrics at the application's density.
    pub fn metrics(&self) -> ControlMetrics {
        self.for_density(self.density)
    }

    /// The metrics at `density`, whatever the application chose.
    pub fn for_density(&self, density: Density) -> ControlMetrics {
        density.metrics().scaled(self.scale)
    }

    /// The metrics for a control with its own density, if it has one, else the application's.
    pub fn resolve(&self, own: Option<Density>) -> ControlMetrics {
        self.for_density(own.unwrap_or(self.density))
    }
}

/// A named role for text. Components ask for a role, never for a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRole {
    Caption,
    Small,
    Body,
    BodyStrong,
    Button,
    Subtitle,
    Heading,
    Title,
    Mono,
}

impl TypeRole {
    pub const ALL: [Self; 9] = [
        Self::Caption,
        Self::Small,
        Self::Body,
        Self::BodyStrong,
        Self::Button,
        Self::Subtitle,
        Self::Heading,
        Self::Title,
        Self::Mono,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Caption => "caption",
            Self::Small => "small",
            Self::Body => "body",
            Self::BodyStrong => "body_strong",
            Self::Button => "button",
            Self::Subtitle => "subtitle",
            Self::Heading => "heading",
            Self::Title => "title",
            Self::Mono => "mono",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    Medium,
    Semibold,
}

impl FontWeight {
    /// The CSS and OpenType weight: 400, 500 or 600.
    pub const fn value(self) -> f32 {
        match self {
            Self::Regular => 400.0,
            Self::Medium => 500.0,
            Self::Semibold => 600.0,
        }
    }
}

/// Which of the two configured families a role is set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    Ui,
    Mono,
}

/// How text in one role is set. Sizes are logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeStyle {
    pub family: FontFamily,
    pub size: f32,
    pub line_height: f32,
    pub weight: FontWeight,
    /// Extra space between letters, in logical pixels. Zero for every role; GPUI 0.2.2 cannot
    /// apply anything else (`docs/checks.md`).
    pub letter_spacing: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypographyTokens {
    /// The application's UI family, or `None` for Omarchy's.
    pub ui_family: Option<String>,
    /// The application's monospace family, or `None` for Omarchy's.
    pub mono_family: Option<String>,
    /// The user's text size, which the sizes and line heights follow.
    pub scale: TextScale,
}

impl TypographyTokens {
    pub fn style(&self, role: TypeRole) -> TypeStyle {
        use FontWeight::{Medium, Regular, Semibold};
        let (family, size, line_height, weight) = match role {
            TypeRole::Caption => (FontFamily::Ui, 11.0, 16.0, Regular),
            TypeRole::Small => (FontFamily::Ui, 12.0, 18.0, Regular),
            TypeRole::Body => (FontFamily::Ui, 13.0, 20.0, Regular),
            TypeRole::BodyStrong => (FontFamily::Ui, 13.0, 20.0, Medium),
            TypeRole::Button => (FontFamily::Ui, 13.0, 18.0, Medium),
            TypeRole::Subtitle => (FontFamily::Ui, 15.0, 22.0, Medium),
            TypeRole::Heading => (FontFamily::Ui, 18.0, 26.0, Semibold),
            TypeRole::Title => (FontFamily::Ui, 22.0, 30.0, Semibold),
            TypeRole::Mono => (FontFamily::Mono, 12.0, 18.0, Regular),
        };
        TypeStyle {
            family,
            size: self.scale.px(size),
            line_height: self.scale.px(line_height),
            weight,
            letter_spacing: 0.0,
        }
    }

    /// The family the application chose for `family`, or `None` to keep the system's.
    pub fn family(&self, family: FontFamily) -> Option<&str> {
        match family {
            FontFamily::Ui => self.ui_family.as_deref(),
            FontFamily::Mono => self.mono_family.as_deref(),
        }
    }
}

/// A named duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDuration {
    Instant,
    Fast,
    Normal,
    Slow,
    /// Something appearing.
    Enter,
    /// Something going away.
    Exit,
}

/// How a transition moves from start to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotionEasing {
    /// `cubic-bezier(0.22, 1, 0.36, 1)`: fast out of the start, settling gently.
    #[default]
    Standard,
    Linear,
}

impl MotionEasing {
    /// Progress at time `t`, both from 0 to 1. GPUI's `Animation::with_easing` takes this.
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Self::Standard => CubicBezier::STANDARD.apply(t),
            Self::Linear => t.clamp(0.0, 1.0),
        }
    }
}

/// A CSS `cubic-bezier` timing curve. GPUI has none, so ODK evaluates it itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const STANDARD: Self = Self {
        x1: 0.22,
        y1: 1.0,
        x2: 0.36,
        y2: 1.0,
    };

    /// Progress at time `t`, both from 0 to 1: finds where the curve's x is `t` and returns
    /// its y there. Newton's method first, then bisection if that does not settle, so the
    /// answer is the same on every call.
    pub fn apply(self, t: f32) -> f32 {
        let t = f64::from(t.clamp(0.0, 1.0));
        if t <= 0.0 || t >= 1.0 {
            return t as f32;
        }
        // x must stay between 0 and 1 for the curve to be a function of time.
        let (x1, x2) = (
            f64::from(self.x1.clamp(0.0, 1.0)),
            f64::from(self.x2.clamp(0.0, 1.0)),
        );
        let (y1, y2) = (f64::from(self.y1), f64::from(self.y2));
        // Each axis as a polynomial a·s³ + b·s² + c·s of the curve parameter s.
        let (cx, cy) = (3.0 * x1, 3.0 * y1);
        let (bx, by) = (3.0 * (x2 - x1) - cx, 3.0 * (y2 - y1) - cy);
        let (ax, ay) = (1.0 - cx - bx, 1.0 - cy - by);
        let x = |s: f64| ((ax * s + bx) * s + cx) * s;
        let slope = |s: f64| (3.0 * ax * s + 2.0 * bx) * s + cx;
        let y = |s: f64| (((ay * s + by) * s + cy) * s) as f32;
        const EPSILON: f64 = 1e-9;

        let mut s = t;
        for _ in 0..8 {
            let error = x(s) - t;
            if error.abs() < EPSILON {
                return y(s);
            }
            let d = slope(s);
            if d.abs() < EPSILON {
                break;
            }
            s -= error / d;
        }
        let (mut low, mut high) = (0.0, 1.0);
        s = t;
        for _ in 0..64 {
            let error = x(s) - t;
            if error.abs() < EPSILON {
                break;
            }
            if error < 0.0 {
                low = s;
            } else {
                high = s;
            }
            s = (low + high) / 2.0;
        }
        y(s)
    }
}

/// Durations and easing. With `reduced` set, [`MotionTokens::transition`] is zero: a
/// component shows its end state at once, every state change still happens, an indicator of
/// slow work stays visible but stands still, and no key waits for an animation.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionTokens {
    pub instant: Duration,
    pub fast: Duration,
    pub normal: Duration,
    pub slow: Duration,
    pub enter: Duration,
    pub exit: Duration,
    /// One turn of something that loops while work goes on, such as a spinner. Not a
    /// transition, so not a `MotionDuration`; nothing loops when motion is reduced.
    pub cycle: Duration,
    /// The pointer's rest on a tooltip's trigger before it opens (*provisional*).
    pub hover_open: Duration,
    /// How long after the pointer leaves a tooltip or hover card before it closes
    /// (*provisional*).
    pub hover_close: Duration,
    /// How long a toast stays, unless it says otherwise (*provisional*).
    pub toast: Duration,
    pub easing: MotionEasing,
    /// Whether to skip non-essential motion: the application's choice or the system's.
    pub reduced: bool,
}

impl MotionTokens {
    /// The token's own value, whether or not motion is reduced.
    pub fn duration(&self, duration: MotionDuration) -> Duration {
        match duration {
            MotionDuration::Instant => self.instant,
            MotionDuration::Fast => self.fast,
            MotionDuration::Normal => self.normal,
            MotionDuration::Slow => self.slow,
            MotionDuration::Enter => self.enter,
            MotionDuration::Exit => self.exit,
        }
    }

    /// How long a transition runs: the token's value, or zero when motion is reduced.
    pub fn transition(&self, duration: MotionDuration) -> Duration {
        if self.reduced {
            Duration::ZERO
        } else {
            self.duration(duration)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(change: impl FnOnce(&mut FoundationConfig)) -> FoundationTokens {
        let mut config = FoundationConfig::default();
        change(&mut config);
        FoundationTokens::from_config(&config)
    }

    #[test]
    fn the_default_configuration_is_default_density_subtle_corners_and_full_motion() {
        let config = FoundationConfig::default();
        assert_eq!(config.density, Density::Default);
        assert_eq!(config.corner_radius, CornerRadius::Subtle);
        assert!(!config.reduced_motion);
        assert_eq!(config.fonts, FontConfig::default());
        assert_eq!(
            FoundationTokens::default(),
            FoundationTokens::from_config(&config)
        );
    }

    /// The user's text size (ADR 0043) scales text, controls and spacing together, and nothing
    /// about how a control is drawn: not its borders, its focus ring, its corners or its motion.
    #[test]
    fn the_text_size_scales_type_controls_and_spacing_and_nothing_else() {
        let config = FoundationConfig::default();
        let plain = FoundationTokens::from_config(&config);
        let big = FoundationTokens::scaled(&config, TextScale::from_base_size(18.0));
        assert_eq!(plain.text_scale, TextScale::default());
        assert_eq!(big.text_scale.base_size(), 18.0);

        // 1.5 times, for every length that is text or sits around it.
        for role in TypeRole::ALL {
            let (a, b) = (plain.typography.style(role), big.typography.style(role));
            assert_eq!(b.size, (a.size * 1.5).round(), "{role:?}");
            assert_eq!(b.line_height, (a.line_height * 1.5).round(), "{role:?}");
            assert_eq!((b.family, b.weight), (a.family, a.weight));
        }
        for density in Density::ALL {
            let (a, b) = (
                plain.controls.for_density(density),
                big.controls.for_density(density),
            );
            assert_eq!(b.height, (a.height * 1.5).round(), "{density:?}");
            assert_eq!(b.padding_x, (a.padding_x * 1.5).round());
            assert_eq!(b.icon_size, (a.icon_size * 1.5).round());
            assert_eq!(b.gap, (a.gap * 1.5).round());
        }
        assert_eq!(
            big.controls.metrics(),
            big.controls.for_density(Density::Default)
        );
        assert_eq!(big.spacing.s4, 24.0);
        assert_eq!(big.layout.dialog_padding, big.spacing.s6);
        assert_eq!(big.layout.field_gap, 9.0);

        // None of how it is drawn.
        assert_eq!(big.geometry, plain.geometry);
        assert_eq!(big.motion, plain.motion);
        assert_eq!(big.controls.density, plain.controls.density);
    }

    #[test]
    fn the_spec_s_own_sizes_are_those_of_12_px_text() {
        let config = FoundationConfig::default();
        assert_eq!(
            FoundationTokens::scaled(&config, TextScale::from_base_size(12.0)),
            FoundationTokens::from_config(&config)
        );
    }

    #[test]
    fn corners_are_exactly_zero_or_exactly_two() {
        assert_eq!(CornerRadius::Square.px(), 0.0);
        assert_eq!(CornerRadius::Subtle.px(), 2.0);
        let r = tokens(|_| {}).geometry.radius;
        assert_eq!(
            (r.none, r.default, r.small, r.overlay),
            (0.0, 2.0, 0.0, 2.0)
        );
        let r = tokens(|c| c.corner_radius = CornerRadius::Square)
            .geometry
            .radius;
        assert_eq!(
            (r.none, r.default, r.small, r.overlay),
            (0.0, 0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn no_radius_is_larger_than_subtle() {
        for corner in [CornerRadius::Square, CornerRadius::Subtle] {
            for density in [Density::Compact, Density::Default, Density::Comfortable] {
                let g = tokens(|c| {
                    c.corner_radius = corner;
                    c.density = density;
                })
                .geometry;
                assert_eq!(g.corner_radius, corner);
                let r = g.radius;
                for radius in [r.none, r.default, r.small, r.overlay] {
                    assert!((0.0..=2.0).contains(&radius), "{radius} under {corner:?}");
                }
            }
        }
    }

    #[test]
    fn borders_are_one_pixel_and_the_focus_indicator_two() {
        let g = tokens(|_| {}).geometry;
        assert_eq!(g.border_width, 1.0);
        assert_eq!(g.focus_width, 2.0);
        assert_eq!(g.focus_offset, 2.0);
        assert_eq!(g.thumb_gap, 2.0);
    }

    #[test]
    fn each_density_has_its_height_padding_and_icon_size() {
        let size = |d: Density| {
            let m = d.metrics();
            (m.height, m.padding_x, m.icon_size)
        };
        assert_eq!(size(Density::Compact), (28.0, 10.0, 14.0));
        assert_eq!(size(Density::Default), (34.0, 12.0, 16.0));
        assert_eq!(size(Density::Comfortable), (40.0, 16.0, 18.0));
    }

    #[test]
    fn a_control_takes_the_application_density_unless_it_has_its_own() {
        let controls = tokens(|_| {}).controls;
        assert_eq!(controls.metrics().height, 34.0);
        assert_eq!(controls.resolve(None).height, 34.0);
        assert_eq!(controls.resolve(Some(Density::Compact)).height, 28.0);

        let controls = tokens(|c| c.density = Density::Comfortable).controls;
        assert_eq!(controls.metrics().height, 40.0);
        assert_eq!(controls.resolve(None).height, 40.0);
        assert_eq!(controls.resolve(Some(Density::Compact)).height, 28.0);
        assert_eq!(controls.for_density(Density::Default).height, 34.0);
    }

    #[test]
    fn a_density_steps_one_tighter_or_roomier_and_stops_at_the_ends() {
        use Density::{Comfortable, Compact, Default};
        assert_eq!(
            Density::ALL.map(Density::smaller),
            [Compact, Compact, Default]
        );
        assert_eq!(
            Density::ALL.map(Density::larger),
            [Default, Comfortable, Comfortable]
        );
    }

    #[test]
    fn density_changes_only_the_controls() {
        let base = tokens(|_| {});
        for density in [Density::Compact, Density::Comfortable] {
            let other = tokens(|c| c.density = density);
            assert_ne!(other.controls, base.controls);
            assert_eq!(
                FoundationTokens {
                    controls: base.controls.clone(),
                    ..other
                },
                base
            );
        }
    }

    #[test]
    fn the_spacing_scale_is_on_a_4_px_grid() {
        let s = tokens(|_| {}).spacing;
        assert_eq!(
            [s.s0, s.s1, s.s2, s.s3, s.s4, s.s5, s.s6, s.s8, s.s12],
            [0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 48.0]
        );
    }

    #[test]
    fn layout_spacing_has_its_values() {
        let l = tokens(|_| {}).layout;
        assert_eq!(l.panel_padding, 16.0);
        assert_eq!(l.dialog_padding, 24.0);
        assert_eq!(l.section_gap, 24.0);
        assert_eq!(l.form_gap, 16.0);
        assert_eq!(l.field_gap, 6.0);
        assert_eq!(l.menu_item_gap, 8.0);
    }

    #[test]
    fn each_type_role_has_its_size_line_height_and_weight() {
        use FontWeight::{Medium, Regular, Semibold};
        let t = tokens(|_| {}).typography;
        let want = [
            (TypeRole::Caption, 11.0, 16.0, Regular),
            (TypeRole::Small, 12.0, 18.0, Regular),
            (TypeRole::Body, 13.0, 20.0, Regular),
            (TypeRole::BodyStrong, 13.0, 20.0, Medium),
            (TypeRole::Button, 13.0, 18.0, Medium),
            (TypeRole::Subtitle, 15.0, 22.0, Medium),
            (TypeRole::Heading, 18.0, 26.0, Semibold),
            (TypeRole::Title, 22.0, 30.0, Semibold),
            (TypeRole::Mono, 12.0, 18.0, Regular),
        ];
        assert_eq!(want.len(), TypeRole::ALL.len());
        for (role, size, line_height, weight) in want {
            let s = t.style(role);
            assert_eq!(
                (s.size, s.line_height, s.weight),
                (size, line_height, weight)
            );
            assert_eq!(s.letter_spacing, 0.0, "{role:?}");
            let family = if role == TypeRole::Mono {
                FontFamily::Mono
            } else {
                FontFamily::Ui
            };
            assert_eq!(s.family, family, "{role:?}");
        }
        assert_eq!(
            (Regular.value(), Medium.value(), Semibold.value()),
            (400.0, 500.0, 600.0)
        );
    }

    #[test]
    fn fonts_are_the_system_s_unless_the_application_names_them() {
        let t = tokens(|_| {}).typography;
        assert_eq!(t.family(FontFamily::Ui), None);
        assert_eq!(t.family(FontFamily::Mono), None);

        let t = tokens(|c| {
            c.fonts = FontConfig {
                ui: Some("Inter".into()),
                mono: Some("JetBrains Mono".into()),
            }
        })
        .typography;
        assert_eq!(t.family(FontFamily::Ui), Some("Inter"));
        assert_eq!(t.family(FontFamily::Mono), Some("JetBrains Mono"));
        // The roles themselves do not change with the family.
        assert_eq!(
            t.style(TypeRole::Body),
            tokens(|_| {}).typography.style(TypeRole::Body)
        );
    }

    #[test]
    fn each_duration_has_its_value() {
        let m = tokens(|_| {}).motion;
        let ms = |d: MotionDuration| m.duration(d).as_millis();
        assert_eq!(ms(MotionDuration::Instant), 0);
        assert_eq!(ms(MotionDuration::Fast), 140);
        assert_eq!(ms(MotionDuration::Normal), 180);
        assert_eq!(ms(MotionDuration::Slow), 240);
        assert_eq!(ms(MotionDuration::Enter), 240);
        assert_eq!(ms(MotionDuration::Exit), 160);
        assert_eq!(m.cycle.as_millis(), 800, "a loop, not a transition");
        assert_eq!(
            (
                m.hover_open.as_millis(),
                m.hover_close.as_millis(),
                m.toast.as_millis()
            ),
            (500, 300, 5000),
            "delays carried over from the legacy tokens"
        );
        assert_eq!(m.easing, MotionEasing::Standard);
        assert_eq!(m.transition(MotionDuration::Fast), m.fast);
    }

    #[test]
    fn reduced_motion_makes_every_transition_instant_and_keeps_the_values() {
        let m = tokens(|c| c.reduced_motion = true).motion;
        assert!(m.reduced);
        for d in [
            MotionDuration::Instant,
            MotionDuration::Fast,
            MotionDuration::Normal,
            MotionDuration::Slow,
            MotionDuration::Enter,
            MotionDuration::Exit,
        ] {
            assert_eq!(m.transition(d), Duration::ZERO, "{d:?}");
        }
        assert_eq!(m.duration(MotionDuration::Slow).as_millis(), 240);
    }

    /// Reference values solved exactly, with rational arithmetic, outside this code.
    #[test]
    fn the_standard_curve_matches_cubic_bezier_0_22_1_0_36_1() {
        for (t, want) in [
            (0.1, 0.401_097),
            (0.25, 0.764_865),
            (0.5, 0.961_383),
            (0.75, 0.996_894),
            (0.9, 0.999_84),
        ] {
            let got = MotionEasing::Standard.apply(t);
            assert!((got - want).abs() < 1e-5, "at {t}: {got}, not {want}");
            assert_eq!(got, MotionEasing::Standard.apply(t), "deterministic");
        }
    }

    #[test]
    fn easing_starts_at_0_ends_at_1_and_never_goes_back() {
        for easing in [MotionEasing::Standard, MotionEasing::Linear] {
            assert_eq!(easing.apply(0.0), 0.0);
            assert_eq!(easing.apply(1.0), 1.0);
            assert_eq!(easing.apply(-1.0), 0.0);
            assert_eq!(easing.apply(2.0), 1.0);
            let mut last = 0.0;
            for i in 0..=1000 {
                let v = easing.apply(i as f32 / 1000.0);
                assert!((0.0..=1.0).contains(&v) && v >= last, "{easing:?} at {i}");
                last = v;
            }
        }
        assert_eq!(MotionEasing::Linear.apply(0.3), 0.3);
    }
}
