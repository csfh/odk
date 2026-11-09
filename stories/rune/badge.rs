//! The Badge story on Rune: every role at every emphasis, with an edge, with marks and icons, and
//! in a column too narrow for its label. Run it with `tools/story rune-badge`.

use odk_theme::semantic::{Emphasis, Role};
use odk_view::{Badge, Flow, Glyph, Hosted, Icon, Layout, Space, Tone};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for a label longer than its room.
const NARROW: f32 = 120.0;

const ROLES: [(Role, &str); 6] = [
    (Role::Accent, "Accent"),
    (Role::Neutral, "Neutral"),
    (Role::Info, "Info"),
    (Role::Success, "Success"),
    (Role::Warning, "Warning"),
    (Role::Danger, "Danger"),
];
const EMPHASES: [(Emphasis, &str); 3] = [
    (Emphasis::Solid, "Solid"),
    (Emphasis::Muted, "Muted"),
    (Emphasis::Subtle, "Subtle"),
];

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let grid = EMPHASES
        .iter()
        .fold(layout.stack(Space::Sm), |stack, (emphasis, name)| {
            stack.child(
                ROLES
                    .iter()
                    .fold(layout.row(Space::Sm), |row, (role, label)| {
                        row.child(
                            Badge::new(format!("{label} {name}"))
                                .role(*role)
                                .emphasis(*emphasis)
                                .show(hosted),
                        )
                    }),
            )
        });
    let outlined = ROLES
        .iter()
        .fold(layout.row(Space::Sm), |row, (role, label)| {
            row.child(Badge::new(*label).role(*role).outlined().show(hosted))
        });
    let marked = [Tone::Info, Tone::Success, Tone::Warning, Tone::Danger]
        .iter()
        .fold(layout.row(Space::Sm), |row, tone| {
            row.child(
                Badge::new(format!("{tone:?}"))
                    .tone(*tone)
                    .marked()
                    .show(hosted),
            )
        })
        .child(
            Badge::new("Starred")
                .icon(Glyph::Icon(Icon::StarFilled))
                .show(hosted),
        )
        .child(Badge::new("Character").icon("★").show(hosted));
    let narrow = |flow: Flow| {
        col().w(hosted.theme().text.px(NARROW)).child(
            Badge::new("A badge with a label far too long for its column")
                .flow(flow)
                .show(hosted),
        )
    };
    let worst = layout
        .row(Space::Md)
        .child(narrow(Flow::End))
        .child(narrow(Flow::Wrap));

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Roles and emphasis",
                "Any of the six roles at any emphasis; the size does not change.",
                grid,
            ),
            section(
                hosted,
                "Outlined",
                "An edge in the role's colour.",
                outlined,
            ),
            section(
                hosted,
                "Marks",
                "A status mark so the status reads without its colour; an icon or a character.",
                marked,
            ),
            section(
                hosted,
                "Worst case",
                "One line ending in an ellipsis, or wrapped (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
