//! Component tokens for the Rune host, from the same theme snapshot GPUI still draws.

use std::sync::Arc;

use odk_theme::component::{ComponentTokens, FocusRing};
use odk_theme::foundation::{FoundationConfig, FoundationTokens, TypeRole, TypeStyle};
use odk_theme::{TextScale, ThemeSnapshot};
use rune::{El, TextStyle};

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
    /// `points` at the base text size, scaled by the user's text size, as a control's own width
    /// is. A component reads the theme here and nowhere else.
    pub fn px(&self, points: f32) -> f32 {
        self.scale.px(points)
    }
}

pub(crate) fn text_style(style: TypeStyle) -> TextStyle {
    TextStyle::new(style.size, style.weight.value())
}

/// `element`'s focus ring, as the component layer sets it: drawn when the keyboard has focus on
/// the control, and nothing else moves.
pub(crate) fn focus_ring(element: El, ring: &FocusRing) -> El {
    element.focus_ring(ring.width, ring.offset, ink(ring.colour))
}
