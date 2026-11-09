//! The Empty story on Rune: an icon, words and actions, words alone, and worst-case text. Run it
//! with `tools/story rune-empty`.

use odk_view::{Button, Empty, Hosted, Icon};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{page, section};

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let nothing = |_: &mut ()| {};
    let full = Empty::new("No matching processes")
        .description("Nothing matches that search.")
        .icon(Icon::Search)
        .action(Button::new("clear", "Clear search", nothing).show(hosted))
        .action(Button::new("reload", "Reload", nothing).show(hosted))
        .show(hosted);
    let words = Empty::new("Nothing here yet").show(hosted);
    let worst = Empty::new("An empty state whose title is a good deal longer than any thirty ems")
        .description(
            "A description that keeps going, with Ünïcödé, 日本語の文字 and \
             Unbreakable_word_with_no_place_to_wrap_at_all_in_the_measure, to see it stay a short \
             paragraph on a wide page.",
        )
        .icon(Icon::Info)
        .show(hosted);

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "With an icon and actions",
                "Centred in the room it is given.",
                full,
            ),
            section(hosted, "Words only", "A title alone.", words),
            section(
                hosted,
                "Worst case",
                "Words wrap at about thirty ems (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
