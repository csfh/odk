//! The NavList story on Rune: a sidebar's navigation with group headings, icons, counters, a
//! disabled row, each density, and a label too long for its sidebar. Run it with
//! `tools/story rune-nav_list`.

use odk_theme::foundation::Density;
use odk_view::{Hosted, Icon, Layout, NavList, Space};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{line, page, section};

/// A sidebar this many points wide.
const SIDEBAR: f32 = 240.0;

#[derive(Default)]
struct Story {
    page: Option<String>,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let width = hosted.theme().text.px(SIDEBAR);
    let mut settings = NavList::new("settings", "Settings")
        .group("Workspace")
        .item("general", "General")
        .item("members", "Members")
        .item("billing", "Billing")
        .group("Account")
        .item("profile", "Profile")
        .item("security", "Security")
        .icon("general", Icon::Settings)
        .icon("members", Icon::Team)
        .icon("billing", Icon::BankCard)
        .icon("profile", Icon::User)
        .counter("members", "12")
        .disable("billing")
        .on_change(|story: &mut Story, page| story.page = Some(page.to_string()));
    settings = settings.value(hosted.app.page.clone().unwrap_or_else(|| "general".into()));
    let live = layout
        .stack(Space::Sm)
        .child(col().w(width).child(settings.show(hosted)))
        .child(line(
            hosted,
            format!("Page: {}.", hosted.app.page.as_deref().unwrap_or("general")),
        ));
    let densities =
        Density::ALL
            .iter()
            .enumerate()
            .fold(layout.stack(Space::Md), |stack, (n, density)| {
                stack.child(
                    col().w(width).child(
                        NavList::<Story>::new(format!("density-{n}"), format!("{density:?}"))
                            .item("a", format!("{density:?}, one"))
                            .item("b", format!("{density:?}, two"))
                            .default_value("a")
                            .density(*density)
                            .show(hosted),
                    ),
                )
            });
    let worst = col().w(width).child(
        NavList::<Story>::new("worst", "Worst case")
            .group("A group heading far longer than the sidebar it sits in")
            .item(
                "a",
                "A row whose label is far longer than the room it has, 日本語の文字",
            )
            .icon("a", Icon::Inbox)
            .counter("a", "1234")
            .item(
                "b",
                "Unbreakable_word_with_no_place_to_wrap_at_all_in_the_row",
            )
            .default_value("a")
            .show(hosted),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Interactive",
                "Tab reaches the current page; Up and Down move; Enter or Space opens a row.",
                live,
            ),
            section(
                hosted,
                "Densities",
                "Rows are 28, 34 or 40 points high.",
                densities,
            ),
            section(
                hosted,
                "Worst case",
                "Labels and headings end in an ellipsis; the counter keeps its place (item 23).",
                worst,
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
