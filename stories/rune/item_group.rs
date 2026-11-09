//! The ItemGroup story on Rune: a titled group of items with a line between them, plain and
//! outlined. Run it with `tools/story rune-item_group`.

use odk_theme::component::ItemGroupVariant;
use odk_view::{Badge, Button, Hosted, Item, ItemGroup, Layout, Space, Tone};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{page, section};

fn group(hosted: &Hosted<()>, variant: ItemGroupVariant) -> El {
    let nothing = |_: &mut ()| {};
    ItemGroup::new("Devices")
        .description("Where you are signed in.")
        .variant(variant)
        .child(
            Item::new("Laptop")
                .description("This device")
                .action(Badge::new("Active").tone(Tone::Success).show(hosted))
                .show(hosted),
        )
        .child(
            Item::new("Phone")
                .description("Last seen yesterday")
                .action(Button::new(format!("{variant:?}-phone"), "Sign out", nothing).show(hosted))
                .show(hosted),
        )
        .child(
            Item::new("Tablet")
                .description("Last seen last week")
                .show(hosted),
        )
        .show(hosted)
}

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Default",
                "No box and no padding: its text lines up with the page.",
                layout
                    .stack(Space::Sm)
                    .child(group(hosted, ItemGroupVariant::Default)),
            ),
            section(
                hosted,
                "Outline",
                "Inside a border, for a group that lists things.",
                layout
                    .stack(Space::Sm)
                    .child(group(hosted, ItemGroupVariant::Outline)),
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
