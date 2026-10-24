// snippet start
use odk::prelude::*;
use odk::theme::foundation::CornerRadius;

// Density and corners are the application's choice, made once; no component picks its own.
fn tighten(cx: &mut App) {
    odk::ui::configure(cx, |c| {
        c.density = Density::Compact;
        c.corner_radius = CornerRadius::Square;
    });
}

// A control may take a density of its own:
fn compact_button() -> Button {
    Button::new("tight").label("Tight").density(Density::Compact)
}
// snippet end
