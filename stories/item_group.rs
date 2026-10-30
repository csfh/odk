//! The ItemGroup story: `ItemGroup` and `Item` with every slot (description, lines, actions,
//! footer), both group variants, each density, and long words beside wide actions in a narrow
//! width. Run it in watch mode with `tools/story item_group`.

use odk::gpui::{Div, px};
use odk::prelude::*;

struct ItemGroupStory;

/// A titled group of children, with a line saying what it shows.
fn section(title: &'static str, note: &'static str, body: impl IntoElement) -> Div {
    v_flex()
        .gap(Space::Sm)
        .child(Heading::new(title).level(3))
        .child(Text::new(note).muted())
        .child(body)
}

/// Every slot an item has, in one group.
fn settings(variant: ItemGroupVariant, key: &'static str) -> ItemGroup {
    ItemGroup::new("Input")
        .description("How the pointer and keyboard behave.")
        .variant(variant)
        .child(
            Item::new("Natural scrolling")
                .description("Content follows the fingers, as on a touch screen.")
                .child(Text::new("Applies to touchpads only.").mono().muted())
                .action(Switch::new((key, 0usize), "Natural scrolling").default_checked(true)),
        )
        .child(
            Item::new("Acceleration")
                .description("How far the pointer moves for how fast the hand does.")
                .action(
                    Select::new((key, 1usize), "Acceleration")
                        .item("default", "Default")
                        .item("flat", "Flat")
                        .item("adaptive", "Adaptive")
                        .default_value("default"),
                ),
        )
        .child(
            Item::new("Speed")
                .description("A footer is as wide as the item, for a control that needs room.")
                .footer(
                    NumberInput::new((key, 2usize), "Speed")
                        .min(-1.0)
                        .max(1.0)
                        .step(0.1)
                        .default_value(-0.6),
                ),
        )
        .child(
            Item::new("Work profile")
                .description("Two actions at the end, each at its own width.")
                .action(Button::new((key, 3usize)).label("Apply"))
                .action(
                    Button::icon((key, 4usize), Icon::Star, "Add to favorites")
                        .variant(ButtonVariant::Ghost),
                ),
        )
        .child(
            Item::new("Keyboard layout").action(Badge::new("Unknown").tone(Tone::Warning).marked()),
        )
}

impl Render for ItemGroupStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let densities = v_flex()
            .gap(Space::Lg)
            .children(Density::ALL.iter().enumerate().map(|(n, &d)| {
                ItemGroup::new(format!("{} density", d.name()))
                    .child(
                        Item::new("Notifications")
                            .description("Show a banner when something needs you.")
                            .density(d)
                            .action(Switch::new(("d-switch", n), "Notifications")),
                    )
                    .child(
                        Item::new("Sound")
                            .density(d)
                            .action(Button::new(("d-button", n)).label("Choose…")),
                    )
            }));

        // The words wrap in the room the actions leave; the actions keep their own width.
        let narrow = div().w(px(380.0)).child(
            ItemGroup::new("Narrow")
                .variant(ItemGroupVariant::Outline)
                .child(
                    Item::new("A title long enough that it has to wrap onto a second line")
                        .description(
                            "And a description that runs on for a while, so the words give way \
                             to the action and never the action to the words.",
                        )
                        .action(
                            Button::new("n-wide")
                                .label("A wide action")
                                .variant(ButtonVariant::Primary),
                        ),
                )
                .child(
                    Item::new("Unbroken_identifier_with_no_place_to_break_it_at_all")
                        .action(Switch::new("n-switch", "Unbroken")),
                ),
        );

        let edges = v_flex()
            .gap(Space::Lg)
            .child(
                ItemGroup::new("One item")
                    .child(Item::new("Only").description("No line above or below it.")),
            )
            .child(ItemGroup::new("Empty"))
            .child(
                ItemGroup::new("Without a description")
                    .child(Item::new("First"))
                    .child(Item::new("Second"))
                    .child(Item::new("Third")),
            );

        root().child(
            scroll_y("item-group-story").child(
                content_column()
                    .child(Heading::new("ItemGroup").level(1))
                    .child(section(
                        "Default",
                        "On the surface it is on, with a line between one item and the next.",
                        settings(ItemGroupVariant::Default, "default"),
                    ))
                    .child(section(
                        "Outline",
                        "Inside a box with an edge and no fill. Tab reaches the actions, not the items.",
                        settings(ItemGroupVariant::Outline, "outline"),
                    ))
                    .child(section(
                        "Densities",
                        "The padding above and below an item, at each density.",
                        densities,
                    ))
                    .child(section(
                        "Narrow",
                        "Long words beside a wide action, in 380 px.",
                        narrow,
                    ))
                    .child(section(
                        "Edges",
                        "One item, no items, and items with no descriptions.",
                        edges,
                    )),
            ),
        )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("dev.csfh.odk.stories.item-group")
            .title("ItemGroup")
            .size(900.0, 860.0);
        odk::ui::open_window(cx, window, |_, cx| cx.new(|_| ItemGroupStory)).unwrap();
    });
}
