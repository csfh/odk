//! The FormField story on Rune: a label, a description, a control and a message, with each
//! control a field holds, each kind of message, required and optional, disabled, and worst-case
//! words in a narrow column. Run it with `tools/story rune-form_field`.

use odk_view::{Checkbox, FormField, Hosted, Input, RadioGroup, Select, Switch, form};
use rune::{Cx, El, col};

#[path = "support.rs"]
mod support;
use support::{page, section};

/// A column this many points wide, for words longer than the room.
const NARROW: f32 = 280.0;

#[derive(Default)]
struct Story {
    name: String,
    fruit: Option<usize>,
}

fn fruits() -> Vec<String> {
    ["Apple", "Banana", "Cherry"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn view(hosted: &Hosted<Story>, cx: &mut Cx<Hosted<Story>>) -> El {
    let mut layers = Vec::new();
    let name = |id: &str| {
        Input::new(
            id.to_string(),
            "Name",
            |story: &mut Story, text| story.name = text,
            |_: &mut Story| {},
        )
    };

    let controls = form(hosted)
        .child(
            FormField::new("name", "Name")
                .description("Press the label to focus the field.")
                .required(true)
                .child(name("name-input"))
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("fruit", "Fruit")
                .optional(true)
                .child(Select::new(
                    "fruit-select",
                    "Fruit",
                    fruits(),
                    hosted.app.fruit,
                    |story: &mut Story, index| story.fruit = Some(index),
                ))
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("terms", "Terms")
                .child(Checkbox::new("terms-box", "I agree to the terms"))
                .required(true)
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("push", "Notifications")
                .description("Tell me about activity.")
                .child(Switch::new("push-switch", "Push"))
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("theme", "Theme")
                .child(
                    RadioGroup::new("theme-group", "Theme")
                        .item("light", "Light")
                        .item("dark", "Dark"),
                )
                .show(hosted, &mut layers),
        );

    let messages = form(hosted)
        .child(
            FormField::new("err", "Username")
                .child(name("err-input"))
                .error("That name is taken.")
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("err-select", "Fruit")
                .child(Select::new(
                    "err-fruit",
                    "Fruit",
                    fruits(),
                    None,
                    |_: &mut Story, _| {},
                ))
                .error("Choose a fruit.")
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("warn", "Username")
                .child(name("warn-input"))
                .warning("Short names are easy to guess.")
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("ok", "Username")
                .child(name("ok-input"))
                .success("That name is free.")
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("help", "Username")
                .child(name("help-input"))
                .help("Letters and numbers only.")
                .show(hosted, &mut layers),
        );

    let disabled = FormField::new("off", "Name")
        .description("A disabled field's label and control are both out of the Tab order.")
        .disabled(true)
        .child(name("off-input"))
        .show(hosted, &mut layers);

    let worst = col().w(hosted.theme().text.px(NARROW)).child(
        FormField::new(
            "worst",
            "A label far longer than the room the column gives it, however wide that is",
        )
        .description("Unbreakable_word_with_no_place_to_wrap_at_all_in_the_column, 日本語の文字")
        .required(true)
        .child(name("worst-input"))
        .error("Unbreakable_message_with_no_place_to_wrap_at_all_in_the_column, ünïcödé")
        .show(hosted, &mut layers),
    );

    page(
        hosted,
        cx,
        [
            section(
                hosted,
                "Controls",
                "Input, select, check box, switch and radio group, each with its field's state.",
                controls,
            ),
            section(
                hosted,
                "Messages",
                "An error, a warning, a success and help; the mark means colour is never alone.",
                messages,
            ),
            section(hosted, "Disabled", "The label does nothing.", disabled),
            section(
                hosted,
                "Worst case",
                "Words that do not fit wrap in the column (item 23).",
                worst,
            ),
        ],
        layers,
    )
}

fn main() {
    support::run(Story::default(), view);
}
