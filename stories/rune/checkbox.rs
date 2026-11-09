//! The Checkbox story on Rune: checked, unchecked and mixed, disabled and invalid, each density,
//! and a long label in a narrow column. Run it with `tools/story rune-checkbox`.

use odk_theme::foundation::Density;
use odk_view::{Checkbox, Hosted, Layout, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// A column this many points wide, for a label longer than its room.
const NARROW: f32 = 200.0;

#[derive(Default)]
struct Story {
    agree: bool,
    all: bool,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let app = &hosted.app;
    let live = layout
        .stack(Space::Sm)
        .child(
            Checkbox::new("agree", "I agree to the terms")
                .checked(app.agree)
                .on_change(|s: &mut Story, on| s.agree = on)
                .required(true)
                .show(hosted),
        )
        .child(
            Checkbox::new("all", "Select all")
                .checked(app.all)
                .indeterminate(app.agree && !app.all)
                .on_change(|s: &mut Story, on| s.all = on)
                .show(hosted),
        )
        .child(line(
            hosted,
            format!("Agree {}, all {}.", app.agree, app.all),
        ))
        .child(
            Checkbox::new("own", "Keeps its own state")
                .default_checked(true)
                .show(hosted),
        );
    let states = layout
        .stack(Space::Sm)
        .child(Checkbox::new("off", "Unchecked").show(hosted))
        .child(
            Checkbox::new("on", "Checked")
                .default_checked(true)
                .show(hosted),
        )
        .child(
            Checkbox::new("mixed", "Mixed")
                .indeterminate(true)
                .show(hosted),
        )
        .child(
            Checkbox::new("dis-off", "Disabled")
                .disabled(true)
                .show(hosted),
        )
        .child(
            Checkbox::new("dis-on", "Disabled and checked")
                .default_checked(true)
                .disabled(true)
                .show(hosted),
        )
        .child(Checkbox::new("bad", "Invalid").invalid(true).show(hosted));
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.stack(Space::Sm), |stack, (n, density)| {
                stack.child(
                    Checkbox::new(format!("density-{n}"), format!("{density:?}"))
                        .density(*density)
                        .show(hosted),
                )
            });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        Checkbox::new(
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
                "Tab, then Space; a click on the box or the label. A mixed box becomes checked.",
                live,
            ),
            section(
                hosted,
                "States",
                "A tick and a dash, never a fill alone; invalid adds a mark after the label.",
                states,
            ),
            section(
                hosted,
                "Densities",
                "The box is the density's icon size.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "The label wraps beside the box (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
