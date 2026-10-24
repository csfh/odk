use std::cell::Cell;
use std::rc::Rc;

use gpui::{ParentElement, Styled, TestAppContext, px};
use odk_a11y::{Describe, Role};

use odk_theme::component::{BadgeAppearance, BadgeStyle, status_mark};
use odk_theme::foundation::CornerRadius;
use odk_theme::semantic::{Emphasis, Role as Intent};

use super::{FIXTURES, open, publish_fixture, redraw};
use crate::{Badge, Density, Tone, h_flex, v_flex};

/// Items 14, 15: a badge is text named by its label.
#[test]
fn reports_its_label() {
    let a = Badge::new("3 new").tone(Tone::Info).accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Text, "3 new"));
}

/// Item 32: changing a badge's variant or tone, as when a status changes, never changes its
/// size.
#[gpui::test]
fn no_variant_or_tone_changes_its_size(cx: &mut TestAppContext) {
    let look = Rc::new(Cell::new(0usize));
    let l = look.clone();
    let cx = open(cx, move |_, _| {
        let badge = Badge::new("Synced");
        match l.get() {
            0 => badge,
            1 => badge.role(Intent::Accent).emphasis(Emphasis::Solid),
            2 => {
                badge.appearance(BadgeAppearance::new(Intent::Neutral, Emphasis::Subtle).outlined())
            }
            3 => badge.tone(Tone::Success),
            4 => badge.tone(Tone::Danger),
            5 => badge.role(Intent::Accent).emphasis(Emphasis::Solid),
            6 => badge.role(Intent::Warning).emphasis(Emphasis::Subtle),
            _ => badge.role(Intent::Info).outlined(),
        }
    });
    let size = cx.debug_bounds("odk-badge").unwrap().size;
    for n in 1..8 {
        look.set(n);
        redraw(cx);
        assert_eq!(cx.debug_bounds("odk-badge").unwrap().size, size, "look {n}");
    }
}

/// Item 23: a long label in a narrow column stays on one line, as wide as its label, and the
/// badge is not stretched across the column.
#[gpui::test]
fn a_long_label_stays_on_one_line(cx: &mut TestAppContext) {
    let long = Rc::new(Cell::new(false));
    let l = long.clone();
    let cx = open(cx, move |_, _| {
        let label = if l.get() {
            "Synchronisierung ausstehend – 1.284 Dateien"
        } else {
            "Synced"
        };
        v_flex().w(px(120.0)).child(Badge::new(label))
    });
    let short = cx.debug_bounds("odk-badge").unwrap();
    assert!(
        short.size.width < px(120.0),
        "as wide as its label: {:?}",
        short.size
    );
    long.set(true);
    redraw(cx);
    let wide = cx.debug_bounds("odk-badge").unwrap();
    assert_eq!(wide.size.height, short.size.height, "still one line");
    assert!(wide.size.width > short.size.width);
}

fn drawn(label: &str) -> BadgeStyle {
    crate::drawn::last::<BadgeStyle>(&format!("odk-badge/{label}"))
}

/// Items 1, 18, 34: every Role at every Emphasis, with and without an edge, draws exactly its
/// lookup in every fixture; in a monochrome theme every badge is grey and a marked one keeps its
/// mark, a different one for each status.
#[gpui::test]
fn every_role_and_emphasis_draws_its_tokens(cx: &mut TestAppContext) {
    let looks: Vec<(String, BadgeAppearance)> = Intent::ALL
        .into_iter()
        .flat_map(|role| {
            Emphasis::ALL.into_iter().flat_map(move |e| {
                let plain = BadgeAppearance::new(role, e);
                [
                    (format!("{role} {}", e.name()), plain),
                    (format!("{role} {} outline", e.name()), plain.outlined()),
                ]
            })
        })
        .collect();
    let shown = looks.clone();
    let cx = open(cx, move |_, _| {
        h_flex().flex_wrap().children(
            shown
                .iter()
                .map(|(label, a)| Badge::new(label.clone()).appearance(*a).marked()),
        )
    });
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        for (label, a) in &looks {
            let got = drawn(label);
            assert_eq!(got, c.badge(*a), "{theme} {label}");
            if theme == "vantablack" {
                for colour in [
                    got.colours.background,
                    got.colours.foreground,
                    got.colours.border,
                ] {
                    assert!(colour.to_oklch().c <= odk_theme::JND, "{label} has a hue");
                }
            }
        }
    }
    let marks: Vec<_> = [
        Intent::Info,
        Intent::Success,
        Intent::Warning,
        Intent::Danger,
    ]
    .map(|r| status_mark(r).unwrap())
    .into();
    assert_eq!(marks.len(), 4);
    // A tone keeps its meaning.
    assert_eq!(
        BadgeAppearance::from(Tone::Warning),
        BadgeAppearance::new(Intent::Warning, Emphasis::Muted)
    );
}

/// A badge is as high as a line of small text and its borders at every density, with the
/// application's corners; an icon or a status mark is the text's size, a gap before the label.
#[gpui::test]
fn a_badge_is_its_text_high_with_the_application_s_corners(cx: &mut TestAppContext) {
    let cx = open(cx, move |_, _| {
        h_flex()
            .child(Badge::new("Failed").tone(Tone::Danger).marked())
            .child(Badge::new("3"))
    });
    for d in Density::ALL {
        cx.update(|_, cx| crate::configure(cx, |c| c.density = d));
        redraw(cx);
        let g = drawn("3").geometry;
        assert_eq!(g.height, 18.0 + 2.0, "{d:?}: not a control's height");
        assert_eq!(
            cx.debug_bounds("odk-badge/icon").unwrap().size.width,
            gpui::px(g.icon_size)
        );
    }
    for (corner, radius) in [(CornerRadius::Square, 0.0), (CornerRadius::Subtle, 2.0)] {
        cx.update(|_, cx| crate::configure(cx, |c| c.corner_radius = corner));
        redraw(cx);
        assert_eq!(drawn("3").geometry.radius, radius);
    }
}
