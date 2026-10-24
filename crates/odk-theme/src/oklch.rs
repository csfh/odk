//! OKLab and OKLCH (Björn Ottosson, 2020, as CSS Color 4 defines them): the perceptual space the
//! semantic resolver compares and adjusts colours in (`docs/spec/odk-semantic-tokens.md`).
//!
//! Distances in OKLab follow how different two colours look far better than distances in sRGB,
//! and OKLCH separates lightness (`l`), colourfulness (`c`) and hue (`h`), so the resolver can
//! make a colour lighter without changing its hue. Arithmetic is done in `f64`; the public types
//! hold `f32` like [`Color`].

use crate::Color;
use crate::color::{linear_to_srgb, srgb_to_linear};

/// The OKLab distance of a just-noticeable difference, roughly: CSS Color 4 uses 0.02 as the
/// tolerance of its gamut mapping. Every perceptual step of the resolver is a multiple of it.
pub const JND: f32 = 0.02;

/// A colour in OKLab: perceptual lightness `l` from 0 (black) to 1 (white), and the two
/// opponent axes `a` (green to red) and `b` (blue to yellow).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

/// A colour in OKLCH: lightness `l` as in OKLab, chroma `c` (0 for a grey) and hue `h` in
/// degrees, `0.0..360.0`. The hue of a grey means nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

impl Color {
    /// The colour in OKLab, ignoring alpha.
    pub fn to_oklab(self) -> Oklab {
        let [r, g, b] = [self.r, self.g, self.b].map(|c| srgb_to_linear(f64::from(c)));
        let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
        let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
        let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
        let [l, m, s] = [l, m, s].map(f64::cbrt);
        Oklab {
            l: (0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s) as f32,
            a: (1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s) as f32,
            b: (0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s) as f32,
        }
    }

    /// The colour in OKLCH, ignoring alpha.
    pub fn to_oklch(self) -> Oklch {
        self.to_oklab().to_oklch()
    }
}

impl Oklab {
    pub fn to_oklch(self) -> Oklch {
        let c = f64::from(self.a).hypot(f64::from(self.b));
        let h = f64::from(self.b)
            .atan2(f64::from(self.a))
            .to_degrees()
            .rem_euclid(360.0);
        Oklch {
            l: self.l,
            c: c as f32,
            // `rem_euclid` can round up to exactly 360.
            h: if h >= 360.0 { 0.0 } else { h as f32 },
        }
    }

    /// The perceptual distance between two colours, ΔE in OKLab: Euclidean, so a step in
    /// lightness and a step in hue of the same size count the same.
    pub fn distance(self, other: Oklab) -> f32 {
        let d = |x: f32, y: f32| f64::from(x) - f64::from(y);
        let (dl, da, db) = (d(self.l, other.l), d(self.a, other.a), d(self.b, other.b));
        (dl * dl + da * da + db * db).sqrt() as f32
    }
}

impl Color {
    /// CIE 1976 lightness L*, divided by 100: 0 for black, 1 for white, and linear in
    /// luminance near black, where the eye barely tells dark greys apart.
    pub fn lightness(self) -> f32 {
        const EDGE: f64 = 216.0 / 24389.0;
        const SLOPE: f64 = 24389.0 / 27.0;
        let y = f64::from(self.luminance());
        let l = if y <= EDGE {
            SLOPE * y
        } else {
            116.0 * y.cbrt() - 16.0
        };
        (l / 100.0) as f32
    }

    /// How different two opaque colours look side by side: [`Oklab::distance`], with OKLab's
    /// lightness replaced by [`Color::lightness`]. OKLab's lightness overstates differences
    /// between very dark colours (`#010101` is 0.067 from black there, and 0.003 here), so a
    /// tint measured with it would vanish on a black canvas. From mid grey up a step in L* is
    /// a little larger than the same step in OKLab, by a fifth to a third. The resolver
    /// measures every perceptual step and every change with this.
    pub fn apparent_distance(self, other: Color) -> f32 {
        let (a, b) = (self.to_oklab(), other.to_oklab());
        let with = |lab: Oklab, l: f32| Oklab { l, ..lab };
        with(a, self.lightness()).distance(with(b, other.lightness()))
    }
}

impl Oklab {
    /// The colour in linear-light sRGB, each channel unclamped: outside `0.0..=1.0` when sRGB
    /// cannot show it.
    fn to_linear_srgb(self) -> [f64; 3] {
        let (l, a, b) = (f64::from(self.l), f64::from(self.a), f64::from(self.b));
        let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
        let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
        let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
        let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
        [
            4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
            -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
            -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
        ]
    }
}

/// How far outside `0.0..=1.0` a linear channel may be and still count as in gamut: rounding in
/// the conversion, far below one 8-bit step.
const GAMUT_EPSILON: f64 = 1e-6;
/// Halvings of the chroma when mapping into gamut: enough to place it far below one 8-bit step.
const GAMUT_STEPS: u32 = 32;

