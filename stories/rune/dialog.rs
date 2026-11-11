//! The Dialog story on Rune: a confirmation, a destructive confirmation that turns the ways out
//! off, a form, and a body too tall for the window. Run it with `tools/story rune-dialog`.

use odk_theme::component::ButtonVariant;
use odk_view::{Button, Dialog, Hosted, Input, Layout, Space, Text};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Confirm,
    Destructive,
    Form,
    Long,
}

#[derive(Default)]
struct Story {
    open: Option<Kind>,
    name: String,
    last: String,
}

fn close(story: &mut Story, said: &str) {
    story.open = None;
    story.last = said.to_string();
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let mut layers = Vec::new();
    let open = |kind: Kind| move |s: &mut Story| s.open = Some(kind);

    let triggers = layout
        .row(Space::Sm)
        .child(Button::new("confirm", "Confirm", open(Kind::Confirm)).show(hosted))
        .child(
            Button::new("destructive", "Destructive", open(Kind::Destructive))
                .variant(ButtonVariant::Destructive)
                .show(hosted),
        )
        .child(Button::new("form", "Form", open(Kind::Form)).show(hosted))
        .child(Button::new("long", "Long body", open(Kind::Long)).show(hosted));

    Dialog::<Story>::new("d-confirm", "Save your changes?")
        .open(story.open == Some(Kind::Confirm))
        .description("You have changes that are not saved yet.")
        .on_close(|s| close(s, "Closed the confirmation."))
        .returns_focus_to("confirm")
        .action(Button::new("c-no", "Discard", |s: &mut Story| close(s, "Discarded.")).show(hosted))
        .action(
            Button::new("c-yes", "Save", |s: &mut Story| close(s, "Saved."))
                .variant(ButtonVariant::Primary)
                .show(hosted),
        )
        .show(hosted, &mut layers);

    Dialog::<Story>::new("d-destructive", "Delete the project?")
        .open(story.open == Some(Kind::Destructive))
        .description(
            "This cannot be undone. Escape, the scrim and the close button are off: choose.",
        )
        .close_on_escape(false)
        .close_on_backdrop(false)
        .close_button(false)
        .returns_focus_to("destructive")
        .action(Button::new("d-no", "Keep it", |s: &mut Story| close(s, "Kept.")).show(hosted))
        .action(
            Button::new("d-yes", "Delete", |s: &mut Story| close(s, "Deleted."))
                .variant(ButtonVariant::Destructive)
                .show(hosted),
        )
        .show(hosted, &mut layers);

    Dialog::<Story>::new("d-form", "Name the project")
        .open(story.open == Some(Kind::Form))
        .on_close(|s| close(s, "Closed the form."))
        .returns_focus_to("form")
        .content(
            Input::new(
                "p-name",
                "Project name",
                |s: &mut Story, text| s.name = text,
                |_: &mut Story| {},
            )
            .show(hosted),
        )
        .action(
            Button::new("f-cancel", "Cancel", |s: &mut Story| close(s, "Cancelled.")).show(hosted),
        )
        .action(
            Button::new("f-ok", "Create", |s: &mut Story| close(s, "Created."))
                .variant(ButtonVariant::Primary)
                .show(hosted),
        )
        .show(hosted, &mut layers);

    let mut long = Dialog::<Story>::new("d-long", "Terms of use")
        .open(story.open == Some(Kind::Long))
        .description("The body scrolls; the header and the actions stay.")
        .on_close(|s| close(s, "Closed the terms."))
        .returns_focus_to("long")
        .action(Button::new("l-ok", "Accept", |s: &mut Story| close(s, "Accepted.")).show(hosted));
    for i in 1..=60 {
        long = long
            .content(Text::new(format!("{i}. A line of the terms, which are long.")).show(hosted));
    }
    long.show(hosted, &mut layers);

    let said = if story.last.is_empty() {
        "Open a dialog; Escape, the close button and the scrim close the first, third and fourth."
            .to_string()
    } else {
        story.last.clone()
    };
    page(
        hosted,
        cx,
        [section(
            hosted,
            "Dialogs",
            "A modal panel over a scrim: Tab stays inside, and the page behind takes nothing.",
            layout
                .stack(Space::Sm)
                .child(triggers)
                .child(caption(hosted, &said)),
        )],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
