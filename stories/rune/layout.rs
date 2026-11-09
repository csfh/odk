//! The layout groups on Rune: `Stack`, `Row` and `Grid`, at each of the foundation's spacing
//! steps. Resize the window to see the grid change its columns. Run it with
//! `tools/story rune-layout`.

use odk_theme::semantic::{Emphasis, Role, State};
use odk_view::{Hosted, Layout, Space, ink};
use rune::{Cx, El, el};

#[path = "support.rs"]
mod support;
use support::{line, section, wide_page};

/// A box of the story's size, in points at the base text size.
const BOX: (f32, f32) = (64.0, 32.0);
const CELL_HEIGHT: f32 = 48.0;

fn block<T>(hosted: &Hosted<T>, key: &str) -> El {
    let theme = hosted.theme();
    el().key(key)
        .size(theme.text.px(BOX.0), theme.text.px(BOX.1))
        .fill(ink(theme
            .semantic
            .treatment(Role::Accent, Emphasis::Solid, State::Default)
            .background))
}

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let steps = [Space::Xs, Space::Sm, Space::Md, Space::Lg, Space::Xl];

    let rows = steps.iter().fold(layout.stack(Space::Sm), |stack, space| {
        stack.child((0..4).fold(layout.row(*space), |row, n| {
            row.child(block(hosted, &format!("row-{space:?}-{n}")))
        }))
    });
    let stack = (0..3).fold(layout.stack(Space::Md), |stack, n| {
        stack.child(block(hosted, &format!("stack-{n}")))
    });
    let theme = hosted.theme();
    let cell_height = theme.text.px(CELL_HEIGHT);
    let grid = layout.grid(
        cx,
        "grid",
        3,
        (0..6).map(|n| {
            el().key(format!("cell-{n}"))
                .h(cell_height)
                .fill(ink(theme.semantic.surface.canvas))
                .child(line(hosted, format!("Cell {n}")))
        }),
    );

    wide_page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Row",
                "Side by side and centred across, at each spacing step.",
                rows,
            ),
            section(hosted, "Stack", "One under the next, a Md apart.", stack),
            section(
                hosted,
                "Grid",
                "As many columns as fit, no more than three.",
                grid,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
