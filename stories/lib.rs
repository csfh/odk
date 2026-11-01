//! What the stories share: a titled section, a wrapping row and a narrow column. A story is
//! `stories/<name>.rs`, a binary of this crate, run in watch mode with `tools/story <name>`.
//! This file draws text through ODK components only, like the stories do.

use odk::gpui::{Div, px};
use odk::prelude::*;

/// A titled group of children, with a line saying what it shows.
pub fn section(title: &'static str, note: &'static str, body: impl IntoElement) -> Stack {
    Stack::new()
        .gap(Space::Sm)
        .child(Heading::new(title).level(2))
        .child(Text::new(note).muted())
        .child(body)
}

/// Children side by side, wrapping onto the next line when the window is narrow.
pub fn row() -> Row {
    Row::new().flex_wrap().gap(Space::Sm)
}

/// A column `width` pixels wide with a rule above it at that width, for the worst-case content
/// of checklist item 23: long labels, words that cannot break and text that is not Latin, in
/// less room than they want. Whatever is drawn past the rule has overflowed the column.
pub fn narrow(width: f32, body: impl IntoElement) -> Div {
    div().w(px(width)).child(
        Stack::new()
            .gap(Space::Sm)
            .child(Separator::horizontal())
            .child(body),
    )
}
