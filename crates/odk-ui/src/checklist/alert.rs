use gpui::{ParentElement, TestAppContext};
use odk_a11y::{Describe, Role};

use odk_theme::component::AlertStyle;
use odk_theme::semantic::{Emphasis, Role as Intent};

use super::{Count, FIXTURES, open, press, publish_fixture, redraw};
use crate::{Alert, Button, Size, Tone, v_flex};

/// Items 8, 10: the dismiss button and the action are reachable with Tab and pressed with
/// Enter.
#[gpui::test]
fn its_buttons_work_from_the_keyboard(cx: &mut TestAppContext) {
    let (acted, dismissed) = (Count::default(), Count::default());
    let (a, d) = (acted.clone(), dismissed.clone());
    let cx = open(cx, move |_, _| {
        let (a, d) = (a.clone(), d.clone());
        Alert::new("x", "Disk almost full")
            .variant(Tone::Warning)
            .action(
                Button::new("clean")
                    .label("Clean up")
                    .size(Size::Sm)
                    .on_click(move |_, _, _| a.bump()),
            )
            .on_dismiss(move |_, _, _| d.bump())
    });
    press(cx, "tab enter tab enter");
    assert_eq!((acted.get(), dismissed.get()), (1, 1));
}

/// Items 14, 15: an alert is an Alert named by its title and description.
#[test]
fn reports_its_title_and_description() {
    let a = Alert::new("x", "Saved")
        .description("All changes kept.")
        .accessible();
    assert_eq!(
        (a.role, a.name.as_str()),
        (Role::Alert, "Saved: All changes kept.")
    );
}

/// Item 18: every tone has its own mark, so colour is not the only signal.
#[test]
fn every_tone_has_its_own_mark() {
    let marks = [Tone::Info, Tone::Success, Tone::Warning, Tone::Danger].map(Tone::mark);
    for (i, m) in marks.iter().enumerate() {
        assert!(!m.is_empty());
        assert!(!marks[i + 1..].contains(m), "two tones share the mark {m}");
    }
}

/// Item 23: long, empty and non-Latin text renders.
#[gpui::test]
fn renders_odd_text(cx: &mut TestAppContext) {
    open(cx, |_, _| {
        v_flex()
            .child(Alert::new("a", ""))
            .child(Alert::new("b", "x".repeat(500)).description("y ".repeat(500)))
            .child(Alert::new("c", "ディスクの空き容量が少なくなっています").description("مرحبا"))
    });
}

const TONES: [Tone; 4] = [Tone::Info, Tone::Success, Tone::Warning, Tone::Danger];

/// Items 1, 18, 34: every tone at every emphasis draws exactly its lookup in every fixture, muted
/// by default; in a monochrome theme every alert is grey and still has its own mark and words.
#[gpui::test]
fn every_tone_draws_its_tokens(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex().children(TONES.into_iter().flat_map(|tone| {
            [None, Some(Emphasis::Solid), Some(Emphasis::Subtle)].map(move |e| {
                let id = gpui::SharedString::from(format!("{tone:?}-{e:?}"));
                let alert = Alert::new(id, "Disk almost full").variant(tone);
                match e {
                    Some(e) => alert.emphasis(e),
                    None => alert,
                }
            })
        }))
    });
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        for tone in TONES {
            for (e, emphasis) in [
                (None, Emphasis::Muted),
                (Some(Emphasis::Solid), Emphasis::Solid),
                (Some(Emphasis::Subtle), Emphasis::Subtle),
            ] {
                let (got, compact) =
                    crate::drawn::last::<(AlertStyle, bool)>(&format!("{tone:?}-{e:?}"));
                assert_eq!(
                    got,
                    c.alert(Intent::from(tone), emphasis),
                    "{theme} {tone:?}"
                );
                assert!(!compact);
                if theme == "vantablack" {
                    for colour in [got.surface.background, got.surface.border, got.mark] {
                        assert!(colour.to_oklch().c <= odk_theme::JND, "{tone:?} has a hue");
                    }
                }
            }
        }
    }
}

/// A compact alert has the tighter padding of its style, and its dismiss button and a small
/// action sit with their centres on the title's line.
#[gpui::test]
fn a_compact_alert_and_its_buttons_line_up(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        v_flex()
            .child(
                Alert::new("a", "Saved")
                    .compact(true)
                    .action(Button::new("undo").label("Undo").size(Size::Sm))
                    .on_dismiss(|_, _, _| {}),
            )
            .child(Alert::new("b", "Saved").on_dismiss(|_, _, _| {}))
    });
    let (style, _) = crate::drawn::last::<(AlertStyle, bool)>("a");
    let (a, b) = (
        cx.debug_bounds("a/alert").unwrap(),
        cx.debug_bounds("b/alert").unwrap(),
    );
    assert!(a.size.height < b.size.height, "compact is tighter");
    let text = cx.debug_bounds("a/text").unwrap();
    assert_eq!(
        text.top(),
        a.top() + gpui::px(style.compact.padding_y + style.compact.border_width)
    );
    let title_centre = text.top() + gpui::px(10.0);
    for part in ["a/dismiss/button", "undo/button"] {
        let b = cx.debug_bounds(part).unwrap();
        assert_eq!(b.origin.y + b.size.height / 2.0, title_centre, "{part}");
        assert!(
            b.top() >= a.top() && b.bottom() <= a.bottom(),
            "{part} stays inside"
        );
    }
}
