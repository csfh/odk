//! The InputGroup story on Rune: quiet text before and after the field, an action inside the box,
//! and a masked group. Run it with `tools/story rune-input_group`.

use odk_theme::component::ButtonVariant;
use odk_view::{Button, Hosted, Icon, InputGroup, Layout, Space};
use rune::{Cx, El};

#[path = "support.rs"]
mod support;
use support::{caption, page, section};

#[derive(Default)]
struct Story {
    pressed: u32,
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let layout = Layout::of(hosted);
    let on_change = |_: &mut Story, _: String| {};
    let on_submit = |_: &mut Story| {};
    let said = format!("The action was pressed {} times.", hosted.app.pressed);
    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Addons and an action",
                "A press on the quiet text focuses the field; the button stays inside the box.",
                layout
                    .stack(Space::Sm)
                    .child(
                        InputGroup::<Story, _, _>::new("site", "Site", on_change, on_submit)
                            .prefix("https://")
                            .suffix(".example")
                            .placeholder("name")
                            .width(320.0)
                            .action(
                                Button::icon("go", Icon::ArrowRight, "Go", |s: &mut Story| {
                                    s.pressed += 1
                                })
                                .variant(ButtonVariant::Ghost),
                            )
                            .show(hosted),
                    )
                    .child(caption(hosted, &said)),
            ),
            section(
                hosted,
                "Other groups",
                "A long prefix truncates at a quarter of the box, a masked group, and disabled.",
                layout
                    .stack(Space::Sm)
                    .child(
                        InputGroup::<Story, _, _>::new("long", "Long prefix", on_change, on_submit)
                            .prefix("https://a-very-long-address-that-cannot-fit.example.org/")
                            .width(320.0)
                            .show(hosted),
                    )
                    .child(
                        InputGroup::<Story, _, _>::new("key", "Key", on_change, on_submit)
                            .prefix("key:")
                            .masked(true)
                            .width(320.0)
                            .show(hosted),
                    )
                    .child(
                        InputGroup::<Story, _, _>::new("off", "Disabled", on_change, on_submit)
                            .suffix("kg")
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
