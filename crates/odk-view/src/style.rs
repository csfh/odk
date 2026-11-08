//! Component tokens for the Rune host, from the same theme snapshot GPUI still draws.

use std::sync::Arc;

use odk_theme::ThemeSnapshot;
use odk_theme::component::ComponentTokens;
use odk_theme::foundation::{FoundationConfig, FoundationTokens, TypeRole, TypeStyle};
use rune::TextStyle;

pub(crate) struct Paint {
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
        tokens: ComponentTokens::new(
            Arc::new(foundation),
            theme.semantic.clone(),
            theme.generation,
        ),
        button,
        body,
    }
}

pub(crate) fn text_style(style: TypeStyle) -> TextStyle {
    TextStyle::new(style.size, style.weight.value())
}
