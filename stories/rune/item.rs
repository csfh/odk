//! The Item story on Rune: a title, a description, lines, actions and a footer, at each density,
//! and worst-case words. Run it with `tools/story rune-item`.

use odk_theme::foundation::Density;
use odk_view::{Badge, Button, Hosted, Item, Layout, Space, Text, Tone};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for words longer than the room an action leaves.
const NARROW: f32 = 320.0;

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let nothing = |_: &mut ()| {};
    let full = Item::new("Notifications")
        .description("Choose when you are told about activity.")
        .child(Text::new("Last changed yesterday").muted().show(hosted))
        .action(Badge::new("On").tone(Tone::Success).show(hosted))
        .action(Button::new("edit", "Edit", nothing).show(hosted))
        .footer(
            Text::new("A footer spans the whole item.")
                .muted()
                .show(hosted),
        )
        .show(hosted);
    let densities = Density::ALL
        .iter()
        .fold(layout.stack(Space::Sm), |stack, density| {
            stack.child(
                Item::new(format!("{density:?}"))
                    .description("An item is padded above and below by its density.")
                    .density(*density)
                    .show(hosted),
            )
        });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        Item::new("A setting whose title is a good deal longer than the room its control leaves")
            .description(
                "Unbreakable_word_with_no_place_to_wrap_at_all_beside_the_control, 日本語の文字",
            )
            .action(Button::new("worst", "Change", nothing).show(hosted))
            .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Everything",
                "Title, description, a line, two actions and a footer.",
                full,
            ),
            section(
                hosted,
                "Densities",
                "Padding above and below follows the density.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "The words wrap in what the action leaves (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
