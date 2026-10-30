//! The Button story: one component in every variant, density, content layout, state and emphasis,
//! for looking at while its code changes. Run it in watch mode with `tools/story button`.

use odk::gpui::Div;
use odk::prelude::*;

struct ButtonStory {
    saving: bool,
}

/// A titled group of children, with a line saying what it shows.
fn section(title: &'static str, note: &'static str, body: impl IntoElement) -> Div {
    v_flex()
        .gap(Space::Sm)
        .child(Heading::new(title).level(2))
        .child(Text::new(note).muted())
        .child(body)
}

fn row() -> Div {
    h_flex().flex_wrap().gap(Space::Sm)
}

impl Render for ButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let variants = row().children(
            ButtonVariant::ALL
                .iter()
                .enumerate()
                .map(|(n, v)| Button::new(("variant", n)).label(v.name()).variant(*v)),
        );

        let densities = v_flex()
            .gap(Space::Sm)
            .children(Density::ALL.iter().enumerate().map(|(n, &d)| {
                row()
                    .child(
                        Button::new(("d-text", n))
                            .label("Save changes")
                            .variant(ButtonVariant::Primary)
                            .density(d),
                    )
                    .child(
                        Button::new(("d-lead", n))
                            .leading_icon(Icon::Check)
                            .label("Apply")
                            .density(d),
                    )
                    .child(
                        Button::new(("d-trail", n))
                            .label("Continue")
                            .trailing_icon(Icon::ArrowRight)
                            .variant(ButtonVariant::Outline)
                            .density(d),
                    )
                    .child(
                        Button::icon(("d-icon", n), Icon::Settings, "Settings")
                            .variant(ButtonVariant::Ghost)
                            .density(d),
                    )
                    .child(Text::new(d.name()).muted())
            }));

        let states = row()
            .child(Button::new("s-default").label("Default"))
            .child(Button::new("s-disabled").label("Disabled").disabled(true))
            .child(
                Button::new("s-loading")
                    .label("Loading")
                    .variant(ButtonVariant::Primary)
                    .loading(true),
            )
            .child(
                Button::new("s-try")
                    .leading_icon(Icon::Check)
                    .label(if self.saving { "Saving…" } else { "Save" })
                    .variant(ButtonVariant::Primary)
                    .loading(self.saving)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.saving = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("s-stop")
                    .label("Stop")
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.saving = false;
                        cx.notify();
                    })),
            );

        let emphasis = v_flex().gap(Space::Sm).children(
            [
                ButtonVariant::Primary,
                ButtonVariant::Destructive,
                ButtonVariant::Success,
                ButtonVariant::Warning,
            ]
            .iter()
            .enumerate()
            .map(|(n, &v)| {
                row().children(Emphasis::ALL.iter().enumerate().map(move |(m, &e)| {
                    Button::new(("emphasis", n * 3 + m))
                        .label(format!("{} {}", v.name(), e.name()))
                        .variant(v)
                        .emphasis(e)
                }))
            }),
        );

        root().child(
            scroll_y("button-story").child(
                content_column()
                    .child(Heading::new("Button").level(1))
                    .child(section(
                        "Variants",
                        "Tab to see the focus ring; hover and press with the pointer.",
                        variants,
                    ))
                    .child(section(
                        "Densities and content",
                        "Text, leading icon, trailing icon and icon alone, at each density.",
                        densities,
                    ))
                    .child(section(
                        "States",
                        "Default, disabled, loading, and a Save that loads until Stop.",
                        states,
                    ))
                    .child(section(
                        "Emphasis",
                        "Solid, muted and subtle for each role that acts.",
                        emphasis,
                    )),
            ),
        )
    }
}

fn main() {
    odk::ui::run(|cx| {
        let window = WindowConfig::new("dev.csfh.odk.stories.button")
            .title("Button")
            .size(900.0, 760.0);
        odk::ui::open_window(cx, window, |_, cx| {
            cx.new(|_| ButtonStory { saving: false })
        })
        .unwrap();
    });
}
