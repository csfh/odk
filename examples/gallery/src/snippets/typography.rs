// snippet start
use odk::prelude::*;
use odk::theme::foundation::TypeRole;

fn text() -> impl IntoElement {
    Stack::new()
        .gap(Space::Sm)
        .child(Heading::new("Settings").level(1))
        .child(Text::new("Applies to every window."))
        .child(Text::new("Last changed yesterday").muted())
        .child(Text::new("~/.config/omarchy").role(TypeRole::Mono))
        .child(
            Text::new("A file name that is much too long for its column.tar.gz")
                .decorate(EndEllipsis),
        )
}
// snippet end
