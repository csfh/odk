// snippet start
use odk::gpui::rgba;
use odk::prelude::*;
use odk::theme::semantic::{Emphasis, Role, State};

// Read the resolved colours inside `render`, never the palette: the resolver has already
// checked each pair for contrast.
fn warning_chip() -> impl IntoElement {
    let theme = odk::ui::theme_snapshot();
    let t = theme.semantic.treatment(Role::Warning, Emphasis::Muted, State::Default);
    div()
        .p(Space::Sm)
        .bg(rgba(t.background.to_rgba_u32()))
        .text_color(rgba(t.foreground.to_rgba_u32()))
        .border_1()
        .border_color(rgba(t.border.to_rgba_u32()))
        .child("Check the cable")
}
// snippet end
