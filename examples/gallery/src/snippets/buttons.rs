// snippet start
use odk::prelude::*;

fn actions() -> impl IntoElement {
    h_flex()
        .gap(Space::Sm)
        .child(Button::new("save").label("Save").variant(ButtonVariant::Primary))
        .child(Button::new("cancel").label("Cancel").variant(ButtonVariant::Outline))
        .child(
            Button::new("delete")
                .leading_icon("×")
                .label("Delete")
                .variant(ButtonVariant::Destructive)
                .emphasis(Emphasis::Muted),
        )
        .child(
            Button::icon("settings", "⚙", "Settings")
                .variant(ButtonVariant::Ghost)
                .density(Density::Compact),
        )
        .child(Button::new("busy").label("Saving…").loading(true))
        .child(Button::new("off").label("Unavailable").disabled(true))
}
// snippet end
