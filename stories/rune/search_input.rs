//! The SearchInput story on Rune: the search icon, the clear button, a shortcut hint, a spinner while
//! loading, and Escape that clears. Run it with `tools/story rune-search_input`.

use odk_view::{Hosted, Layout, SearchInput, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    query: String,
    submitted: String,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let on_change = |s: &mut Story, text: String| s.query = text;
    let on_submit = |s: &mut Story| s.submitted = s.query.clone();
    let said = format!(
        "Typed: “{}”. Submitted: “{}”.",
        story.query, story.submitted
    );
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Search",
                "Escape clears a field that has text. The clear button is not a tab stop.",
                layout
                    .stack(Space::Sm)
                    .child(
                        SearchInput::new("search", "Search files", on_change, on_submit)
                            .shortcut("Ctrl+K")
                            .show(hosted),
                    )
                    .child(caption(hosted, &said)),
            ),
            section(
                hosted,
                "States",
                "Loading, a starting value, and disabled.",
                layout
                    .stack(Space::Sm)
                    .child(
                        SearchInput::new("loading", "Search while loading", on_change, on_submit)
                            .loading(true)
                            .show(hosted),
                    )
                    .child(
                        SearchInput::new("seed", "Search with a value", on_change, on_submit)
                            .default_value("odk-view")
                            .show(hosted),
                    )
                    .child(
                        SearchInput::new("off", "Search, disabled", on_change, on_submit)
                            .disabled(true)
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
