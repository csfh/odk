// snippet start
use odk::prelude::*;

// The foundation's spacing, read inside `render` so a change of the user's text size follows.
fn panel() -> impl IntoElement {
    let f = odk::ui::foundation();
    Stack::new()
        .p(odk::gpui::px(f.layout.panel_padding))
        .gap(odk::gpui::px(f.layout.section_gap))
        .child(Heading::new("Panel"))
        .child(Text::new("Spaced by purpose, not by a number."))
}
// snippet end
