/// An sRGB colour with straight alpha, every channel in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Parses `#RRGGBB` or `#RRGGBBAA`. Anything else, such as a Hyprland gradient, is `None`.
    pub fn from_hex(text: &str) -> Option<Self> {
        Self::parse_hex(text).ok()
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA`, in either case, ignoring whitespace around it, and says
    /// what is wrong with anything else. The channels are exact: [`Color::to_rgba8`] gives back
    /// the bytes that were written.
    pub fn parse_hex(text: &str) -> Result<Self, ColorError> {
        let hex = text.trim().strip_prefix('#').ok_or(ColorError::NoHash)?;
        if let Some(c) = hex.chars().find(|c| !c.is_ascii_hexdigit()) {
            return Err(ColorError::Digit(c));
        }
        if !(hex.len() == 6 || hex.len() == 8) {
            return Err(ColorError::Length(hex.len()));
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("checked hex digits");
        let alpha = if hex.len() == 8 { byte(6) } else { u8::MAX };
        Ok(Self::from_rgba8([byte(0), byte(2), byte(4), alpha]))
    }

    /// The colour of four bytes, red, green, blue and alpha.
    pub fn from_rgba8([r, g, b, a]: [u8; 4]) -> Self {
        let channel = |v: u8| f32::from(v) / 255.0;
        Self {
            r: channel(r),
            g: channel(g),
            b: channel(b),
            a: channel(a),
        }
    }

    /// The four bytes of this colour, each channel rounded to the nearest of 256 steps. Exact
    /// for a colour made by [`Color::parse_hex`] or [`Color::from_rgba8`].
    pub fn to_rgba8(self) -> [u8; 4] {
        let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [
            channel(self.r),
            channel(self.g),
            channel(self.b),
            channel(self.a),
        ]
    }

    /// `#rrggbb`, or `#rrggbbaa` when the colour is not opaque, in lower case.
    pub fn to_hex(self) -> String {
        match self.to_rgba8() {
            [r, g, b, 255] => format!("#{r:02x}{g:02x}{b:02x}"),
            [r, g, b, a] => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        }
    }

    /// The same colour with its alpha replaced.
    pub fn with_alpha(self, a: f32) -> Self {
        Self {
            a: a.clamp(0.0, 1.0),
            ..self
        }
    }

    /// `0xRRGGBBAA`, the form GPUI's `rgba` takes.
    pub fn to_rgba_u32(self) -> u32 {
        u32::from_be_bytes(self.to_rgba8())
    }

    /// This colour drawn over an opaque `background`, as one opaque colour.
    pub fn over(self, background: Color) -> Self {
        let blend = |top: f32, bottom: f32| top * self.a + bottom * (1.0 - self.a);
        Self {
            r: blend(self.r, background.r),
            g: blend(self.g, background.g),
            b: blend(self.b, background.b),
            a: 1.0,
        }
    }

    /// The colour `t` of the way from this one to `other`, `t` in `0.0..=1.0`.
    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }

    /// The WCAG contrast ratio between two opaque colours, from 1 to 21.
    pub fn contrast(self, other: Color) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Relative luminance, 0 for black and 1 for white. WCAG 2's formula, whose cutoff of
    /// 0.03928 differs from the sRGB standard's 0.04045 ([`srgb_to_linear`]) only between two
    /// 8-bit steps, so the two agree on every colour a theme can write.
    pub fn luminance(self) -> f32 {
        let linear = |c: f32| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// The colour rounded to the 8-bit channels it reaches the screen with. Contrast is judged
    /// on this, so a colour that passes is still passing once GPUI draws it.
    pub fn quantized(self) -> Self {
        Self::from_rgba8(self.to_rgba8())
    }

    /// Whether every channel is a number in `0.0..=1.0`: a colour sRGB can show as it is.
    pub fn is_in_gamut(self) -> bool {
        [self.r, self.g, self.b, self.a]
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    }

    /// Whether the two have the same 8-bit channels.
    pub fn same_rgba8(self, other: Color) -> bool {
        self.to_rgba8() == other.to_rgba8()
    }
}

/// WCAG 2.2's least contrast for normal text (1.4.3, level AA). ODK holds every label to it,
/// whatever its size or density.
pub const TEXT_CONTRAST: f32 = 4.5;
/// WCAG 2.2's least contrast for large text (1.4.3): 18 pt, or 14 pt bold.
pub const LARGE_TEXT_CONTRAST: f32 = 3.0;
/// WCAG 2.2's least contrast for what identifies a control or its state, and for a focus
/// indicator (1.4.11, level AA).
pub const NON_TEXT_CONTRAST: f32 = 3.0;

