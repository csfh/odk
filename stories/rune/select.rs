//! The Select story on Rune: a closed list, a chosen option and worst-case labels. Run it with
//! `tools/story rune-select`.

use odk_view::{Hosted, Layout, Select, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

#[derive(Default)]
struct Story {
    fruit: Option<usize>,
    worst: Option<usize>,
}

fn fruits() -> Vec<String> {
    ["Apple", "Banana", "Cherry", "Damson"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn worst() -> Vec<String> {
    [
        "A label far too long for the room the list gives it, however wide that is",
        "Ünïcödé and accents",
        "日本語の文字",
        "Unbreakable_word_with_no_place_to_wrap_at_all_in_the_list",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let chosen = hosted
        .app
        .fruit
        .map_or("Nothing chosen.".to_string(), |index| {
            format!("Chose {}.", fruits()[index])
        });
    let fruit = layout
        .stack(Space::Sm)
        .child(
            Select::new(
                "fruit",
                "Fruit",
                fruits(),
                hosted.app.fruit,
                |story: &mut Story, index| story.fruit = Some(index),
            )
            .show(hosted, &mut layers),
        )
        .child(line(hosted, chosen));
    let long = Select::new(
        "worst",
        "Worst case",
        worst(),
        hosted.app.worst,
        |story: &mut Story, index| story.worst = Some(index),
    )
    .show(hosted, &mut layers);

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Fruit",
                "Space, Enter or Down opens it; arrows move; Escape closes it.",
                fruit,
            ),
            section(
                hosted,
                "Worst case",
                "Long labels, accents, Japanese and an unbreakable word (item 23).",
                long,
            ),
        ],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
