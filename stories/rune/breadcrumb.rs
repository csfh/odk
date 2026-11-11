//! The Breadcrumb story on Rune: a path of links, the current page, a custom separator and a long
//! path folded into a `…` menu. Run it with `tools/story rune-breadcrumb`.

use odk_view::{Breadcrumb, Hosted, Layout, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    last: String,
}

fn went(label: &'static str) -> impl Fn(&mut Story) + 'static {
    move |s: &mut Story| s.last = format!("Went to {label}.")
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let plain = Breadcrumb::<Story>::new("plain")
        .item("Home", went("Home"))
        .item("Projects", went("Projects"))
        .current("Odk")
        .show(hosted, &mut layers);
    let slash = Breadcrumb::<Story>::new("slash")
        .separator("/")
        .item("Home", went("Home"))
        .item("Projects", went("Projects"))
        .current("Odk")
        .show(hosted, &mut layers);
    let long = Breadcrumb::<Story>::new("long")
        .max_items(3)
        .item("Home", went("Home"))
        .item("Projects", went("Projects"))
        .item("Odk", went("Odk"))
        .item("Crates", went("Crates"))
        .item("odk-view", went("odk-view"))
        .current("breadcrumb.rs")
        .show(hosted, &mut layers);
    let said = if hosted.app.last.is_empty() {
        "Tab reaches each link and Enter follows it; the … opens the hidden ones.".to_string()
    } else {
        hosted.app.last.clone()
    };
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "A path",
                "Links, then the current page in primary text.",
                layout.stack(Space::Sm).child(plain).child(slash),
            ),
            section(
                hosted,
                "Folded",
                "At most three items; the middle ones are a menu.",
                layout
                    .stack(Space::Sm)
                    .child(long)
                    .child(caption(hosted, &said)),
            ),
        ],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