impl Oklch {
    pub fn to_oklab(self) -> Oklab {
        let (c, h) = (f64::from(self.c), f64::from(self.h).to_radians());
        Oklab {
            l: self.l,
            a: (c * h.cos()) as f32,
            b: (c * h.sin()) as f32,
        }
    }

    /// Whether sRGB can show this colour as it is.
    pub fn in_gamut(self) -> bool {
        (0.0..=1.0).contains(&self.l)
            && self
                .to_oklab()
                .to_linear_srgb()
                .iter()
                .all(|c| (-GAMUT_EPSILON..=1.0 + GAMUT_EPSILON).contains(c))
    }

    /// The colour in sRGB, opaque, or `None` when sRGB cannot show it.
    pub fn to_color(self) -> Option<Color> {
        self.in_gamut()
            .then(|| encode(self.to_oklab().to_linear_srgb()))
    }

    /// The colour in sRGB, opaque, brought into gamut the way CSS Color 4 recommends: lightness
    /// and hue are kept and chroma is reduced, by bisection, to the most sRGB can show at that
    /// lightness and hue. A lightness at or beyond white or black is white or black. No channel
    /// is clamped beyond conversion rounding, far below one 8-bit step. A part that is not a number
    /// counts as 0, so the result is always a colour.
    pub fn to_color_mapped(self) -> Color {
        let number = |v: f32| if v.is_nan() { 0.0 } else { v };
        let lch = Self {
            l: number(self.l),
            c: number(self.c).max(0.0),
            h: if self.h.is_finite() { self.h } else { 0.0 },
        };
        if lch.l >= 1.0 {
            return Color::from_rgba8([255; 4]);
        }
        if lch.l <= 0.0 {
            return Color::BLACK;
        }
        if let Some(colour) = lch.to_color() {
            return colour;
        }
        let (mut inside, mut outside) = (0.0_f32, lch.c.min(1.0));
        for _ in 0..GAMUT_STEPS {
            let mid = (inside + outside) / 2.0;
            let candidate = Self { c: mid, ..lch };
            if candidate.in_gamut() {
                inside = mid;
            } else {
                outside = mid;
            }
        }
        encode(Self { c: inside, ..lch }.to_oklab().to_linear_srgb())
    }
}

/// Linear-light channels as an opaque sRGB colour, clamping only the rounding that
/// [`GAMUT_EPSILON`] allows.
fn encode(linear: [f64; 3]) -> Color {
    let [r, g, b] = linear.map(|c| linear_to_srgb(c.clamp(0.0, 1.0)) as f32);
    Color { r, g, b, a: 1.0 }
}

