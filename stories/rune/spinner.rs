//! The Spinner story on Rune: each density, beside text, many at once, and one you can take away
//! and bring back. Run it with `tools/story rune-spinner`.

use odk_theme::foundation::Density;
use odk_view::{Button, Hosted, Layout, Space, Spinner, Text};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// How many spinners the crowd has.
const CROWD: usize = 24;

struct Story {
    shown: bool,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let live = layout
        .row(Space::Md)
        .child(
            Button::new(
                "toggle",
                if hosted.app.shown { "Stop" } else { "Start" },
                |story: &mut Story| story.shown = !story.shown,
            )
            .show(hosted),
        )
        .child(if hosted.app.shown {
            Spinner::new().label("Syncing").show(hosted)
        } else {
            Text::new("Idle: nothing asks for a redraw.")
                .muted()
                .show(hosted)
        });
    let densities = Density::ALL
        .iter()
        .fold(layout.row(Space::Lg), |row, density| {
            row.child(
                Spinner::new()
                    .density(*density)
                    .label(format!("{density:?}"))
                    .show(hosted),
            )
        });
    let beside = layout
        .row(Space::Sm)
        .child(Spinner::new().show(hosted))
        .child(Text::new("Fetching your messages…").show(hosted));
    let crowd = (0..CROWD).fold(layout.row(Space::Md), |row, _| {
        row.child(Spinner::new().show(hosted))
    });

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Start and stop",
                "A window with no spinner is never woken; with one it wakes at each new glyph.",
                live,
            ),
            section(
                hosted,
                "Densities",
                "The icon size follows the density.",
                densities,
            ),
            section(
                hosted,
                "Beside text",
                "Its name says what is loading, so it never relies on its movement.",
                beside,
            ),
            section(
                hosted,
                "Many",
                "They all show the same glyph at the same time, on one wake.",
                crowd,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story { shown: true }, view);
}
