//! The Separator story on Rune: a line across a column, an inset one and a vertical one. Run it
//! with `tools/story rune-separator`.

use odk_view::{Hosted, Layout, Separator, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// What the vertical line divides: a row this many points high.
const ROW_HEIGHT: f32 = 32.0;

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let across = layout
        .stack(Space::Sm)
        .child(line(hosted, "Above"))
        .child(Separator::horizontal().show(hosted))
        .child(line(hosted, "Below"));
    let inset = col()
        .gap(layout.space(Space::Sm))
        .child(line(hosted, "Above"))
        .child(Separator::horizontal().inset().show(hosted))
        .child(line(hosted, "Below"));
    let down = layout
        .row(Space::Md)
        .h(hosted.theme().text.px(ROW_HEIGHT))
        .child(line(hosted, "Left"))
        .child(Separator::vertical().show(hosted))
        .child(line(hosted, "Right"));

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Across",
                "One border width, in the subtle border colour.",
                across,
            ),
            section(
                hosted,
                "Inset",
                "Stops short of each end by a control's padding.",
                inset,
            ),
            section(hosted, "Down", "A vertical line spans its row.", down),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