/// How far apart two hues are around the circle, in degrees: `0.0..=180.0`.
pub fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Color {
        Color::parse_hex(s).unwrap()
    }

    /// Reference values published with CSS Color 4 and its sample code (also what colorjs.io
    /// and oklch.com print), independent of this module's matrices.
    #[test]
    fn known_colours_have_their_published_oklch_values() {
        let cases = [
            ("#ff0000", 0.627_955, 0.257_683, 29.233_885),
            ("#00ff00", 0.866_440, 0.294_827, 142.4953),
            ("#0000ff", 0.452_014, 0.313_214, 264.052),
        ];
        for (colour, l, c, h) in cases {
            let got = hex(colour).to_oklch();
            assert!((got.l - l).abs() < 1e-4, "{colour} l {}", got.l);
            assert!((got.c - c).abs() < 1e-4, "{colour} c {}", got.c);
            assert!((got.h - h).abs() < 0.05, "{colour} h {}", got.h);
        }
        let white = hex("#ffffff").to_oklch();
        assert!((white.l - 1.0).abs() < 1e-5 && white.c < 1e-4, "{white:?}");
        let black = Color::BLACK.to_oklch();
        assert!(black.l.abs() < 1e-6 && black.c < 1e-6, "{black:?}");
    }

    #[test]
    fn greys_have_no_chroma() {
        for v in (0..=u8::MAX).step_by(5) {
            let grey = Color::from_rgba8([v, v, v, 255]).to_oklch();
            assert!(grey.c < 1e-4, "{v}: {grey:?}");
        }
    }

    /// Every colour on a lattice of the 8-bit cube comes back to the same bytes.
    #[test]
    fn srgb_survives_a_round_trip_through_oklch() {
        for r in (0..=255u8).step_by(17) {
            for g in (0..=255u8).step_by(17) {
                for b in (0..=255u8).step_by(17) {
                    let colour = Color::from_rgba8([r, g, b, 255]);
                    let lch = colour.to_oklch();
                    let back = lch.to_color().unwrap_or_else(|| panic!("{lch:?} in gamut"));
                    assert_eq!(back.to_rgba8(), colour.to_rgba8(), "{lch:?}");
                    let mapped = lch.to_color_mapped();
                    assert_eq!(mapped.to_rgba8(), colour.to_rgba8(), "{lch:?}");
                }
            }
        }
    }

    #[test]
    fn oklab_and_oklch_convert_both_ways() {
        let lab = hex("#7aa2f7").to_oklab();
        let back = lab.to_oklch().to_oklab();
        assert!(lab.distance(back) < 1e-6, "{lab:?} {back:?}");
    }

    /// Published CIE L* values: mid grey `#808080` is 53.59, `#777777` is 50.03.
    #[test]
    fn lightness_is_cie_l_star() {
        let close = |c: &str, want: f32| {
            let got = hex(c).lightness() * 100.0;
            assert!((got - want).abs() < 0.02, "{c}: {got}");
        };
        close("#000000", 0.0);
        close("#ffffff", 100.0);
        close("#808080", 53.59);
        close("#777777", 50.03);
        // Linear near black.
        let one = Color::from_rgba8([1, 1, 1, 255]).lightness();
        let two = Color::from_rgba8([2, 2, 2, 255]).lightness();
        assert!((two / one - 2.0).abs() < 1e-3);
    }

    #[test]
    fn apparent_distance_does_not_overstate_steps_near_black() {
        let black = Color::BLACK;
        let step = Color::from_rgba8([1, 1, 1, 255]);
        assert!(step.to_oklab().distance(black.to_oklab()) > 0.06);
        assert!(step.apparent_distance(black) < 0.005);
        // From mid grey up a step in L* is a little larger than in OKLab.
        for (from, to) in [("#808080", "#8c8c8c"), ("#f0f0f0", "#ffffff")] {
            let (from, to) = (hex(from), hex(to));
            let ratio = to.apparent_distance(from) / to.to_oklab().distance(from.to_oklab());
            assert!((1.0..1.4).contains(&ratio), "{ratio}");
        }
        let grey = hex("#808080");
        // Hue still counts: two colours of one lightness are apart.
        assert!(hex("#ff0000").apparent_distance(hex("#0000ff")) > 0.3);
        assert_eq!(grey.apparent_distance(grey), 0.0);
    }

    #[test]
    fn hue_distance_wraps_around_the_circle() {
        assert_eq!(hue_distance(350.0, 10.0), 20.0);
        assert_eq!(hue_distance(10.0, 350.0), 20.0);
        assert_eq!(hue_distance(0.0, 180.0), 180.0);
        assert_eq!(hue_distance(90.0, 90.0), 0.0);
        assert_eq!(hue_distance(-30.0, 30.0), 60.0);
        assert_eq!(hue_distance(720.0, 0.0), 0.0);
    }

    #[test]
    fn distance_is_euclidean_and_lightness_dominates_between_greys() {
        let black = Color::BLACK.to_oklab();
        let white = hex("#ffffff").to_oklab();
        assert!((black.distance(white) - 1.0).abs() < 1e-4);
        assert_eq!(white.distance(white), 0.0);
        assert_eq!(black.distance(white), white.distance(black));
    }

    #[test]
    fn out_of_gamut_colours_lose_chroma_and_keep_lightness_and_hue() {
        // A green more colourful than sRGB can show.
        let wanted = Oklch {
            l: 0.7,
            c: 0.4,
            h: 150.0,
        };
        assert!(!wanted.in_gamut());
        assert_eq!(wanted.to_color(), None);
        let mapped = wanted.to_color_mapped();
        assert!(mapped.is_in_gamut());
        let got = mapped.to_oklch();
        assert!((got.l - wanted.l).abs() < 1e-4, "{got:?}");
        assert!(hue_distance(got.h, wanted.h) < 0.1, "{got:?}");
        assert!(got.c < wanted.c && got.c > 0.1, "{got:?}");
        // It is the most chroma sRGB has there: a little more is out of gamut.
        let more = Oklch {
            c: got.c + 0.002,
            ..got
        };
        assert!(!more.in_gamut());
    }

    #[test]
    fn lightness_beyond_white_or_black_maps_to_white_or_black() {
        let bright = Oklch {
            l: 1.2,
            c: 0.1,
            h: 30.0,
        };
        assert_eq!(bright.to_color_mapped().to_hex(), "#ffffff");
        let dark = Oklch {
            l: -0.1,
            c: 0.1,
            h: 30.0,
        };
        assert_eq!(dark.to_color_mapped().to_hex(), "#000000");
        let nan = Oklch {
            l: f32::NAN,
            c: 0.1,
            h: 30.0,
        };
        assert!(nan.to_color_mapped().is_in_gamut(), "never a NaN channel");
        for odd in [
            Oklch {
                l: 0.5,
                c: f32::NAN,
                h: 30.0,
            },
            Oklch {
                l: 0.5,
                c: 0.1,
                h: f32::NAN,
            },
            Oklch {
                l: 0.5,
                c: f32::INFINITY,
                h: 30.0,
            },
        ] {
            assert!(odd.to_color_mapped().is_in_gamut(), "{odd:?}");
        }
    }
}
