//! The Sheet story on Rune: a modal sheet from each edge, and a side panel that leaves the page
//! live. Run it with `tools/story rune-sheet`.

use odk_theme::component::ButtonVariant;
use odk_view::{Button, Hosted, Layout, Sheet, Side, Space, Text};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Right,
    Left,
    Bottom,
    Panel,
}

#[derive(Default)]
struct Story {
    open: Option<Kind>,
    presses: u32,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let mut layers = Vec::new();
    let open = |kind: Kind| move |s: &mut Story| s.open = Some(kind);
    let triggers = layout
        .row(Space::Sm)
        .child(Button::new("right", "From the right", open(Kind::Right)).show(hosted))
        .child(Button::new("left", "From the left", open(Kind::Left)).show(hosted))
        .child(Button::new("bottom", "From the bottom", open(Kind::Bottom)).show(hosted))
        .child(
            Button::new("panel", "Side panel", open(Kind::Panel))
                .variant(ButtonVariant::Outline)
                .show(hosted),
        );
    let sheet = |id: &str, title: &str, kind: Kind, side: Side, trigger: &str| {
        Sheet::<Story>::new(id.to_string(), title.to_string())
            .open(story.open == Some(kind))
            .side(side)
            .on_close(|s: &mut Story| s.open = None)
            .returns_focus_to(trigger.to_string())
            .content(
                Text::new("A modal sheet dims the page, keeps Tab inside and blocks it.")
                    .show(hosted),
            )
            .content(
                Button::new(format!("{id}-ok"), "Done", |s: &mut Story| s.open = None).show(hosted),
            )
    };
    sheet("s-right", "Details", Kind::Right, Side::Right, "right").show(hosted, &mut layers);
    sheet("s-left", "Navigation", Kind::Left, Side::Left, "left").show(hosted, &mut layers);
    sheet("s-bottom", "Actions", Kind::Bottom, Side::Bottom, "bottom")
        .extent(200.0)
        .show(hosted, &mut layers);
    Sheet::<Story>::new("s-panel", "Inspector")
        .open(story.open == Some(Kind::Panel))
        .modal(false)
        .on_close(|s: &mut Story| s.open = None)
        .content(
            Text::new("A side panel leaves the page live; Escape closes it while focus is in it.")
                .show(hosted),
        )
        .content(Button::new("s-panel-ok", "Done", |s: &mut Story| s.open = None).show(hosted))
        .show(hosted, &mut layers);

    let counter = Button::new("page", "A button on the page", |s: &mut Story| {
        s.presses += 1
    })
    .show(hosted);
    page(
        hosted,
        cx,
        [section(
            hosted,
            "Sheets",
            "Attached to an edge, no corner of their own, never larger than the window.",
            layout
                .stack(Space::Sm)
                .child(triggers)
                .child(counter)
                .child(caption(
                    hosted,
                    &format!("The page's button was pressed {} times.", story.presses),
                )),
        )],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
