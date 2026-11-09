//! What the Rune stories share: the window, and a titled section. A story is
//! `stories/rune/<name>.rs`, a binary named `rune-<name>`, run with `tools/story rune-<name>`.
//! The GPUI stories beside it stay until GPUI leaves (ADR 0089).
//!
//! This is the one place the Rune stories set a font: `odk-view` has no `Text` or `Heading` yet,
//! so the text a story needs comes from here, and `tests/no_direct_text.rs` lets this file be.

#![allow(
    dead_code,
    reason = "each story binary includes this file and uses some of it"
)]

use odk_theme::ThemeManager;
use odk_view::{Face, Hosted, Layout, Space, frame, ink, start};
use rune::{Cx, El, TextStyle, text};

/// A story's window width and height in points.
const WINDOW: (f32, f32) = (900.0, 700.0);

/// A heading's size and weight, in points at the base text size, and a note's.
const HEADING: (f32, f32) = (16.0, 600.0);
const NOTE: (f32, f32) = (12.0, 400.0);
/// A caption's size.
const CAPTION: f32 = 11.0;

/// Open the story's window on the user's theme, and return when it closes.
pub fn run<T: 'static>(app: T, view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El) {
    let theme = ThemeManager::current().snapshot();
    let hosted = Hosted::new(app, (*theme).clone(), Face::bundled());
    if let Err(error) = start(hosted, view, WINDOW) {
        eprintln!("odk-view: {error}");
        std::process::exit(1);
    }
}

/// A titled group of `body`, with a line saying what it shows.
pub fn section<T>(hosted: &Hosted<T>, title: &str, note: &str, body: El) -> El {
    let theme = hosted.theme();
    let (heading, quiet) = (
        TextStyle::new(theme.text.px(HEADING.0), HEADING.1),
        TextStyle::new(theme.text.px(NOTE.0), NOTE.1),
    );
    Layout::of(hosted)
        .stack(Space::Sm)
        .child(
            text(title)
                .font(heading)
                .color(ink(theme.semantic.text.primary)),
        )
        .child(
            text(note)
                .font(quiet)
                .color(ink(theme.semantic.text.secondary)),
        )
        .child(body)
}

/// The story's page: `sections` in a page column, then `layers`, such as an open list.
pub fn page<T>(
    hosted: &Hosted<T>,
    cx: &mut Cx<Hosted<T>>,
    sections: impl IntoIterator<Item = El>,
    layers: impl IntoIterator<Item = El>,
) -> El {
    frame(
        hosted,
        cx,
        Layout::of(hosted).page().children(sections),
        layers,
    )
}

/// [`page`] at the wide content width, for a story with a grid.
pub fn wide_page<T>(
    hosted: &Hosted<T>,
    cx: &mut Cx<Hosted<T>>,
    sections: impl IntoIterator<Item = El>,
    layers: impl IntoIterator<Item = El>,
) -> El {
    frame(
        hosted,
        cx,
        Layout::of(hosted).wide_page().children(sections),
        layers,
    )
}

/// A line of the story's own text in the theme's primary colour.
pub fn line<T>(hosted: &Hosted<T>, words: impl Into<String>) -> El {
    let theme = hosted.theme();
    text(words.into())
        .font(TextStyle::new(theme.text.px(NOTE.0), NOTE.1))
        .color(ink(theme.semantic.text.primary))
}

/// A small label in the theme's quieter text colour.
pub fn caption<T>(hosted: &Hosted<T>, words: &str) -> El {
    let theme = hosted.theme();
    text(words)
        .font(TextStyle::new(theme.text.px(CAPTION), NOTE.1))
        .color(ink(theme.semantic.text.secondary))
}
