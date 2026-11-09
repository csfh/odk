//! The Kbd story on Rune: a key, a chord and a chord that ends in Plus, in a sentence. Run it
//! with `tools/story rune-kbd`.

use odk_view::{Hosted, Kbd, Layout, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let chords = ["K", "Ctrl+K", "Ctrl+Shift+P", "Ctrl++", "Alt+Left"]
        .iter()
        .fold(layout.row(Space::Md), |row, chord| {
            row.child(Kbd::new(chord).show(hosted))
        });
    let inline = layout
        .row(Space::Sm)
        .child(line(hosted, "Press"))
        .child(Kbd::new("Ctrl+K").show(hosted))
        .child(line(hosted, "to open the command palette."));

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Chords",
                "One keycap per key, in the monospace face.",
                chords,
            ),
            section(
                hosted,
                "Inline",
                "Small enough to sit in a sentence.",
                inline,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
