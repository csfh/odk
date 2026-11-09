//! The Input story on Rune: a field with its label, one that submits, one set to a width, and
//! worst-case text. Run it with `tools/story rune-input`.

use odk_view::{Hosted, Input, Layout, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

#[derive(Default)]
struct Story {
    name: String,
    submitted: Vec<String>,
    wide: String,
    worst: String,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let submitted = hosted.app.submitted.last().map_or_else(
        || "Nothing submitted yet.".to_string(),
        |name| format!("Submitted {name:?}, {} in all.", hosted.app.submitted.len()),
    );
    let name = layout
        .stack(Space::Sm)
        .child(
            Input::new(
                "name",
                "Name",
                |story: &mut Story, text| story.name = text,
                |story: &mut Story| {
                    let name = story.name.clone();
                    story.submitted.push(name);
                },
            )
            .show(hosted),
        )
        .child(line(hosted, submitted));
    let wide = Input::new(
        "wide",
        "Address",
        |story: &mut Story, text| story.wide = text,
        |_: &mut Story| {},
    )
    .width(480.0)
    .show(hosted);
    let worst = Input::new(
        "worst",
        "Message",
        |story: &mut Story, text| story.worst = text,
        |_: &mut Story| {},
    )
    .show(hosted);

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Name",
                "Enter submits. The label is the field's accessible name.",
                name,
            ),
            section(
                hosted,
                "Width",
                "A width is points at the base text size, and follows the user's.",
                wide,
            ),
            section(
                hosted,
                "Worst case",
                "Type a long line, accents or Japanese: it scrolls inside the field.",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
