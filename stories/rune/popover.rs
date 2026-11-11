//! The Popover story on Rune: a panel opened from a trigger, with a title and controls, an
//! uncontrolled one and a controlled one. Run it with `tools/story rune-popover`.

use odk_view::{Button, Checkbox, Hosted, Icon, Layout, Popover, Space, Text};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

#[derive(Default)]
struct Story {
    open: bool,
    applied: u32,
}

fn trigger(hosted: &Hosted<Story>, label: &str) -> El {
    Layout::of(hosted)
        .row(Space::Xs)
        .child(Text::new(label).show(hosted))
        .child(Icon::ChevronDown.mark(14.0, odk_view::ink(hosted.theme().semantic.text.primary)))
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let filters = Popover::new("filters", "Filters")
        .title("Filter the list")
        .content(|hosted: &Hosted<Story>| {
            Layout::of(hosted)
                .stack(Space::Sm)
                .child(Checkbox::new("unread", "Unread only").show(hosted))
                .child(Checkbox::new("starred", "Starred").show(hosted))
                .child(
                    Button::new("apply", "Apply", |story: &mut Story| story.applied += 1)
                        .show(hosted),
                )
        })
        .wrap(hosted, trigger(hosted, "Filters"), &mut layers);
    let live = layout.stack(Space::Sm).child(filters).child(line(
        hosted,
        format!("Applied {} times.", hosted.app.applied),
    ));
    let controlled = Popover::new("help", "Help")
        .title("Help")
        .open(hosted.app.open)
        .on_open_change(|story: &mut Story, open| story.open = open)
        .content(|hosted: &Hosted<Story>| {
            Text::new("The application decides whether this is open.")
                .muted()
                .show(hosted)
        })
        .wrap(hosted, trigger(hosted, "Help"), &mut layers);

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Uncontrolled",
                "A press, Enter or Space opens it; Escape or a press outside closes it. In Rune it is modal: Tab stays inside.",
                live,
            ),
            section(
                hosted,
                "Controlled",
                "The application holds the state and follows each request.",
                controlled,
            ),
        ],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
