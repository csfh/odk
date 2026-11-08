//! The Switch story: an on/off switch in every state, with the description and disabled state a
//! `FormField` gives it, each density, a controlled one, and long, unbreakable and non-Latin
//! labels in a narrow column. Run it in watch mode with `tools/story switch`.

use odk::prelude::*;
use stories::{narrow, row, section};

struct SwitchStory {
    wifi: bool,
}

impl Render for SwitchStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let states = row()
            .child(Switch::new(("state", 0usize), "Off"))
            .child(Switch::new(("state", 1usize), "On").default_checked(true))
            .child(Switch::new(("state", 2usize), "Disabled off").disabled(true))
            .child(
                Switch::new(("state", 3usize), "Disabled on")
                    .default_checked(true)
                    .disabled(true),
            );

        // A switch has no invalid state and no description of its own: a FormField gives it a
        // description and the disabled state, and its error does not reach a switch.
        let messages = narrow(
            360.0,
            Stack::new()
                .gap(Space::Md)
                .child(
                    FormField::new("m-description", "Network")
                        .description("Connect to known networks when they are in range.")
                        .child(Switch::new("m-description-switch", "Wi-Fi").default_checked(true)),
                )
                .child(
                    FormField::new("m-disabled", "Bluetooth")
                        .description("Managed by your administrator.")
                        .disabled(true)
                        .child(Switch::new("m-disabled-switch", "Bluetooth")),
                ),
        );

        let densities = Stack::new()
            .gap(Space::Sm)
            .children(Density::ALL.iter().enumerate().map(|(n, &d)| {
                row()
                    .child(
                        Switch::new(("density", n), "Wi-Fi")
                            .default_checked(true)
                            .density(d),
                    )
                    .child(Text::new(d.name()).muted())
            }));

        let view = cx.entity();
        let controlled = Stack::new()
            .gap(Space::Sm)
            .child(
                Switch::new("controlled", "Wi-Fi")
                    .checked(self.wifi)
                    .on_change(move |on, _, cx| {
                        view.update(cx, |this, cx| {
                            this.wifi = on;
                            cx.notify();
                        })
                    }),
            )
            .child(
                Text::new(if self.wifi {
                    "The story holds it on."
                } else {
                    "The story holds it off."
                })
                .muted(),
            );

        let worst = narrow(
            220.0,
            Stack::new()
                .gap(Space::Md)
                .child(Switch::new(("worst", 0usize), "x").default_checked(true))
                .child(Switch::new(
                    ("worst", 1usize),
                    "A label that runs on for far longer than the room it has, to see whether \
                     the track stays put beside it",
                ))
                .child(
                    Switch::new(
                        ("worst", 2usize),
                        "Pneumonoultramicroscopicsilicovolcanoconiosis_and_then_some_more",
                    )
                    .default_checked(true),
                )
                .child(Switch::new(
                    ("worst", 3usize),
                    "利用規約とプライバシーポリシーに同意し、すべての更新通知を受け取ります",
                ))
                .child(
                    Switch::new(("worst", 4usize), "أوافق على الشروط وسياسة الخصوصية")
                        .default_checked(true),
                ),
        );

        root().child(
            scroll_y("switch-story").child(
                Stack::page()
                    .child(Heading::new("Switch").level(1))
                    .child(section(
                        "States",
                        "Tab to see the focus ring; press Space to flip. A switch has no invalid state.",
                        states,
                    ))
                    .child(section(
                        "Description and disabled",
                        "A FormField gives a switch its description and the disabled state; its \
                         error does not reach a switch.",
                        messages,
                    ))
                    .child(section(
                        "Densities",
                        "Compact, default and comfortable.",
                        densities,
                    ))
                    .child(section(
                        "Controlled",
                        "The story passes `checked` and follows `on_change`, which hears each change once.",
                        controlled,
                    ))
                    .child(section(
                        "Worst case",
                        "220 px wide, to the rule: one character, a long label, a word that cannot break, \
                         Japanese and Arabic.",
                        worst,
                    )),
            ),
        )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("dev.csfh.odk.stories.switch")
            .title("Switch")
            .size(900.0, 840.0);
        odk::ui::open_window(cx, window, |_, cx| cx.new(|_| SwitchStory { wifi: false })).unwrap();
    });
}
