//! What the Rune stories share: the window, and a titled section. A story is
//! `stories/rune/<name>.rs`, a binary named `rune-<name>`, run with `tools/story rune-<name>`.
//! The GPUI stories beside it stay until GPUI leaves (ADR 0089).

#![allow(
    dead_code,
    reason = "each story binary includes this file and uses some of it"
)]

use odk_theme::ThemeManager;
use odk_theme::foundation::TypeRole;
use odk_view::{Face, Heading, Hosted, Layout, Space, Text, frame, start};
use rune::{Cx, El};

/// A story's window width and height in points.
const WINDOW: (f32, f32) = (900.0, 700.0);

/// Omarchy's font, which fontconfig names as the monospace one, as bytes; Rune's own Inter when it
/// cannot be read. Rune has no font fallback, so a glyph the font lacks (the spinner's braille) is
/// drawn as a missing-glyph box.
fn face() -> Face {
    let ask = |format: &str| {
        let output = std::process::Command::new("fc-match")
            .args(["-f", format, "monospace"])
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8(output.stdout).ok())
            .flatten()
    };
    let font = ask("%{file}").and_then(|path| std::fs::read(path.trim()).ok());
    let family = ask("%{family[0]}");
    match (font, family) {
        (Some(bytes), Some(family)) => Face::leak(bytes, family.trim().to_string()),
        _ => Face::bundled(),
    }
}

/// Open the story's window on the user's theme, and return when it closes.
pub fn run<T: 'static>(app: T, view: fn(&Hosted<T>, &mut Cx<Hosted<T>>) -> El) {
    let theme = ThemeManager::current().snapshot();
    let hosted = Hosted::new(app, (*theme).clone(), face());
    if let Err(error) = start(hosted, view, WINDOW) {
        eprintln!("odk-view: {error}");
        std::process::exit(1);
    }
}

/// A titled group of `body`, with a line saying what it shows.
pub fn section<T>(hosted: &Hosted<T>, title: &str, note: &str, body: El) -> El {
    Layout::of(hosted)
        .stack(Space::Sm)
        .child(Heading::new(title).level(2).show(hosted))
        .child(Text::new(note).muted().show(hosted))
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

/// A line of the story's own text.
pub fn line<T>(hosted: &Hosted<T>, words: impl Into<String>) -> El {
    Text::new(words).show(hosted)
}

/// A small label in the quieter text colour.
pub fn caption<T>(hosted: &Hosted<T>, words: &str) -> El {
    Text::new(words)
        .muted()
        .role(TypeRole::Caption)
        .show(hosted)
}
