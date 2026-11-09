//! The Switch story on Rune: on and off, disabled, each density, and a long label. Run it with
//! `tools/story rune-switch`.

use odk_theme::foundation::Density;
use odk_view::{Hosted, Layout, Space, Switch};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// A column this many points wide, for a label longer than its room.
const NARROW: f32 = 200.0;

#[derive(Default)]
struct Story {
    wifi: bool,
    bluetooth: bool,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let app = &hosted.app;
    let live = layout
        .stack(Space::Sm)
        .child(
            Switch::new("wifi", "Wi-Fi")
                .checked(app.wifi)
                .on_change(|s: &mut Story, on| s.wifi = on)
                .show(hosted),
        )
        .child(
            Switch::new("bluetooth", "Bluetooth")
                .checked(app.bluetooth)
                .on_change(|s: &mut Story, on| s.bluetooth = on)
                .show(hosted),
        )
        .child(line(
            hosted,
            format!("Wi-Fi {}, Bluetooth {}.", app.wifi, app.bluetooth),
        ))
        .child(Switch::new("own", "Keeps its own state").show(hosted));
    let states = layout
        .stack(Space::Sm)
        .child(Switch::new("off", "Off").show(hosted))
        .child(Switch::new("on", "On").default_checked(true).show(hosted))
        .child(
            Switch::new("dis-off", "Disabled")
                .disabled(true)
                .show(hosted),
        )
        .child(
            Switch::new("dis-on", "Disabled and on")
                .default_checked(true)
                .disabled(true)
                .show(hosted),
        );
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.stack(Space::Sm), |stack, (n, density)| {
                stack.child(
                    Switch::new(format!("density-{n}"), format!("{density:?}"))
                        .density(*density)
                        .show(hosted),
                )
            });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        Switch::new(
            "worst",
            "A label far longer than the room its row gives it, with 日本語の文字 and Unbreakable_word_with_no_place_to_wrap_at_all",
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
                "Tab, then Space; a click on the track or the label. The thumb slides.",
                live,
            ),
            section(
                hosted,
                "States",
                "The track's fill and the thumb's place both say which it is.",
                states,
            ),
            section(
                hosted,
                "Densities",
                "Track and thumb follow the density.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "The label wraps beside the track (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
