//! More than one window (`Windows`), as far as a test can go without a display: the application
//! and its other windows are built with states of their own, and a window is shown and hidden
//! before the application runs without harm. Nothing here opens a window.

use odk_view::{Hosted, WindowConfig, Windows, frame};
use rune::{Cx, El, col};

struct Main;
struct Settings {
    saved: u32,
}

fn main_view(hosted: &Hosted<Main>, cx: &mut Cx<Hosted<Main>>) -> El {
    frame(hosted, cx, col(), [])
}

fn settings_view(hosted: &Hosted<Settings>, cx: &mut Cx<Hosted<Settings>>) -> El {
    let _ = hosted.app.saved;
    frame(hosted, cx, col(), [])
}

#[test]
fn an_application_has_other_windows_with_states_of_their_own() {
    let mut windows = Windows::new(Main, WindowConfig::new((480.0, 360.0)), main_view);
    let settings = windows.window(
        "settings",
        Settings { saved: 0 },
        WindowConfig::new((320.0, 240.0)).title("Settings"),
        settings_view,
    );
    let about = windows.window(
        "about",
        Settings { saved: 1 },
        WindowConfig::new((320.0, 240.0)).title("About"),
        settings_view,
    );
    // Before the application runs they are not made; asking is queued and harmless, and the
    // handle can be kept and cloned for a handler to use.
    let again = settings.clone();
    settings.show();
    again.hide();
    about.focus();
    drop(windows);
}