/// The contrast of `foreground` drawn on `background`, both drawn over the opaque `surface`
/// beneath: the colours as they reach the screen, not as they are written. A translucent
/// colour is composited the way GPUI does, in gamma-encoded sRGB (`docs/checks.md`).
pub fn drawn_contrast(foreground: Color, background: Color, surface: Color) -> f32 {
    let background = background.over(surface);
    foreground.over(background).contrast(background)
}

/// An sRGB channel, gamma-encoded as written, in linear light (IEC 61966-2-1).
pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A linear-light channel gamma-encoded for sRGB, the inverse of [`srgb_to_linear`].
pub fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Why a text is not a colour [`Color::parse_hex`] accepts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ColorError {
    #[error("a colour is written #RRGGBB or #RRGGBBAA, starting with `#`")]
    NoHash,
    #[error("{0:?} is not a hex digit")]
    Digit(char),
    #[error("a colour has 6 or 8 hex digits after `#`, not {0}")]
    Length(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_channel_value_survives_a_round_trip() {
        for v in 0..=u8::MAX {
            let bytes = [v, v.wrapping_add(85), v.wrapping_add(170), v];
            assert_eq!(Color::from_rgba8(bytes).to_rgba8(), bytes, "{v}");
            let hex = format!("#{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], v);
            assert_eq!(Color::parse_hex(&hex).unwrap().to_rgba8(), bytes, "{hex}");
        }
    }

    #[test]
    fn upper_and_lower_case_are_the_same_colour() {
        assert_eq!(Color::parse_hex("#7AA2F7"), Color::parse_hex("#7aa2f7"));
        assert_eq!(Color::parse_hex("#7aA2f7").unwrap().to_hex(), "#7aa2f7");
    }

    #[test]
    fn whitespace_around_a_colour_is_ignored_and_inside_it_is_not() {
        assert_eq!(Color::parse_hex("  #1a1b26\t").unwrap().to_hex(), "#1a1b26");
        assert_eq!(Color::parse_hex("#1a1b 26"), Err(ColorError::Digit(' ')));
        assert_eq!(Color::parse_hex("# 1a1b26"), Err(ColorError::Digit(' ')));
    }

    #[test]
    fn says_what_is_wrong() {
        assert_eq!(Color::parse_hex("1a1b26"), Err(ColorError::NoHash));
        assert_eq!(Color::parse_hex("red"), Err(ColorError::NoHash));
        assert_eq!(
            Color::parse_hex("rgba(26a269ee) rgba(2ec27eee) 45deg"),
            Err(ColorError::NoHash)
        );
        assert_eq!(Color::parse_hex("#12345g"), Err(ColorError::Digit('g')));
        assert_eq!(Color::parse_hex("#fff"), Err(ColorError::Length(3)));
        assert_eq!(Color::parse_hex("#ffff"), Err(ColorError::Length(4)));
        assert_eq!(Color::parse_hex("#fffffff"), Err(ColorError::Length(7)));
        assert_eq!(Color::parse_hex("#ffffffff0"), Err(ColorError::Length(9)));
        assert_eq!(Color::parse_hex("#"), Err(ColorError::Length(0)));
        assert_eq!(Color::parse_hex("#ffffé0"), Err(ColorError::Digit('é')));
    }

    #[test]
    fn to_hex_writes_alpha_only_when_there_is_some() {
        assert_eq!(Color::parse_hex("#00000080").unwrap().to_hex(), "#00000080");
        assert_eq!(Color::parse_hex("#000000ff").unwrap().to_hex(), "#000000");
        assert_eq!(Color::BLACK.to_hex(), "#000000");
    }

    #[test]
    fn parses_hex_with_and_without_alpha() {
        assert_eq!(
            Color::from_hex("#ffffff"),
            Some(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0
            })
        );
        let c = Color::from_hex("#00000080").unwrap();
        assert!((c.a - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(
            Color::from_hex("#D6A63C").unwrap().to_rgba_u32(),
            0xD6A63CFF
        );
    }

    #[test]
    fn rejects_everything_else() {
        for bad in [
            "",
            "ffffff",
            "#fff",
            "#gggggg",
            "#ffffffff0",
            "rgba(255,0,0,1) rgba(0,0,0,1) 45deg",
            "red",
        ] {
            assert_eq!(Color::from_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn alpha_is_clamped() {
        assert_eq!(Color::BLACK.with_alpha(2.0).a, 1.0);
        assert_eq!(Color::BLACK.with_alpha(-1.0).a, 0.0);
    }

    #[test]
    fn luminance_orders_black_below_white() {
        assert!(Color::BLACK.luminance() < Color::from_hex("#ffffff").unwrap().luminance());
    }

    fn hex(s: &str) -> Color {
        Color::parse_hex(s).unwrap()
    }

    /// Published WCAG reference pairs: black on white is the most there is, `#767676` is the
    /// lightest grey that passes for text on white and `#777777` the darkest that does not.
    #[test]
    fn contrast_matches_known_wcag_values() {
        let white = hex("#ffffff");
        let close = |pair: (&str, &str), want: f32| {
            let got = hex(pair.0).contrast(hex(pair.1));
            assert!((got - want).abs() < 0.01, "{pair:?}: {got}");
        };
        close(("#000000", "#ffffff"), 21.0);
        close(("#ffffff", "#ffffff"), 1.0);
        close(("#767676", "#ffffff"), 4.54);
        close(("#777777", "#ffffff"), 4.48);
        close(("#ff0000", "#ffffff"), 4.0);
        close(("#0000ff", "#ffffff"), 8.59);
        close(("#ffff00", "#ffffff"), 1.07);
        assert!(hex("#767676").contrast(white) >= TEXT_CONTRAST);
        assert!(hex("#777777").contrast(white) < TEXT_CONTRAST);
        assert_eq!(white.contrast(Color::BLACK), Color::BLACK.contrast(white));
    }

    #[test]
    fn the_transfer_functions_are_the_standard_s_and_invert_each_other() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-12);
        assert!((srgb_to_linear(0.5) - 0.214_041_14).abs() < 1e-6);
        assert!(
            (srgb_to_linear(0.04) - 0.04 / 12.92).abs() < 1e-12,
            "linear below the cutoff"
        );
        for v in 0..=u8::MAX {
            let c = f64::from(v) / 255.0;
            assert!((linear_to_srgb(srgb_to_linear(c)) - c).abs() < 1e-9, "{v}");
        }
    }

    /// WCAG 2's cutoff (0.03928) and the sRGB standard's (0.04045) fall between the same two
    /// 8-bit steps, so `luminance` agrees with the transfer function the OKLab conversion uses
    /// on every colour a theme can write.
    #[test]
    fn wcag_luminance_agrees_with_the_srgb_transfer_on_every_8_bit_value() {
        for v in 0..=u8::MAX {
            let grey = Color::from_rgba8([v, v, v, 255]);
            let standard = srgb_to_linear(f64::from(v) / 255.0) as f32;
            assert!((grey.luminance() - standard).abs() < 1e-6, "{v}");
        }
    }

    #[test]
    fn translucent_colours_are_composited_before_contrast_is_measured() {
        let (white, black) = (hex("#ffffff"), Color::BLACK);
        let half = white.with_alpha(0.5).over(black);
        assert!(
            (half.r - 0.5).abs() < 1e-6 && half.a == 1.0,
            "blended in gamma space"
        );
        // Black text on a white fill at half opacity over black is on mid grey, not on white.
        let drawn = drawn_contrast(black, white.with_alpha(0.5), black);
        assert!((drawn - half.contrast(black)).abs() < 1e-6);
        assert!(drawn < 6.0, "{drawn}");
        assert!((drawn_contrast(black, white, black) - 21.0).abs() < 1e-4);
        // Text at half opacity is composited too.
        let faint = drawn_contrast(white.with_alpha(0.5), black, black);
        assert!((faint - half.contrast(black)).abs() < 1e-6);
    }

    #[test]
    fn quantized_rounds_to_the_drawn_bytes_and_gamut_is_checked() {
        let c = Color {
            r: 0.5,
            g: 0.123_456,
            b: 1.0,
            a: 1.0,
        };
        assert_eq!(c.quantized().to_rgba8(), c.to_rgba8());
        assert!(c.same_rgba8(c.quantized()));
        assert!(c.is_in_gamut());
        assert!(!Color { r: 1.01, ..c }.is_in_gamut());
        assert!(!Color { g: f32::NAN, ..c }.is_in_gamut());
    }
}
