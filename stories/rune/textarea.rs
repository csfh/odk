//! The Textarea story on Rune: a box that starts a few rows high, grows with its text to a limit,
//! wraps, and ends a line on Enter. Run it with `tools/story rune-textarea`.

use odk_view::{Hosted, Layout, Space, Textarea};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    length: usize,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let on_change = |s: &mut Story, text: String| s.length = text.chars().count();
    let said = format!(
        "{} characters. Enter ends a line; Tab leaves.",
        hosted.app.length
    );
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Notes",
                "Two rows to start, five at most; past that the text scrolls.",
                layout
                    .stack(Space::Sm)
                    .child(
                        Textarea::new("notes", "Notes", on_change)
                            .rows(2)
                            .max_rows(5)
                            .placeholder("Write something")
                            .width(360.0)
                            .show(hosted),
                    )
                    .child(caption(hosted, &said)),
            ),
            section(
                hosted,
                "States",
                "A starting value of several lines, invalid, and disabled.",
                layout
                    .stack(Space::Sm)
                    .child(
                        Textarea::new("seed", "Starting value", on_change)
                            .default_value("First line\nSecond line")
                            .width(360.0)
                            .show(hosted),
                    )
                    .child(
                        Textarea::new("bad", "Invalid", on_change)
                            .invalid(true)
                            .width(360.0)
                            .show(hosted),
                    )
                    .child(
                        Textarea::new("off", "Disabled", on_change)
                            .disabled(true)
                            .width(360.0)
                            .show(hosted),
                    ),
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
