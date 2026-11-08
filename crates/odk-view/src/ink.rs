//! ODK's gamma-encoded sRGB as Rune's linear colour.

use odk_theme::Color;
use rune::color;

/// The Rune colour for an ODK colour. The hex is the 8-bit value ODK stores, which Rune decodes.
pub fn ink(color: Color) -> rune::Color {
    color::hex(&color.to_hex()).expect("ODK's hex is #rrggbb or #rrggbbaa")
}
