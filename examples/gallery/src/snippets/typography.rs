// snippet start
use odk::prelude::*;
use odk::theme::foundation::TypeRole;

fn text() -> impl IntoElement {
    v_flex()
        .gap(Space::Sm)
        .child(Heading::new("Settings").level(1))
        .child(Text::new("Applies to every window."))
        .child(Text::new("Last changed yesterday").muted())
        // Any element, in one of the foundation's type roles:
        .child(div().text_role(TypeRole::Mono).child("~/.config/omarchy"))
}
// snippet end
