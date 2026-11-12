//! Component tokens for the Rune host, from the same theme snapshot GPUI still draws.

use std::sync::Arc;

use odk_theme::component::{ComponentTokens, FocusRing};
use odk_theme::foundation::{FontFamily, FoundationConfig, FoundationTokens, TypeRole, TypeStyle};
use odk_theme::{TextScale, ThemeSnapshot};
use rune::{El, Family, TextStyle, Transition};

use crate::ink::ink;

pub(crate) struct Paint {
    scale: TextScale,
    pub tokens: ComponentTokens,
    pub button: TypeStyle,
    pub body: TypeStyle,
}

/// The tokens a host draws with: its theme's colours and text size, and its own foundation
/// settings (density, corners, the application's choice of reduced motion) with the user's request
/// for reduced motion added.
pub(crate) fn paint<T>(hosted: &crate::host::Hosted<T>) -> Paint {
    paint_for(hosted.theme(), hosted.config())
}

pub(crate) fn paint_for(theme: &ThemeSnapshot, config: &FoundationConfig) -> Paint {
    let config = FoundationConfig {
        reduced_motion: theme.reduced_motion || config.reduced_motion,
        ..config.clone()
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

    /// `element` as something that is mounted and removed (checklist items 20 and 21): with
    /// Rune's own fade, which keeps a removed element drawn until its exit has run; and with
    /// motion reduced, shown and gone at once with everything inside it, since Rune turns any
    /// other transition into a fade under reduced motion and a fade is not "its end state at
    /// once".
    pub fn present(&self, element: El) -> El {
        if self.tokens.foundation().motion.reduced {
            element.transition(Transition::none()).instant_changes()
        } else {
            element
        }
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

    /// Item 11: a ring is the token's width, offset and colour, outside the part, with the part's
    /// corner grown by what it stands off and is wide, and it moves nothing.
    #[test]
    fn a_focus_ring_is_the_tokens_width_offset_and_colour_outside_the_part() {
        use crate::host::{Hosted, headless};
        use rune::{Cx, Key, KeyEvent, el};

        fn view(_: &Hosted<()>, _: &mut Cx<Hosted<()>>) -> El {
            let ring = FocusRing {
                colour: odk_theme::Color::from_rgba8([0x3b, 0x82, 0xf6, 0xff]),
                width: 2.0,
                offset: 1.5,
            };
            rune::col().padding(20.0).items_start().child(focus_ring(
                el().key("part")
                    .size(40.0, 20.0)
                    .radius(6.0)
                    .fill(ink(odk_theme::Color::from_rgba8([0x20, 0x20, 0x20, 0xff])))
                    .focusable(true),
                &ring,
            ))
        }
        let mut runtime = headless(
            Hosted::new((), ThemeSnapshot::emergency(), crate::face::Face::bundled()),
            view,
            (200.0, 100.0),
        );
        runtime.set_tab_visits_controls(true);
        let rings = |rt: &rune::Runtime<Hosted<()>>| {
            rt.instances
                .iter()
                .filter(|i| i.fill[3] == 0.0 && i.border[3] > 0.0 && i.params[0] > 0.0)
                .map(|i| (i.rect, i.params[0], i.radii[0]))
                .collect::<Vec<_>>()
        };
        runtime.frame(0.0);
        assert!(
            rings(&runtime).is_empty(),
            "no ring before the keyboard has focus"
        );
        let at_rest = runtime.find("part").expect("the part");
        runtime.key(KeyEvent::from(Key::Tab));
        for _ in 0..30 {
            runtime.frame(1.0 / 60.0);
        }
        let drawn = rings(&runtime);
        assert_eq!(drawn.len(), 1, "one ring: {drawn:?}");
        let (rect, width, radius) = drawn[0];
        let out = 2.0 + 1.5;
        assert_eq!(
            rect,
            [20.0 - out, 20.0 - out, 40.0 + 2.0 * out, 20.0 + 2.0 * out]
        );
        assert_eq!(width, 2.0, "token width");
        assert_eq!(radius, 6.0 + out, "the part's corner, grown");
        assert_eq!(
            runtime.find("part"),
            Some(at_rest),
            "and the part has not moved"
        );
    }
}
