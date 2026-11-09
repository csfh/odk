//! Component tokens for the Rune host, from the same theme snapshot GPUI still draws.

use std::sync::Arc;

use odk_theme::component::{ComponentTokens, FocusRing};
use odk_theme::foundation::{FontFamily, FoundationConfig, FoundationTokens, TypeRole, TypeStyle};
use odk_theme::{TextScale, ThemeSnapshot};
use rune::{El, Family, TextStyle};

use crate::ink::ink;

pub(crate) struct Paint {
    scale: TextScale,
    pub tokens: ComponentTokens,
    pub button: TypeStyle,
    pub body: TypeStyle,
}

pub(crate) fn paint(theme: &ThemeSnapshot) -> Paint {
    let config = FoundationConfig {
        reduced_motion: theme.reduced_motion,
        ..FoundationConfig::default()
    };
    let foundation = FoundationTokens::scaled(&config, theme.text);
    let button = foundation.typography.style(TypeRole::Button);
    let body = foundation.typography.style(TypeRole::Body);
    Paint {
        scale: theme.text,
        tokens: ComponentTokens::new(
            Arc::new(foundation),
            theme.semantic.clone(),
            theme.generation,
        ),
        button,
        body,
    }
}

impl Paint {
    /// How the foundation sets text in `role`.
    pub fn role(&self, role: TypeRole) -> TypeStyle {
        self.tokens.foundation().typography.style(role)
    }

    /// `points` at the base text size, scaled by the user's text size, as a control's own width
    /// is. A component reads the theme here and nowhere else.
    pub fn px(&self, points: f32) -> f32 {
        self.scale.px(points)
    }
}

/// Rune's text style for a type role: its size and weight, its family (Rune's mono slot for the
/// monospace role), its line height and its letter spacing.
pub(crate) fn text_style(style: TypeStyle) -> TextStyle {
    TextStyle {
        family: match style.family {
            FontFamily::Ui => Family::Sans,
            FontFamily::Mono => Family::Mono,
        },
        line_height: style.line_height,
        tracking: style.letter_spacing,
        ..TextStyle::new(style.size, style.weight.value())
    }
}

/// `element`'s focus ring, as the component layer sets it: drawn when the keyboard has focus on
/// the control, and nothing else moves.
pub(crate) fn focus_ring(element: El, ring: &FocusRing) -> El {
    element.focus_ring(ring.width, ring.offset, ink(ring.colour))
}

/// `element`'s focus ring drawn inside it, for an element a scrolling or clipping parent would
/// cut the ring of: a tab in a list that scrolls.
pub(crate) fn inset_focus_ring(element: El, ring: &FocusRing) -> El {
    element.focus_ring(ring.width, -ring.width, ink(ring.colour))
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_theme::foundation::TypeRole;

    /// The test face names one font for both of Rune's slots, so a family is checked here and
    /// not by a width.
    #[test]
    fn a_type_role_reaches_rune_with_its_family_line_height_and_weight() {
        let foundation = FoundationTokens::scaled(
            &FoundationConfig::default(),
            ThemeSnapshot::emergency().text,
        );
        for role in TypeRole::ALL {
            let style = foundation.typography.style(role);
            let rune = text_style(style);
            assert_eq!(rune.size, style.size, "{role:?}");
            assert_eq!(rune.line_height, style.line_height, "{role:?}");
            assert_eq!(rune.weight, style.weight.value(), "{role:?}");
            let mono = matches!(rune.family, Family::Mono);
            assert_eq!(mono, role == TypeRole::Mono, "{role:?}");
        }
    }
}
