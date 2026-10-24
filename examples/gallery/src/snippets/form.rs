// snippet start
use odk::prelude::*;

fn sign_up() -> impl IntoElement {
    form()
        .child(
            FormField::new("email-field", "Email")
                .required(true)
                .description("We only write about your account.")
                .child(Input::new("email", "Email")),
        )
        .child(
            FormField::new("terms-field", "Terms")
                .error("Accept the terms to continue.")
                .child(Checkbox::new("terms", "I accept the terms").invalid(true)),
        )
        .child(
            h_flex()
                .gap(Space::Sm)
                .child(Button::new("submit").label("Create account").variant(ButtonVariant::Primary))
                .child(Button::new("cancel").label("Cancel").variant(ButtonVariant::Outline)),
        )
}
// snippet end
