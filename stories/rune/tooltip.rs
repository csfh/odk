//! The Tooltip story on Rune: a tooltip on each side of its trigger, one aligned to the end, one
//! with a long text. Rest the pointer on a button. Run it with `tools/story rune-tooltip`.

use odk_view::{Align, Button, Hosted, Icon, Layout, Placement, Space, Tooltip};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{page, section};

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let nothing = |_: &mut ()| {};
    let mut side = |id: &str, label: &str, placement: Placement, align: Align| {
        Tooltip::new(id, format!("On the {label} side"))
            .placement(placement)
            .align(align)
            .wrap(
                hosted,
                Button::new(format!("{id}-b"), label.to_string(), nothing).show(hosted),
                &mut layers,
            )
    };
    let sides = layout
        .row(Space::Lg)
        .child(side("top", "Top", Placement::Top, Align::Start))
        .child(side("bottom", "Bottom", Placement::Bottom, Align::Start))
        .child(side("left", "Left", Placement::Left, Align::Start))
        .child(side("right", "Right", Placement::Right, Align::Start))
        .child(side(
            "end",
            "Aligned to the end",
            Placement::Top,
            Align::End,
        ));
    let icon = Tooltip::new("icon", "Settings").wrap(
        hosted,
        Button::icon("icon-b", Icon::Settings, "Settings", nothing).show(hosted),
        &mut layers,
    );
    let long = Tooltip::new(
        "long",
        "A tooltip whose text is a good deal longer than one line, with 日本語の文字 and Unbreakable_word_with_no_place_to_wrap_at_all_in_the_tooltip",
    )
    .wrap(hosted, Button::new("long-b", "Long", nothing).show(hosted), &mut layers);

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Sides",
                "After the pointer rests; it stays while on the tooltip; Escape closes it.",
                sides,
            ),
            section(
                hosted,
                "An icon button",
                "A tooltip names a control that has no text.",
                icon,
            ),
            section(
                hosted,
                "Worst case",
                "The text wraps inside a measure (item 23).",
                long,
            ),
        ],
        layers,
    )
}

fn main() {
    support::run((), view);
}
