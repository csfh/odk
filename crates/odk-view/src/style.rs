//! Component tokens for the Rune host, from the same theme snapshot GPUI still draws.

use std::sync::Arc;

use odk_theme::component::ComponentTokens;
use odk_theme::foundation::{FoundationConfig, FoundationTokens, TypeRole, TypeStyle};
use odk_theme::{TextScale, ThemeSnapshot};
use rune::TextStyle;

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
