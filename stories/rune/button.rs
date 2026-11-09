//! The Button story on Rune: every variant, density and state, and a count of its presses. Run it
//! with `tools/story rune-button`.

use odk_theme::component::ButtonVariant;
use odk_theme::foundation::Density;
use odk_view::{Button, Hosted, Layout, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

#[derive(Default)]
struct Story {
    presses: u32,
    busy: bool,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let press = |story: &mut Story| story.presses += 1;

    let variants =
        ButtonVariant::ALL
            .iter()
            .enumerate()
            .fold(layout.row(Space::Sm), |row, (n, variant)| {
                row.child(
                    Button::new(format!("variant-{n}"), variant.name(), press)
                        .variant(*variant)
                        .show(hosted),
                )
            });
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.row(Space::Sm), |row, (n, density)| {
                row.child(
                    Button::new(format!("density-{n}"), format!("{density:?}"), press)
                        .density(*density)
                        .show(hosted),
                )
            });
    let states = layout
        .row(Space::Sm)
        .child(Button::new("default", "Default", press).show(hosted))
        .child(
            Button::new("disabled", "Disabled", press)
                .disabled(true)
                .show(hosted),
        )
        .child(
            Button::new("busy", "Busy", |story: &mut Story| story.busy = !story.busy)
                .busy(hosted.app.busy)
                .show(hosted),
        );
    let worst = layout.row(Space::Sm).child(
        Button::new(
            "long",
            "A label far too long for the room it is given, with Ünïcödé and 日本語の文字",
            press,
        )
        .show(hosted),
    );
    let count = line(hosted, format!("Pressed {} times", hosted.app.presses));

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Variants",
                "Each variant the component layer styles.",
                variants,
            ),
            section(
                hosted,
                "Densities",
                "The application's three control heights.",
                densities,
            ),
            section(
                hosted,
                "States",
                "A disabled button leaves the Tab order; busy keeps its place.",
                states,
            ),
            section(
                hosted,
                "Worst case",
                "A long label, accents and Japanese (item 23).",
                worst,
            ),
            section(
                hosted,
                "Presses",
                "Enter and Space press on release, once per hold.",
                layout.row(Space::Sm).child(count),
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
