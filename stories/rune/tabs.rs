//! The Tabs story on Rune: a list of tabs over panels, with an icon, a counter and a disabled tab,
//! manual activation, each density, and tabs too many for their width. Run it with
//! `tools/story rune-tabs`.

use odk_theme::foundation::Density;
use odk_view::{Hosted, Icon, Layout, Space, Tabs, Text};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for tabs that do not fit.
const NARROW: f32 = 260.0;

fn panel(words: &'static str) -> impl FnOnce(&Hosted<()>) -> El {
    move |hosted: &Hosted<()>| Text::new(words).muted().show(hosted)
}

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let main = Tabs::<()>::new("sections", "Settings sections")
        .tab(
            "general",
            "General",
            panel("General settings: the name and the language."),
        )
        .tab(
            "advanced",
            "Advanced",
            panel("Advanced settings, for those who know what they do."),
        )
        .tab(
            "billing",
            "Billing",
            panel("Billing is not available to you."),
        )
        .tab(
            "audit",
            "Audit log",
            panel("Everything that happened, newest first."),
        )
        .icon("advanced", Icon::Settings)
        .counter("advanced", "3")
        .disable("billing")
        .show(hosted);
    let manual = Tabs::<()>::new("manual", "Manual activation")
        .tab(
            "one",
            "One",
            panel("The arrows only move focus; Enter or Space selects."),
        )
        .tab(
            "two",
            "Two",
            panel("A panel that is slow to build is built only when selected."),
        )
        .tab("three", "Three", panel("Another panel."))
        .manual_activation(true)
        .show(hosted);
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.stack(Space::Md), |stack, (n, density)| {
                stack.child(
                    Tabs::<()>::new(format!("density-{n}"), format!("{density:?}"))
                        .tab("a", format!("{density:?}"), panel("A panel."))
                        .tab("b", "Two", panel("Another panel."))
                        .density(*density)
                        .show(hosted),
                )
            });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        Tabs::<()>::new("worst", "Too many")
            .tab("a", "A tab with a long label", panel("The first."))
            .tab("b", "Another long tab", panel("The second."))
            .tab("c", "A third long tab", panel("The third."))
            .tab("d", "日本語のタブ", panel("The fourth."))
            .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Tabs",
                "Tab reaches the selected tab; Left and Right move and select, wrapping.",
                main,
            ),
            section(
                hosted,
                "Manual activation",
                "Arrows move; Enter or Space selects.",
                manual,
            ),
            section(
                hosted,
                "Densities",
                "Tabs are 28, 34 or 40 points high.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "Tabs that do not fit scroll sideways (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
