//! Proves a missing Wayland socket is an error from [`odk_view::start`], on the main thread.
//!
//! winit refuses an event loop from a test thread. This binary is the main thread. It does
//! nothing unless `ODK_VIEW_MISS_DISPLAY=1`, so running it by hand does not open a window.

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, frame, start};
use rune::{Cx, El, TextStyle, text};

fn view(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        text("probe").font(TextStyle::new(12.0, 400.0)),
        [],
    )
}

fn main() {
    if std::env::var("ODK_VIEW_MISS_DISPLAY").as_deref() != Ok("1") {
        eprintln!("odk-view: display-probe runs only from its test");
        std::process::exit(2);
    }
    match start(
        Hosted::new((), ThemeSnapshot::emergency(), Face::bundled()),
        view,
        (200.0, 100.0),
    ) {
        Ok(()) => std::process::exit(3),
        Err(error) if error.is_empty() => std::process::exit(4),
        Err(_) => {}
    }
}
