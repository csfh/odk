//! The ContextMenu story on Rune: a menu at the pointer on a right click, or below the area on the
//! Menu key or Shift+F10. Run it with `tools/story rune-context_menu`.

use odk_view::{ContextMenu, Hosted, Icon, Layout, Menu, MenuItem, Space, Text};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    last: String,
}

fn said(label: &'static str) -> impl Fn(&mut Story) + 'static {
    move |s: &mut Story| s.last = format!("Chose {label}.")
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let mut layers = Vec::new();
    let menu = Menu::<Story>::new()
        .item(
            MenuItem::new("Rename")
                .icon(Icon::Draft)
                .hint("F2")
                .on_select(said("Rename")),
        )
        .item(MenuItem::new("Copy").hint("Ctrl+C").on_select(said("Copy")))
        .item(MenuItem::new("Move (not available)").disabled(true))
        .separator()
        .item(MenuItem::new("Delete").danger().on_select(said("Delete")));
    let area = ContextMenu::<Story>::new(
        "row",
        "Row actions",
        col()
            .w(320.0)
            .h(120.0)
            .center()
            .child(Text::new("Right-click here").show(hosted)),
    )
    .menu(menu)
    .show(hosted, &mut layers);
    let said = if hosted.app.last.is_empty() {
        "Right-click, or Tab here and press the Menu key or Shift+F10.".to_string()
    } else {
        hosted.app.last.clone()
    };
    page(
        hosted,
        cx,
        [section(
            hosted,
            "Context menu",
            "Opens at the pointer; the same list as a dropdown's.",
            layout
                .stack(Space::Sm)
                .child(area)
                .child(caption(hosted, &said)),
        )],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
