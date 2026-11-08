//! The ToggleGroup story: a segmented control that picks one option or several, with text,
//! icons, a disabled option and a disabled group, each density, a controlled one, and one option,
//! many options, long and non-Latin labels and no options in a narrow column. Run it in watch
//! mode with `tools/story toggle_group`.

use odk::prelude::*;
use stories::{narrow, row, section};

struct ToggleGroupStory {
    view_mode: Vec<SharedString>,
}

impl Render for ToggleGroupStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choice = Stack::new()
            .gap(Space::Sm)
            .child(
                ToggleGroup::new("single", "Alignment")
                    .item("left", "Left")
                    .item("center", "Center")
                    .item("right", "Right")
                    .default_value(["left"]),
            )
            .child(
                ToggleGroup::new("multiple", "Text style")
                    .multiple(true)
                    .icon_item("bold", Icon::Bold, "Bold")
                    .icon_item("italic", Icon::Italic, "Italic")
                    .icon_item("underline", Icon::Underline, "Underline")
                    .default_value(["bold", "italic"]),
            );

        // `disable` and `icon` act on the items already added, so they come after their `item`.
        let disabled = Stack::new()
            .gap(Space::Sm)
            .child(
                ToggleGroup::new("one-disabled", "Alignment")
                    .item("left", "Left")
                    .item("center", "Center")
                    .item("right", "Right")
                    .disable("center")
                    .default_value(["left"]),
            )
            .child(
                ToggleGroup::new("all-disabled", "Alignment")
                    .item("left", "Left")
                    .item("center", "Center")
                    .item("right", "Right")
                    .default_value(["center"])
                    .disabled(true),
            );

        let densities = Stack::new()
            .gap(Space::Sm)
            .children(Density::ALL.iter().enumerate().map(|(n, &d)| {
                row()
                    .child(
                        ToggleGroup::new(("density", n), "Alignment")
                            .item("left", "Left")
                            .item("center", "Center")
                            .item("right", "Right")
                            .default_value(["left"])
                            .density(d),
                    )
                    .child(Text::new(d.name()).muted())
            }));

        let view = cx.entity();
        let shown = if self.view_mode.is_empty() {
            "nothing".to_string()
        } else {
            self.view_mode
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let controlled = Stack::new()
            .gap(Space::Sm)
            .child(
                ToggleGroup::new("controlled", "View")
                    .icon_item("list", Icon::List, "List")
                    .icon_item("grid", Icon::Grid, "Grid")
                    .value(self.view_mode.clone())
                    .on_change(move |value: &[SharedString], _, cx| {
                        let value = value.to_vec();
                        view.update(cx, |this, cx| {
                            this.view_mode = value;
                            cx.notify();
                        })
                    }),
            )
            .child(Text::new(format!("The story holds: {shown}.")).muted());

        let worst = narrow(
            260.0,
            Stack::new()
                .gap(Space::Md)
                .child(
                    ToggleGroup::new(("worst", 0usize), "One option")
                        .item("only", "Only")
                        .default_value(["only"]),
                )
                .child(
                    ToggleGroup::new(("worst", 1usize), "Weekday")
                        .item("mon", "Mon")
                        .item("tue", "Tue")
                        .item("wed", "Wed")
                        .item("thu", "Thu")
                        .item("fri", "Fri")
                        .item("sat", "Sat")
                        .item("sun", "Sun")
                        .multiple(true)
                        .default_value(["mon", "fri"]),
                )
                .child(
                    ToggleGroup::new(("worst", 2usize), "Long labels")
                        .item(
                            "a",
                            "A label that runs on for far longer than the room it has",
                        )
                        .item("b", "Pneumonoultramicroscopicsilicovolcanoconiosis")
                        .default_value(["a"]),
                )
                .child(
                    ToggleGroup::new(("worst", 3usize), "Language")
                        .item("ja", "日本語")
                        .item("ar", "العربية")
                        .item("en", "English")
                        .default_value(["ja"]),
                )
                .child(ToggleGroup::new(("worst", 4usize), "No options")),
        );

        root().child(
            scroll_y("toggle-group-story").child(
                Stack::page()
                    .child(Heading::new("ToggleGroup").level(1))
                    .child(section(
                        "One or several",
                        "Tab to the group and use the arrow keys to move, Space to choose. The first \
                         picks one option, the second any number, with icons alone.",
                        choice,
                    ))
                    .child(section(
                        "Disabled",
                        "One option disabled, which the arrow keys skip, and a whole group disabled.",
                        disabled,
                    ))
                    .child(section(
                        "Densities",
                        "Compact, default and comfortable.",
                        densities,
                    ))
                    .child(section(
                        "Controlled",
                        "The story passes `value` and follows `on_change`.",
                        controlled,
                    ))
                    .child(section(
                        "Worst case",
                        "260 px wide, to the rule: one option, seven, long labels, Japanese and Arabic, and a \
                         group with no options at all, which draws nothing.",
                        worst,
                    )),
            ),
        )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("dev.csfh.odk.stories.toggle-group")
            .title("ToggleGroup")
            .size(900.0, 920.0);
        odk::ui::open_window(cx, window, |_, cx| {
            cx.new(|_| ToggleGroupStory {
                view_mode: vec!["list".into()],
            })
        })
        .unwrap();
    });
}
