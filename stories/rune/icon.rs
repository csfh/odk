//! The Icon story on Rune: every icon ODK ships, in three sizes, and the four status marks. Run
//! it with `tools/story rune-icon`.

use odk_theme::semantic::{Emphasis, Role, State};
use odk_view::{Hosted, Icon, Layout, Space, ink};
use rune::{Cx, El, col, row};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

/// The sizes shown, in points at the base text size.
const SIZES: [f32; 3] = [12.0, 20.0, 32.0];
/// The room a cell gives an icon's name.
const CELL: f32 = 112.0;

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let theme = hosted.theme();
    let scale = |points: f32| theme.text.px(points);
    let primary = ink(theme.semantic.text.primary);

    let all = Icon::ALL.iter().fold(
        row().flex_wrap().gap(layout.space(Space::Md)),
        |wrap, icon| {
            wrap.child(
                col()
                    .w(scale(CELL))
                    .gap(layout.space(Space::Xs))
                    .items_center()
                    .child(icon.mark(scale(SIZES[1]), primary))
                    .child(caption(hosted, icon.file().trim_end_matches(".svg"))),
            )
        },
    );
    let sizes = SIZES.iter().fold(layout.row(Space::Md), |row, size| {
        row.child(Icon::Settings.mark(scale(*size), primary))
    });
    let status = [Role::Info, Role::Success, Role::Warning, Role::Danger]
        .iter()
        .fold(layout.row(Space::Md), |row, role| {
            match Icon::status(*role) {
                Some(icon) => row.child(
                    icon.mark(
                        scale(SIZES[1]),
                        ink(theme
                            .semantic
                            .treatment(*role, Emphasis::Subtle, State::Default)
                            .foreground),
                    ),
                ),
                None => row,
            }
        });

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Sizes",
                "One icon at three sizes, in points at the base text size.",
                sizes,
            ),
            section(
                hosted,
                "Status",
                "A status reads without its colour.",
                status,
            ),
            section(
                hosted,
                "All",
                "Every icon of ODK's subset of Remix Icon 4.8.0.",
                all,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
