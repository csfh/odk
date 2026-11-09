//! The Text story on Rune: body text, muted and mono, every type role, and what it does when
//! its line is longer than its room. Run it with `tools/story rune-text`.

use odk_theme::foundation::TypeRole;
use odk_view::{Flow, Hosted, Layout, Space, Text};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for what is longer than its room.
const NARROW: f32 = 200.0;

const LONG: &str = "A sentence a good deal longer than the room the column gives it";
const UNBROKEN: &str = "Unbreakable_word_with_no_place_to_wrap_at_all_in_the_column";
const JAPANESE: &str = "日本語の文字を含む、とても長いラベルで、列の幅に収まらないもの";

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);

    let kinds = layout
        .stack(Space::Xs)
        .child(Text::new("Body text, in the primary colour.").show(hosted))
        .child(
            Text::new("Muted text, for what says less.")
                .muted()
                .show(hosted),
        )
        .child(
            Text::new("cargo build --release --locked")
                .mono()
                .show(hosted),
        );
    let roles = TypeRole::ALL
        .iter()
        .fold(layout.stack(Space::Xs), |stack, role| {
            stack.child(Text::new(format!("{role:?}")).role(*role).show(hosted))
        });
    let flow = |flow: Flow, text: &str| {
        col()
            .w(hosted.theme().text.px(NARROW))
            .child(Text::new(text).flow(flow).show(hosted))
    };
    let flows = [
        ("Wrap", Flow::Wrap),
        ("End", Flow::End),
        ("Middle", Flow::Middle),
        ("Clamp(2)", Flow::Clamp(2)),
    ]
    .iter()
    .fold(layout.stack(Space::Md), |stack, (name, kind)| {
        stack
            .child(Text::new(*name).muted().show(hosted))
            .child(flow(*kind, LONG))
            .child(flow(*kind, UNBROKEN))
            .child(flow(*kind, JAPANESE))
    });

    page(
        hosted,
        cx,
        [
            section(hosted, "Kinds", "Body, muted and mono.", kinds),
            section(hosted, "Roles", "Every type role of the foundation.", roles),
            section(
                hosted,
                "Flow",
                "A long line, an unbreakable word and Japanese, in a narrow column (item 23).",
                flows,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
