//! The usage examples each page shows. Each is a real source file under `snippets/`, compiled
//! with the gallery (so an example that stops building with the API breaks the build) and
//! shown as the text between its `snippet start` and `snippet end` lines.

use crate::registry::Page;

macro_rules! snippets {
    ($($module:ident => $page:ident),* $(,)?) => {
        $(
            #[allow(dead_code, unused)]
            mod $module;
        )*

        /// The usage example of `page`, if it has one.
        pub fn usage(page: Page) -> Option<&'static str> {
            match page {
                $(Page::$page => Some(between(include_str!(concat!("snippets/", stringify!($module), ".rs")))),)*
                _ => None,
            }
        }

        /// Every example, for the tests that check each page's is there and has its markers.
        pub fn all() -> Vec<(Page, &'static str)> {
            vec![$((Page::$page, between(include_str!(concat!("snippets/", stringify!($module), ".rs"))))),*]
        }
    };
}

snippets! {
    buttons => Buttons,
    fields => Fields,
    selection => Selection,
    form => Form,
    navigation => Navigation,
    feedback => Feedback,
    overlays => Overlays,
    dialogs => Dialogs,
    typography => Typography,
    colors => Colors,
    spacing => Spacing,
    geometry => Geometry,
    motion => Motion,
}

/// The lines between the markers.
fn between(source: &'static str) -> &'static str {
    let start = source
        .find("// snippet start\n")
        .map_or(0, |i| i + "// snippet start\n".len());
    let end = source.find("// snippet end").unwrap_or(source.len());
    source[start..end].trim_end()
}
