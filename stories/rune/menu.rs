//! The Menu story on Rune: a dropdown with headings, hints, icons, a check item, radios, a
//! disabled item and a danger item. Run it with `tools/story rune-menu`.

use odk_view::{DropdownMenu, Hosted, Icon, Layout, Menu, MenuItem, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    open: bool,
    autosave: bool,
    large: bool,
    last: String,
}

fn said(label: &'static str) -> impl Fn(&mut Story) + 'static {
    move |s: &mut Story| s.last = format!("Chose {label}.")
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let mut layers = Vec::new();
    let menu = Menu::<Story>::new()
        .heading("File")
        .item(
            MenuItem::new("New")
                .icon(Icon::Plus)
                .hint("Ctrl+N")
                .on_select(said("New")),
        )
        .item(
            MenuItem::new("Open…")
                .icon(Icon::FolderOpen)
                .hint("Ctrl+O")
                .on_select(said("Open")),
        )
        .separator()
        .heading("Options")
        .item(
            MenuItem::new("Autosave")
                .checked(story.autosave)
                .on_select(|s: &mut Story| s.autosave = !s.autosave),
        )
        .item(
            MenuItem::new("Small text")
                .radio(!story.large)
                .on_select(|s: &mut Story| s.large = false),
        )
        .item(
            MenuItem::new("Large text")
                .radio(story.large)
                .on_select(|s: &mut Story| s.large = true),
        )
        .separator()
        .item(MenuItem::new("Export (not available)").disabled(true))
        .item(
            MenuItem::new("Delete project")
                .danger()
                .on_select(said("Delete")),
        );
    let trigger = DropdownMenu::<Story>::new("actions", "Actions")
        .menu(menu)
        .open(story.open)
        .on_open_change(|s: &mut Story, open| s.open = open)
        .show(hosted, &mut layers);
    let said = if story.last.is_empty() {
        "Enter, Space or Down opens it; arrows, Home, End and letters move; Escape closes."
            .to_string()
    } else {
        story.last.clone()
    };
    page(
        hosted,
        cx,
        [section(
            hosted,
            "Dropdown menu",
            "A button that opens a list below itself.",
            layout
                .stack(Space::Sm)
                .child(trigger)
                .child(caption(hosted, &said)),
        )],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
