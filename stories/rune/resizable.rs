//! The Resizable story on Rune: panels side by side and stacked, with limits, a disabled group, and
//! the sizes the application heard. Run it with `tools/story rune-resizable`.

use odk_view::{Hosted, Layout, Resizable, ResizablePanel, Space, Text};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    heard: Vec<f32>,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let word = |s: &str| Text::new(s.to_string()).show(hosted);
    let heard = if hosted.app.heard.is_empty() {
        "Drag a handle, or Tab to one and use the arrow keys, Home and End.".to_string()
    } else {
        format!(
            "Heard: {}.",
            hosted
                .app
                .heard
                .iter()
                .map(|s| format!("{:.0}%", s * 100.0))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let across = Resizable::horizontal("across", "Services, detail and inspector")
        .panel(
            ResizablePanel::new()
                .default_size(0.3)
                .min_size(0.2)
                .child(word("Services")),
        )
        .panel(ResizablePanel::new().min_size(0.3).child(word("Detail")))
        .panel(
            ResizablePanel::new()
                .default_size(0.25)
                .max_size(0.4)
                .child(word("Inspector")),
        )
        .on_layout(|story: &mut Story, sizes: &[f32]| story.heard = sizes.to_vec())
        .show(hosted);
    let down = Resizable::vertical("down", "Editor and terminal")
        .panel(
            ResizablePanel::new()
                .default_size(0.6)
                .min_size(0.3)
                .child(word("Editor")),
        )
        .panel(ResizablePanel::new().min_size(0.2).child(word("Terminal")))
        .show(hosted);
    let off = Resizable::horizontal("off", "A group that is locked")
        .disabled(true)
        .panel(ResizablePanel::new().default_size(0.5).child(word("Left")))
        .panel(ResizablePanel::new().child(word("Right")))
        .show(hosted);
    let boxed = |element: El, height: f32| col().w(520.0).h(height).child(element);

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Across",
                "Three panels with limits; the handles keep them inside.",
                layout
                    .stack(Space::Xs)
                    .child(boxed(across, 140.0))
                    .child(caption(hosted, &heard)),
            ),
            section(
                hosted,
                "Down",
                "Up and Down move a handle in a column.",
                boxed(down, 200.0),
            ),
            section(
                hosted,
                "Disabled",
                "The handle neither moves nor takes focus.",
                boxed(off, 80.0),
            ),
        ],
        [],
    )
}

fn main() {
    support::run(Story::default(), view);
}
