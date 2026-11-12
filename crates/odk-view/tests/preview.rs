//! Previewing a theme without choosing it. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, NoWindow, end_theme_preview, preview_theme};

fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(Face::bundled)
}

fn palette(background: &str) -> ThemeSnapshot {
    ThemeSnapshot::from_toml(
        &format!(
            r##"
mode = "dark"
accent = "#d0d0d0"
muted = "#8a8a8a"
background = "{background}"
dark_background = "#141414"
lighter_background = "#2a2a2a"
foreground = "#d0d0d0"
red = "#d0d0d0"
yellow = "#d0d0d0"
green = "#d0d0d0"
blue = "#d0d0d0"
"##
        ),
        None,
    )
    .expect("a palette")
}

fn host() -> Hosted<()> {
    Hosted::new((), ThemeSnapshot::emergency(), face())
}

#[test]
fn a_previewed_theme_is_drawn_and_the_followed_one_is_kept() {
    let mut host = host();
    let followed = host.followed_theme().clone();
    let shown = palette("#202a30");
    assert_ne!(followed, shown);
    host.preview_theme(shown.clone());
    assert_eq!(host.theme(), &shown, "drawn");
    assert_eq!(host.previewed_theme(), Some(&shown));
    assert_eq!(
        host.followed_theme(),
        &followed,
        "the followed theme is still what it was"
    );
    host.end_theme_preview();
    assert_eq!(host.theme(), &followed);
    assert_eq!(host.previewed_theme(), None);
}

#[test]
fn a_change_of_the_followed_theme_during_a_preview_is_kept_and_drawn_when_it_ends() {
    let mut host = host();
    let shown = palette("#202a30");
    host.preview_theme(shown.clone());
    let newer = palette("#303030");
    host.set_theme(newer.clone());
    assert_eq!(host.theme(), &shown, "not drawn over the preview");
    assert_eq!(host.followed_theme(), &newer);
    host.end_theme_preview();
    assert_eq!(host.theme(), &newer, "as it is now, not as it was");
}

#[test]
fn the_users_request_for_reduced_motion_is_the_followed_themes_whatever_the_preview_says() {
    let mut host = host();
    let mut followed = host.followed_theme().clone();
    followed.reduced_motion = true;
    host.set_theme(followed);
    let mut shown = palette("#202a30");
    shown.reduced_motion = false;
    host.preview_theme(shown);
    assert!(host.theme().reduced_motion, "the request is not a theme's");
    // And it follows the user under a preview too.
    let mut calmer = host.followed_theme().clone();
    calmer.reduced_motion = false;
    host.set_theme(calmer);
    assert!(!host.theme().reduced_motion);
}

#[test]
fn ending_a_preview_that_is_not_there_does_nothing_and_no_window_is_an_error() {
    let mut host = host();
    let before = host.theme().clone();
    host.end_theme_preview();
    assert_eq!(host.theme(), &before);
    assert_eq!(preview_theme(palette("#202a30")), Err(NoWindow));
    assert_eq!(end_theme_preview(), Err(NoWindow));
}
