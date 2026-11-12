//! The Alert story on Rune: every tone and emphasis, a description, an action, a dismiss button,
//! a compact alert and worst-case words. Run it with `tools/story rune-alert`.

use odk_theme::semantic::Emphasis;
use odk_view::{Alert, Button, Hosted, Layout, Space, Tone};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// A column this many points wide, for an alert longer than its room.
const NARROW: f32 = 360.0;

#[derive(Default)]
struct Story {
    dismissed: u32,
    retried: u32,
}

const TONES: [(Tone, &str); 4] = [
    (Tone::Info, "Info"),
    (Tone::Success, "Success"),
    (Tone::Warning, "Warning"),
    (Tone::Danger, "Danger"),
];

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let tones = TONES
        .iter()
        .fold(layout.stack(Space::Sm), |stack, (tone, name)| {
            stack.child(
                Alert::new(format!("tone-{name}"), format!("{name} notice"))
                    .tone(*tone)
                    .description("What happened, and what to do about it.")
                    .show(hosted),
            )
        });
    let emphases = [Emphasis::Muted, Emphasis::Subtle, Emphasis::Solid]
        .iter()
        .fold(layout.stack(Space::Sm), |stack, emphasis| {
            stack.child(
                Alert::new(format!("emphasis-{emphasis:?}"), format!("{emphasis:?}"))
                    .tone(Tone::Warning)
                    .emphasis(*emphasis)
                    .description("The same notice at each emphasis.")
                    .show(hosted),
            )
        });
    let actions = layout
        .stack(Space::Sm)
        .child(
            Alert::new("action", "Could not save")
                .tone(Tone::Danger)
                .description("The disk is full.")
                .action(
                    Button::new("retry", "Retry", |story: &mut Story| story.retried += 1)
                        .show(hosted),
                )
                .on_dismiss(|story: &mut Story| story.dismissed += 1)
                .show(hosted),
        )
        .child(line(
            hosted,
            format!(
                "Retried {} times, dismissed {} times.",
                hosted.app.retried, hosted.app.dismissed
            ),
        ));
    let compact = Alert::new("compact", "Compact")
        .compact(true)
        .description("Tighter, for a dense place such as under a field.")
        .show(hosted);
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        Alert::new(
            "worst",
            "A title far longer than the room the alert gives it, in 日本語",
        )
        .tone(Tone::Danger)
        .description("Unbreakable_word_with_no_place_to_wrap_at_all_inside_the_alert")
        .action(Button::new("worst-act", "Retry", |_: &mut Story| {}).show(hosted))
        .on_dismiss(|_: &mut Story| {})
        .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Tones",
                "The mark and the words carry the meaning, so it reads without colour.",
                tones,
            ),
            section(hosted, "Emphasis", "Muted, subtle and solid.", emphases),
            section(
                hosted,
                "Action and dismiss",
                "Both are ordinary buttons, centred on the title's line.",
                actions,
            ),
            section(hosted, "Compact", "Tighter padding and gap.", compact),
            section(
                hosted,
                "Worst case",
                "The words wrap and the buttons keep their place (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
