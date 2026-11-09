//! The ToggleGroup story on Rune: single and multiple, icons alone and before labels, a disabled
//! option, each density, and labels too long for their column. Run it with
//! `tools/story rune-toggle_group`.

use odk_theme::foundation::Density;
use odk_view::{Hosted, Icon, Layout, Space, ToggleGroup};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// A column this many points wide, for labels longer than its room.
const NARROW: f32 = 200.0;

#[derive(Default)]
struct Story {
    view: Vec<String>,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let live = layout
        .stack(Space::Sm)
        .child(
            ToggleGroup::new("view", "View")
                .item("list", "List")
                .item("grid", "Grid")
                .item("columns", "Columns")
                .value(hosted.app.view.clone())
                .on_change(|story: &mut Story, values| story.view = values.to_vec())
                .show(hosted),
        )
        .child(line(
            hosted,
            format!(
                "On: {}.",
                if hosted.app.view.is_empty() {
                    "nothing".into()
                } else {
                    hosted.app.view.join(", ")
                }
            ),
        ));
    let multiple = ToggleGroup::<Story>::new("format", "Format")
        .multiple(true)
        .icon_item("bold", "B", "Bold")
        .icon_item("italic", "I", "Italic")
        .icon_item("underline", "U", "Underline")
        .default_value(["bold"])
        .show(hosted);
    let icons = layout
        .stack(Space::Sm)
        .child(
            ToggleGroup::<Story>::new("layout", "Layout")
                .icon_item("rows", Icon::List, "Rows")
                .icon_item("tiles", Icon::Grid, "Tiles")
                .icon_item("columns", Icon::Columns, "Columns")
                .default_value(["rows"])
                .show(hosted),
        )
        .child(
            ToggleGroup::<Story>::new("labelled", "Labelled icons")
                .item("list", "List")
                .item("grid", "Grid")
                .icon("list", Icon::List)
                .icon("grid", Icon::Grid)
                .default_value(["grid"])
                .show(hosted),
        );
    let states = layout
        .stack(Space::Sm)
        .child(
            ToggleGroup::<Story>::new("some", "A disabled option")
                .item("day", "Daily")
                .item("week", "Weekly")
                .item("month", "Monthly")
                .disable("week")
                .default_value(["day"])
                .show(hosted),
        )
        .child(
            ToggleGroup::<Story>::new("locked", "Locked")
                .item("day", "Daily")
                .item("week", "Weekly")
                .default_value(["day"])
                .disabled(true)
                .show(hosted),
        );
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.stack(Space::Sm), |stack, (n, density)| {
                stack.child(
                    ToggleGroup::<Story>::new(format!("density-{n}"), format!("{density:?}"))
                        .item("a", format!("{density:?}"))
                        .item("b", "Two")
                        .default_value(["a"])
                        .density(*density)
                        .show(hosted),
                )
            });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        ToggleGroup::<Story>::new("worst", "Worst case")
            .item("a", "A label far longer than the room")
            .item("b", "Another long label for the group, 日本語の文字")
            .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Interactive",
                "Tab reaches the group; the arrows move without choosing; Space or Enter toggles.",
                live,
            ),
            section(
                hosted,
                "Multiple",
                "Each option toggles on its own.",
                multiple,
            ),
            section(
                hosted,
                "Icons",
                "An icon alone is a square segment; an icon can lead a label.",
                icons,
            ),
            section(
                hosted,
                "States",
                "The arrows skip a disabled option; a disabled group takes no focus.",
                states,
            ),
            section(
                hosted,
                "Densities",
                "Height and padding follow the density.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "Long labels end in an ellipsis (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
