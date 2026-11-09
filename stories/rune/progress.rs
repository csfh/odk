//! The Progress story on Rune: a bar you can advance, every tone, a paused and a disabled one,
//! a value in words, values that are out of range, and a name too long for its line. Run it with
//! `tools/story rune-progress`.

use odk_view::{Button, Hosted, Layout, Progress, Space, Tone};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for a name longer than its room.
const NARROW: f32 = 200.0;
/// What each press of Advance adds.
const STEP: f64 = 15.0;

#[derive(Default)]
struct Story {
    done: f64,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let done = hosted.app.done;
    let live = layout
        .stack(Space::Md)
        .child({
            let bar = Progress::new("upload", "Uploading photos")
                .value(done)
                .show_label(true)
                .show_value(true);
            // A finished upload is a success.
            if done >= 100.0 {
                bar.tone(Tone::Success)
            } else {
                bar
            }
            .show(hosted)
        })
        .child(
            layout
                .row(Space::Sm)
                .child(
                    Button::new("advance", "Advance", |story: &mut Story| {
                        story.done = (story.done + STEP).min(100.0)
                    })
                    .show(hosted),
                )
                .child(
                    Button::new("reset", "Reset", |story: &mut Story| story.done = 0.0)
                        .show(hosted),
                ),
        );
    let tones = [Tone::Info, Tone::Success, Tone::Warning, Tone::Danger]
        .iter()
        .fold(
            layout.stack(Space::Md).child(
                Progress::new("accent", "Accent")
                    .value(60.0)
                    .show_label(true)
                    .show_value(true)
                    .show(hosted),
            ),
            |stack, tone| {
                stack.child(
                    Progress::new(format!("{tone:?}"), format!("{tone:?}"))
                        .value(60.0)
                        .tone(*tone)
                        .show_label(true)
                        .show_value(true)
                        .show(hosted),
                )
            },
        );
    let states = layout
        .stack(Space::Md)
        .child(
            Progress::new("paused", "Paused")
                .value(40.0)
                .paused(true)
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(
            Progress::new("disabled", "Disabled")
                .value(40.0)
                .disabled(true)
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(
            Progress::new("files", "Copying")
                .value(3.0)
                .max(7.0)
                .value_text("3 of 7 files")
                .show_label(true)
                .show_value(true)
                .show(hosted),
        );
    let range = layout
        .stack(Space::Md)
        .child(
            Progress::new("below", "Below zero")
                .value(-20.0)
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(
            Progress::new("above", "Above the most")
                .value(250.0)
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(
            Progress::new("nan", "Not a number")
                .value(f64::NAN)
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(
            Progress::new("bare", "No label and no value")
                .value(70.0)
                .show(hosted),
        );
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        Progress::new(
            "long",
            "A name far longer than the room its line gives it, 日本語の文字",
        )
        .value(33.0)
        .show_label(true)
        .show_value(true)
        .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Interactive",
                "The bar moves on Rune's spring, and at once with reduced motion.",
                live,
            ),
            section(hosted, "Tones", "A status in place of the accent.", tones),
            section(
                hosted,
                "States",
                "Paused is grey, disabled greyer; the value can be in words.",
                states,
            ),
            section(hosted, "Out of range", "No value can break the bar.", range),
            section(
                hosted,
                "Worst case",
                "A long name is cut and the value keeps its place (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
