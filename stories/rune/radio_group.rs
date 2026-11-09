//! The RadioGroup story on Rune: a column and a row, a disabled option, a wrong choice, a
//! disabled group, each density, and a long label. Run it with `tools/story rune-radio_group`.

use odk_theme::foundation::Density;
use odk_view::{Hosted, Layout, RadioGroup, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// A column this many points wide, for a label longer than its room.
const NARROW: f32 = 220.0;

#[derive(Default)]
struct Story {
    theme: Option<String>,
}

fn theme_group(hosted: &Hosted<Story>) -> El {
    let mut group = RadioGroup::new("theme", "Theme")
        .item("light", "Light")
        .item("dark", "Dark")
        .item("system", "Follow the system")
        .on_change(|story: &mut Story, value| story.theme = Some(value.to_string()));
    if let Some(chosen) = &hosted.app.theme {
        group = group.value(chosen.clone());
    }
    group.show(hosted)
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let live = layout
        .stack(Space::Sm)
        .child(theme_group(hosted))
        .child(line(
            hosted,
            format!(
                "Chosen: {}.",
                hosted.app.theme.as_deref().unwrap_or("nothing yet")
            ),
        ));
    let rows = layout
        .stack(Space::Md)
        .child(
            RadioGroup::<Story>::new("align", "Alignment")
                .item("left", "Left")
                .item("centre", "Centre")
                .item("right", "Right")
                .horizontal(true)
                .default_value("left")
                .show(hosted),
        )
        .child(
            RadioGroup::<Story>::new("plan", "Plan")
                .item("free", "Free")
                .item("pro", "Pro")
                .item("team", "Team (unavailable)")
                .disable("team")
                .default_value("pro")
                .show(hosted),
        );
    let states = layout
        .stack(Space::Md)
        .child(
            RadioGroup::<Story>::new("wrong", "Wrong")
                .item("a", "Nothing chosen")
                .item("b", "Another option")
                .invalid(true)
                .show(hosted),
        )
        .child(
            RadioGroup::<Story>::new("locked", "Locked")
                .item("a", "Disabled and chosen")
                .item("b", "Disabled")
                .default_value("a")
                .disabled(true)
                .show(hosted),
        );
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.stack(Space::Md), |stack, (n, density)| {
                stack.child(
                    RadioGroup::<Story>::new(format!("density-{n}"), format!("{density:?}"))
                        .item("a", format!("{density:?}, one"))
                        .item("b", format!("{density:?}, two"))
                        .default_value("a")
                        .density(*density)
                        .show(hosted),
                )
            });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        RadioGroup::<Story>::new("worst", "Worst case")
            .item(
                "a",
                "A label far longer than the room its row gives it, with 日本語の文字",
            )
            .item(
                "b",
                "Unbreakable_word_with_no_place_to_wrap_at_all_in_the_column",
            )
            .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Interactive",
                "Tab reaches the chosen option; the arrows move and choose, wrapping.",
                live,
            ),
            section(
                hosted,
                "Layouts",
                "A row, and a disabled option that arrows skip.",
                rows,
            ),
            section(
                hosted,
                "States",
                "A wrong choice marks the first option; a disabled group takes no focus.",
                states,
            ),
            section(
                hosted,
                "Densities",
                "The circle is the density's icon size.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "Labels wrap beside the circle (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
