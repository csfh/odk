//! The "after" code of `docs/migration.md`, compiled. Each function is one section's code, so a
//! change to an API the guide shows fails here before the guide is wrong. Nothing here opens a
//! window; the functions that need an `App` are checked by being built, not run.

use odk::prelude::*;
use odk::theme::foundation::CornerRadius;
use odk::theme::semantic::{ResolveOptions, Role};
use odk::theme::{ThemeManager, ThemeSource};
use odk::ui::{Density, Emphasis};

/// "Starting": `run` loads the theme before the first window, so a component may read the tokens
/// in `render` from its first frame.
#[allow(dead_code)]
fn starting() {
    odk::ui::run(|cx| {
        odk::ui::open_window(cx, WindowOptions::default(), |_, cx| cx.new(|_| Empty)).unwrap();
    });
}

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        root()
    }
}

/// "Density, corners and motion".
#[allow(dead_code)]
fn settings(cx: &mut App) {
    odk::ui::configure(cx, |c| {
        c.density = Density::Compact;
        c.corner_radius = CornerRadius::Square;
    });
    odk::ui::set_reduced_motion(cx, true);
}

/// "Previewing a theme": the user's Omarchy theme is not touched.
#[allow(dead_code)]
fn previewing(cx: &mut App) -> Result<(), odk::theme::Error> {
    let mut manager = ThemeManager::with_users_files(ThemeSource::named("nord")?);
    manager.reload();
    odk::ui::preview_theme(cx, manager.snapshot());
    // ...and back to the theme the system follows.
    odk::ui::end_theme_preview(cx);
    Ok(())
}

/// "Colour options": the resolver's options, set once by an application that wants other than the defaults.
#[allow(dead_code)]
fn colour_options(cx: &mut App) {
    let options = ResolveOptions::default();
    odk::ui::set_colour_options(cx, options).unwrap();
}

/// "Buttons".
#[allow(dead_code)]
fn buttons(cx: &mut Context<Empty>) -> impl IntoElement {
    // Before: Button::new("save").label("Save").variant(Variant::Primary)
    v_flex()
        .gap(Space::Sm)
        .child(
            Button::new("save")
                .label("Save")
                .variant(ButtonVariant::Primary),
        )
        .child(
            Button::new("delete")
                .label("Delete")
                .variant(ButtonVariant::Destructive)
                .emphasis(Emphasis::Muted),
        )
        .child(Button::icon("settings", "⚙", "Settings").variant(ButtonVariant::Ghost))
        .child(
            Button::new("apply")
                .label("Apply")
                .density(Density::Compact)
                .loading(false)
                .on_click(cx.listener(|_, _, _, cx| cx.notify())),
        )
}

/// "Badges and status".
#[allow(dead_code)]
fn badges() -> impl IntoElement {
    // Before: Badge::new("New").variant(Variant::Primary)
    h_flex()
        .gap(Space::Sm)
        .child(
            Badge::new("New")
                .role(Role::Accent)
                .emphasis(Emphasis::Solid),
        )
        .child(Badge::new("Failed").tone(Tone::Danger).marked())
        .child(StatusIndicator::new(Status::Active, "Online"))
}

/// "Reading tokens": a component of your own reads the layers, never the palette.
#[allow(dead_code)]
fn reading_tokens() {
    let foundation = odk::ui::foundation();
    let _height = foundation.controls.metrics().height;
    let snapshot = odk::ui::theme_snapshot();
    let _ink = snapshot.semantic.text.primary;
    let _tokens = odk::ui::components();
}
