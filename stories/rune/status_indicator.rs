//! The StatusIndicator story on Rune: every status, and a label too long for its column. Run it
//! with `tools/story rune-status_indicator`.

use odk_view::{Hosted, Layout, Space, Status, StatusIndicator};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for a label longer than its room.
const NARROW: f32 = 140.0;

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let all = [
        (Status::Active, "Online"),
        (Status::Inactive, "Offline"),
        (Status::Success, "Build passed"),
        (Status::Warning, "Syncing slowly"),
        (Status::Error, "Build failed"),
        (Status::Info, "Update available"),
    ]
    .iter()
    .fold(layout.stack(Space::Sm), |stack, (status, label)| {
        stack.child(StatusIndicator::new(*status, *label).show(hosted))
    });
    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        StatusIndicator::new(
            Status::Error,
            "A build that failed for a reason with a long name",
        )
        .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Statuses",
                "A dot and a label; an inactive status is a hollow ring.",
                all,
            ),
            section(
                hosted,
                "Worst case",
                "The label ends in an ellipsis (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run((), view);
}
