use gpui::{InteractiveElement, IntoElement, ParentElement, Styled, TestAppContext, div, px};
use odk_a11y::{Describe, Role};
use odk_theme::JND;

use super::{FIXTURES, open, publish_fixture, redraw};
use crate::text::drawn;
use crate::{Heading, Stack, Text};

/// Items 14, 15: text reports itself as text, named by its content.
#[test]
fn text_describes_itself() {
    let a = Text::new("Monitors and scaling.").muted().accessible();
    assert_eq!(
        (a.role, a.name.as_str()),
        (Role::Text, "Monitors and scaling.")
    );
}

/// Items 14, 15: a heading reports its level, as given, between 1 and 6.
#[test]
fn a_heading_reports_its_level() {
    let a = Heading::new("Display").level(2).accessible();
    assert_eq!(
        (a.role, a.name.as_str(), a.level),
        (Role::Heading, "Display", Some(2))
    );
    assert_eq!(Heading::new("Top").accessible().level, Some(1));
    assert_eq!(Heading::new("Deep").level(4).accessible().level, Some(4));
    assert_eq!(Heading::new("Zero").level(0).accessible().level, Some(1));
    assert_eq!(Heading::new("Far").level(9).accessible().level, Some(6));
}

/// Items 1, 3, 34: text and headings draw their component tokens in every fixture theme,
/// recoloured on the next frame after a theme change, and grey in a monochrome one.
#[gpui::test]
fn they_draw_their_component_tokens_in_every_fixture(cx: &mut TestAppContext) {
    let cx = open(cx, |_, _| {
        Stack::new()
            .child(Text::new("Body"))
            .child(Text::new("Aside").muted())
            .child(Heading::new("Zero").level(0))
            .child(Heading::new("Title"))
            .child(Heading::new("Section").level(2))
            .child(Heading::new("Group").level(3))
            .child(Heading::new("Deep").level(9))
    });
    let mut primaries = Vec::new();
    for (theme, colors) in FIXTURES {
        publish_fixture(colors);
        redraw(cx);
        let c = crate::components();
        let wanted = [
            ("Body", c.text(false)),
            ("Aside", c.text(true)),
            ("Zero", c.heading(0)),
            ("Title", c.heading(1)),
            ("Section", c.heading(2)),
            ("Group", c.heading(3)),
            ("Deep", c.heading(9)),
        ];
        for (content, want) in wanted {
            assert_eq!(drawn::last(content), want, "{theme} {content}");
        }
        primaries.push(drawn::last("Body").colour);
        if theme == "vantablack" {
            for (content, _) in wanted {
                let colour = drawn::last(content).colour;
                assert!(colour.to_oklch().c <= JND, "{theme}: {content} has a hue");
            }
        }
    }
    let [dark, .., light] = primaries[..] else {
        unreachable!()
    };
    assert_ne!(dark, light, "a theme change recolours the text");
}

/// Items 2, 34: monospace text is the same text in the `mono` role, in the colour of its kind.
#[gpui::test]
fn mono_text_draws_the_mono_role_in_the_colour_of_its_kind(cx: &mut TestAppContext) {
    use odk_theme::foundation::TypeRole;
    let cx = open(cx, |_, _| {
        Stack::new()
            .child(Text::new("plain"))
            .child(Text::new("code").mono())
            .child(Text::new("quiet").muted())
            .child(Text::new("quiet code").muted().mono())
    });
    redraw(cx);
    let c = crate::components();
    assert_eq!(drawn::last("code"), c.text_mono(false));
    assert_eq!(drawn::last("quiet code"), c.text_mono(true));
    assert_eq!(drawn::last("code").role, TypeRole::Mono);
    assert_eq!(drawn::last("plain").role, TypeRole::Body);
    assert_eq!(drawn::last("code").colour, drawn::last("plain").colour);
    assert_eq!(
        drawn::last("quiet code").colour,
        drawn::last("quiet").colour
    );
}

/// Items 2, 34: a text that stays on one line is as tall as one line of its role however long it
/// is and however narrow its parent, where the same text wraps; and it does not push a sibling
/// out of a row.
#[gpui::test]
fn single_line_text_keeps_one_line_where_plain_text_wraps(cx: &mut TestAppContext) {
    fn line(name: &'static str, child: impl IntoElement) -> gpui::Div {
        div().debug_selector(move || name.into()).child(child)
    }
    let long = "/usr/lib/some/very/long/path/to/a/program --with --many --arguments --and more";
    let cx = open(cx, move |_, _| {
        Stack::new()
            .w(px(160.0))
            .child(line("wraps", Text::new(long)))
            .child(line("one", Text::new(long).single_line()))
            .child(line("one-mono", Text::new(long).mono().single_line()))
            .child(line("wraps-mono", Text::new(long).mono()))
    });
    let mut height = |part: &str| {
        let part: &'static str = Box::leak(part.to_owned().into_boxed_str());
        f32::from(cx.debug_bounds(part).unwrap().size.height)
    };
    assert_eq!(height("one"), 20.0, "one body line");
    assert_eq!(height("one-mono"), 18.0, "one mono line");
    assert!(
        height("wraps") > 20.0,
        "the same text wraps: {}",
        height("wraps")
    );
    assert!(height("wraps-mono") > 18.0);
}

/// Items 2, 34: each draws its type role, seen in its line's height: body text 20, a level-1
/// heading 30, level 2 26, and level 3 and below 22, the subtitle's.
#[gpui::test]
fn headings_draw_their_level_s_type_role(cx: &mut TestAppContext) {
    fn line(name: &'static str, child: impl IntoElement) -> gpui::Div {
        div().debug_selector(move || name.into()).child(child)
    }
    let cx = open(cx, |_, _| {
        Stack::new()
            .w(px(600.0))
            .child(line("body", Text::new("Body")))
            .child(line("aside", Text::new("Aside").muted()))
            .child(line("h1", Heading::new("Title")))
            .child(line("h2", Heading::new("Section").level(2)))
            .child(line("h3", Heading::new("Group").level(3)))
            .child(line("h6", Heading::new("Deep").level(6)))
    });
    for (part, height) in [
        ("body", 20.0),
        ("aside", 20.0),
        ("h1", 30.0),
        ("h2", 26.0),
        ("h3", 22.0),
        ("h6", 22.0),
    ] {
        let bounds = cx
            .debug_bounds(part)
            .unwrap_or_else(|| panic!("no part named {part}"));
        assert_eq!(bounds.size.height, px(height), "{part}");
    }
}
