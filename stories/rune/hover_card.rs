//! The HoverCard story on Rune: a card of content that opens when the pointer rests on its
//! trigger. Run it with `tools/story rune-hover_card`.

use odk_view::{Align, Badge, Heading, Hosted, HoverCard, Layout, Placement, Space, Text, Tone};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

fn profile(hosted: &Hosted<()>) -> El {
    Layout::of(hosted)
        .stack(Space::Sm)
        .child(Heading::new("Ada Lovelace").level(3).show(hosted))
        .child(
            Text::new("Mathematician. Wrote the first program.")
                .muted()
                .show(hosted),
        )
        .child(Badge::new("Member").tone(Tone::Success).show(hosted))
}

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let below = HoverCard::new("ada", "Profile of Ada")
        .content(profile(hosted))
        .wrap(hosted, Text::new("@ada").show(hosted), &mut layers);
    let beside = HoverCard::new("grace", "Profile of Grace")
        .content(profile(hosted))
        .placement(Placement::Right)
        .align(Align::Start)
        .wrap(hosted, Text::new("@grace").show(hosted), &mut layers);
    let text = layout
        .row(Space::Sm)
        .child(line(hosted, "Written by"))
        .child(below)
        .child(line(hosted, "and reviewed by"))
        .child(beside);

    page(
        hosted,
        cx,
        [section(
            hosted,
            "Triggers",
            "Rest the pointer on a handle. It opens below by default, here also beside; it stays while the pointer is on it.",
            text,
        )],
        layers,
    )
}

fn main() {
    support::run((), view);
}
