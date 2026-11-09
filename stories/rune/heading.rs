//! The Heading story on Rune: levels 1 to 6, which draw as a title, a heading and a subtitle,
//! and a long one. Run it with `tools/story rune-heading`.

use odk_view::{Flow, Heading, Hosted, Layout, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for a heading longer than its room.
const NARROW: f32 = 240.0;

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let levels = (1..=6).fold(layout.stack(Space::Sm), |stack, level| {
        stack.child(
            Heading::new(format!("Level {level}"))
                .level(level)
                .show(hosted),
        )
    });
    let narrow = |flow: Flow| {
        col().w(hosted.theme().text.px(NARROW)).child(
            Heading::new("A heading far longer than the column it is in")
                .level(2)
                .flow(flow)
                .show(hosted),
        )
    };
    let long = layout
        .stack(Space::Md)
        .child(narrow(Flow::Wrap))
        .child(narrow(Flow::Clamp(1)));

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Levels",
                "Level 1 is a title, 2 a heading, 3 and below a subtitle; each keeps its level for a screen reader.",
                levels,
            ),
            section(
                hosted,
                "Long",
                "A heading wraps, or is clamped to a line (item 23).",
                long,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
