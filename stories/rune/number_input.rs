//! The NumberInput story on Rune: free typing, a commit on Enter that clamps and tidies, Up and Down
//! and the steppers, a bound, fine steps, read-only and disabled. Run it with
//! `tools/story rune-number_input`.

use odk_view::{Hosted, Layout, NumberInput, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    heard: String,
    committed: String,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let said = format!(
        "Heard {}. Committed {}.",
        if story.heard.is_empty() {
            "nothing"
        } else {
            &story.heard
        },
        if story.committed.is_empty() {
            "nothing"
        } else {
            &story.committed
        },
    );
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Quantity, 0 to 10",
                "Type anything; Enter clamps and tidies it. Up and Down step; the buttons are not tab stops.",
                layout
                    .stack(Space::Sm)
                    .child(
                        NumberInput::<Story>::new("qty", "Quantity")
                            .min(0.0)
                            .max(10.0)
                            .default_value(3.0)
                            .on_change(|s: &mut Story, v| s.heard = format!("{v:?}"))
                            .on_commit(|s: &mut Story, v| s.committed = v.to_string())
                            .show(hosted),
                    )
                    .child(caption(hosted, &said)),
            ),
            section(
                hosted,
                "Other fields",
                "A step of 0.1, read-only, and disabled.",
                layout
                    .stack(Space::Sm)
                    .child(
                        NumberInput::<Story>::new("fine", "Opacity")
                            .step(0.1)
                            .min(0.0)
                            .max(1.0)
                            .default_value(0.5)
                            .show(hosted),
                    )
                    .child(
                        NumberInput::<Story>::new("ro", "Read only")
                            .default_value(42.0)
                            .read_only(true)
                            .show(hosted),
                    )
                    .child(
                        NumberInput::<Story>::new("off", "Disabled")
                            .disabled(true)
                            .show(hosted),
                    ),
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
