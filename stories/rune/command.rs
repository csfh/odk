//! The Command story on Rune: a palette that opens over the page, filters as you type, and runs the
//! highlighted command on Enter. Run it with `tools/story rune-command`.

use odk_view::{Button, Command, CommandItem, Hosted, Layout, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    open: bool,
    last: String,
}

fn said(label: &'static str) -> impl Fn(&mut Story) + 'static {
    move |s: &mut Story| s.last = format!("Ran {label}.")
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let story = &hosted.app;
    let mut layers = Vec::new();
    let item = |label: &'static str| CommandItem::<Story>::new(label).on_select(said(label));
    Command::<Story>::new("palette", "Command palette")
        .open(story.open)
        .on_close(|s: &mut Story| s.open = false)
        .returns_focus_to("open")
        .item(item("New file").hint("Ctrl+N").keywords("create document"))
        .item(item("Open file…").hint("Ctrl+O"))
        .item(item("Save").hint("Ctrl+S").keywords("write disk"))
        .item(item("Save as…").hint("Ctrl+Shift+S"))
        .item(
            item("Export (nothing to export)")
                .disabled(true)
                .keywords("share"),
        )
        .item(item("Toggle sidebar").hint("Ctrl+B").keywords("panel"))
        .item(item("Change theme").keywords("colours dark light"))
        .item(item("Settings").hint("Ctrl+,").keywords("preferences"))
        .item(item("Quit").hint("Ctrl+Q").keywords("exit close"))
        .show(hosted, &mut layers);
    let said = if story.last.is_empty() {
        "Type to filter, Up and Down to move, Enter to run, Escape to close.".to_string()
    } else {
        story.last.clone()
    };
    page(
        hosted,
        cx,
        [section(
            hosted,
            "Command palette",
            "A field and a list in a panel hanging from the top of the window.",
            layout
                .stack(Space::Sm)
                .child(
                    Button::new("open", "Open the palette", |s: &mut Story| s.open = true)
                        .show(hosted),
                )
                .child(caption(hosted, &said)),
        )],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
