//! The Checkbox story: a box in every state, with the messages a `FormField` gives it, each
//! density, a controlled one, and long, unbreakable and non-Latin labels in a narrow column.
//! Run it in watch mode with `tools/story checkbox`.

use odk::prelude::*;
use stories::{narrow, row, section};

struct CheckboxStory {
    updates: bool,
}

impl Render for CheckboxStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let states = row()
            .child(Checkbox::new(("state", 0usize), "Unchecked"))
            .child(Checkbox::new(("state", 1usize), "Checked").default_checked(true))
            .child(Checkbox::new(("state", 2usize), "Indeterminate").indeterminate(true))
            .child(Checkbox::new(("state", 3usize), "Disabled").disabled(true))
            .child(
                Checkbox::new(("state", 4usize), "Disabled, checked")
                    .default_checked(true)
                    .disabled(true),
            );

        let messages = narrow(
            360.0,
            Stack::new()
                .gap(Space::Md)
                .child(Checkbox::new("m-invalid", "Accept the terms").invalid(true))
                .child(
                    Checkbox::new("m-required", "Required, announced and not drawn").required(true),
                )
                .child(
                    FormField::new("m-description", "Newsletter")
                        .description("At most one message a month.")
                        .child(Checkbox::new("m-description-box", "Send me product news")),
                )
                .child(
                    FormField::new("m-error", "Terms")
                        .required(true)
                        .error("Accept the terms to continue.")
                        .child(Checkbox::new("m-error-box", "I accept the terms")),
                )
                .child(
                    FormField::new("m-disabled", "Backups")
                        .description("Managed by your administrator.")
                        .disabled(true)
                        .child(Checkbox::new("m-disabled-box", "Back up every night")),
                ),
        );

        let densities = Stack::new()
            .gap(Space::Sm)
            .children(Density::ALL.iter().enumerate().map(|(n, &d)| {
                row()
                    .child(Checkbox::new(("density", n), "Wrap long lines").density(d))
                    .child(Text::new(d.name()).muted())
            }));

        let view = cx.entity();
        let controlled = Stack::new()
            .gap(Space::Sm)
            .child(
                Checkbox::new("controlled", "Email me about updates")
                    .checked(self.updates)
                    .on_change(move |on, _, cx| {
                        view.update(cx, |this, cx| {
                            this.updates = on;
                            cx.notify();
                        })
                    }),
            )
            .child(
                Text::new(if self.updates {
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
                .child(Checkbox::new(("worst", 0usize), "x").default_checked(true))
                .child(Checkbox::new(
                    ("worst", 1usize),
                    "A label that runs on for far longer than the room it has, to see whether \
                     the box stays put beside it",
                ))
                .child(
                    Checkbox::new(
                        ("worst", 2usize),
                        "Pneumonoultramicroscopicsilicovolcanoconiosis_and_then_some_more",
                    )
                    .default_checked(true),
                )
                .child(Checkbox::new(
                    ("worst", 3usize),
                    "利用規約とプライバシーポリシーに同意し、すべての更新通知を受け取ります",
                ))
                .child(
                    Checkbox::new(("worst", 4usize), "أوافق على الشروط وسياسة الخصوصية")
                        .default_checked(true),
                )
                .child(
                    Checkbox::new(
                        ("worst", 5usize),
                        "An invalid box whose label is long enough to wrap onto a second line",
                    )
                    .invalid(true),
                ),
        );

        root().child(
            scroll_y("checkbox-story").child(
                Stack::page()
                    .child(Heading::new("Checkbox").level(1))                    .child(section(
                        "States",
                        "Tab to see the focus ring; press Space to toggle. A ticked box shows a check \
                         and a mixed one a dash, never only a fill.",
                        states,
                    ))
                    .child(section(
                        "Invalid, required and messages",
                        "A box can be invalid on its own, and draws a mark; `required` alone only tells \
                         assistive technology. A FormField adds the `*`, a description, an error and \
                         the disabled state.",
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
                         Japanese, Arabic and a long invalid label.",
                        worst,
                    )),
            ),
        )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("dev.csfh.odk.stories.checkbox")
            .title("Checkbox")
            .size(900.0, 900.0);
        odk::ui::open_window(cx, window, |_, cx| {
            cx.new(|_| CheckboxStory { updates: false })
        })
        .unwrap();
    });
}
