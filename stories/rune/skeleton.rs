//! The Skeleton story on Rune: a line, a paragraph, a box and a control, each the size of what
//! it stands in for. Run it with `tools/story rune-skeleton`.

use odk_view::{Hosted, Layout, Skeleton, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// An avatar's size, and the column the text skeletons sit in, in points.
const AVATAR: f32 = 48.0;
const COLUMN: f32 = 320.0;

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let text = col()
        .w(hosted.theme().text.px(COLUMN))
        .gap(layout.space(Space::Md))
        .child(Skeleton::line().show(hosted))
        .child(Skeleton::text(3).show(hosted));
    let boxes = layout
        .row(Space::Md)
        .child(
            Skeleton::block(AVATAR, AVATAR)
                .label("Loading the avatar")
                .show(hosted),
        )
        .child(Skeleton::control().show(hosted));
    let card = layout
        .row(Space::Md)
        .child(Skeleton::block(AVATAR, AVATAR).show(hosted))
        .child(
            col()
                .w(hosted.theme().text.px(COLUMN))
                .child(Skeleton::text(2).show(hosted)),
        );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Text",
                "Lines of body text; the last of a paragraph is shorter.",
                text,
            ),
            section(
                hosted,
                "Boxes",
                "A box of a size, and the place of a control.",
                boxes,
            ),
            section(
                hosted,
                "Card",
                "What an avatar and two lines will replace.",
                card,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
