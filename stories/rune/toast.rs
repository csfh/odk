//! The Toast story on Rune: a toast of each tone, one with an action, a persistent one, one that
//! updates in place, and a burst that shows the limit and the queue. Run it with
//! `tools/story rune-toast`.

use odk_view::{Button, Hosted, Layout, Space, Toast, ToastId, Toaster, Tone};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    toaster: Toaster<Story>,
    undone: u32,
    uploads: u32,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let mut layers = Vec::new();

    let tones = layout
        .row(Space::Sm)
        .child(
            Button::new("info", "Info", |s: &mut Story| {
                s.toaster
                    .raise(Toast::new("Heads up").description("Something you may want to know."));
            })
            .show(hosted),
        )
        .child(
            Button::new("success", "Success", |s: &mut Story| {
                s.toaster.raise(Toast::new("Saved").tone(Tone::Success));
            })
            .show(hosted),
        )
        .child(
            Button::new("warning", "Warning", |s: &mut Story| {
                s.toaster
                    .raise(Toast::new("Battery is low").tone(Tone::Warning));
            })
            .show(hosted),
        )
        .child(
            Button::new("danger", "Danger", |s: &mut Story| {
                s.toaster.raise(
                    Toast::new("Could not connect")
                        .description("Check the network and try again.")
                        .tone(Tone::Danger),
                );
            })
            .show(hosted),
        )
        .child(
            Button::new("neutral", "Neutral", |s: &mut Story| {
                s.toaster.raise(Toast::new("A plain message").neutral());
            })
            .show(hosted),
        );

    let behaviours = layout
        .row(Space::Sm)
        .child(
            Button::new("action", "With an action", |s: &mut Story| {
                s.toaster.raise(
                    Toast::new("Message deleted").action("Undo", |s: &mut Story| s.undone += 1),
                );
            })
            .show(hosted),
        )
        .child(
            Button::new("persistent", "Persistent", |s: &mut Story| {
                s.toaster.raise(
                    Toast::new("Update ready")
                        .description("It stays until you dismiss it.")
                        .persistent(),
                );
            })
            .show(hosted),
        )
        .child(
            Button::new("update", "Update in place", |s: &mut Story| {
                s.uploads += 1;
                s.toaster.raise(
                    Toast::new(format!("Upload {}", s.uploads))
                        .with_id(ToastId::named("upload"))
                        .description("The same toast, updated, with its clock started again."),
                );
            })
            .show(hosted),
        )
        .child(
            Button::new("burst", "Five at once", |s: &mut Story| {
                for i in 1..=5 {
                    s.toaster.raise(Toast::new(format!("Toast {i} of 5")));
                }
            })
            .show(hosted),
        );

    story.toaster.show(hosted, &mut layers);
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Tones",
                "They stack in the bottom-right corner. Hold the pointer over the stack to stop the clocks.",
                tones,
            ),
            section(
                hosted,
                "Behaviours",
                "At most three show; the rest wait. Tab to the stack and press Escape to dismiss the newest.",
                layout
                    .stack(Space::Sm)
                    .child(behaviours)
                    .child(caption(hosted, &format!("Undone {} times.", story.undone))),
            ),
        ],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
