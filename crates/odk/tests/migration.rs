//! The "after" code of `docs/migration.md`, compiled. Each function is one section's code, so a
//! change to an API the guide shows fails here before the guide is wrong. Nothing here opens a
//! window; the functions that start one are checked by being built, not run.

use odk::prelude::*;
use odk::theme::component::ButtonVariant;
use odk::theme::foundation::{CornerRadius, Density, FoundationConfig};
use odk::theme::semantic::{Emphasis, ResolveOptions, Role};
use odk::theme::{ThemeManager, ThemeSource};
use odk::ui::{Badge, StatusIndicator, Tone};

/// The application's own state, which a handler gets and a view reads.
#[derive(Default)]
struct App {
    saved: u32,
}

/// "Starting": `run` loads the theme before the window opens, so a component may read the tokens
/// in the view from its first frame.
#[allow(dead_code)]
fn starting() -> Result<(), String> {
    let window = WindowConfig::new((960.0, 680.0)).title("My app");
    odk::ui::run(App::default(), window, view)
}

fn view(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    frame(hosted, cx, buttons(hosted), [])
}

/// "Density, corners and motion": the window's settings at start, or from a handler afterwards.
#[allow(dead_code)]
fn settings() -> Result<WindowConfig, odk::ui::NoWindow> {
    odk::ui::configure::<App>(|c: &mut FoundationConfig| {
        c.density = Density::Compact;
        c.corner_radius = CornerRadius::Square;
    })?;
    odk::ui::set_reduced_motion::<App>(true)?;
    Ok(
        WindowConfig::new((960.0, 680.0)).foundation(FoundationConfig {
            density: Density::Compact,
            ..FoundationConfig::default()
        }),
    )
}

/// "Previewing a theme": the user's Omarchy theme is not touched.
#[allow(dead_code)]
fn previewing() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = ThemeManager::following_host(ThemeSource::named("nord")?);
    manager.reload();
    odk::ui::preview_theme((*manager.snapshot()).clone())?;
    // ...and back to the theme the system follows.
    odk::ui::end_theme_preview()?;
    Ok(())
}

/// "Colour options": the resolver's options, set once by an application that wants other than
/// the defaults.
#[allow(dead_code)]
fn colour_options() {
    let options = ResolveOptions::default();
    odk::ui::set_colour_options(options).unwrap();
}

/// "Buttons": a handler takes the application's state.
#[allow(dead_code)]
fn buttons(hosted: &Hosted<App>) -> El {
    // Before: Button::new("save").label("Save").variant(Variant::Primary)
    Layout::of(hosted)
        .stack(Space::Sm)
        .child(
            Button::new("save", "Save", |app: &mut App| app.saved += 1)
                .variant(ButtonVariant::Primary)
                .show(hosted),
        )
        .child(
            Button::new("delete", "Delete", |_: &mut App| {})
                .variant(ButtonVariant::Destructive)
                .show(hosted),
        )
        .child(
            Button::new("apply", "Apply", |_: &mut App| {})
                .density(Density::Compact)
                .busy(false)
                .show(hosted),
        )
}

/// "Badges and status".
#[allow(dead_code)]
fn badges(hosted: &Hosted<App>) -> El {
    // Before: Badge::new("New").variant(Variant::Primary)
    Layout::of(hosted)
        .row(Space::Sm)
        .child(
            Badge::new("New")
                .role(Role::Accent)
                .emphasis(Emphasis::Solid)
                .show(hosted),
        )
        .child(
            Badge::new("Failed")
                .tone(Tone::Danger)
                .marked()
                .show(hosted),
        )
        .child(StatusIndicator::new(odk::ui::Status::Active, "Online").show(hosted))
}

/// "Reading tokens": a component of your own reads the layers, never the palette.
#[allow(dead_code)]
fn reading_tokens(hosted: &Hosted<App>) {
    let foundation = hosted.foundation();
    let _height = foundation.controls.metrics().height;
    let _ink = hosted.theme().semantic.text.primary;
    let _tokens = hosted.components();
}
