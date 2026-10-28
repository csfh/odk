//! What a window is opened with, in ODK's own terms: the application's id, its title, the size it
//! opens at and the size it cannot be made smaller than (`docs/spec/odk-ui-components.md`,
//! "Windows"). An application gives plain numbers, so it needs none of GPUI's types.

use gpui::{App, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use odk_theme::TextScale;

/// How a window opens, for [`crate::open_window`]: its application id, title and sizes.
///
/// ```ignore
/// odk::ui::open_window(
///     cx,
///     WindowConfig::new("dev.example.notes").title("Notes").size(960.0, 680.0).min_size(800.0, 560.0),
///     |_, cx| cx.new(|_| Notes::default()),
/// )
/// ```
///
/// The size it opens at is given at the 12 px text size and grows with the user's text size, as a
/// sidebar's width does (ADR 0043), but never past the display. The smallest size is in pixels
/// as given and does not grow: at a large text size the content scrolls before the window
/// outgrows a small display (`docs/checks.md`, "Window sizes").
#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    app_id: SharedString,
    title: Option<SharedString>,
    size: Option<Extent>,
    min_size: Option<Extent>,
}

impl WindowConfig {
    /// A window of the application `app_id`, such as `dev.example.notes`: Wayland's application
    /// id, which the compositor's window rules and the application's `.desktop` entry match on.
    pub fn new(app_id: impl Into<SharedString>) -> Self {
        Self {
            app_id: app_id.into(),
            title: None,
            size: None,
            min_size: None,
        }
    }

    /// The window's title, which the compositor shows in its window list.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The size the window opens at, at the 12 px text size. It grows with the user's text size
    /// and is never larger than the display. Without one the compositor chooses.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// The smallest the window can be made, in pixels as given. Never larger than the display.
    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some((width, height));
        self
    }
}

/// What [`crate::open_window`] takes: a [`WindowConfig`], or GPUI's `WindowOptions` as they are
/// for what a config does not cover.
pub trait IntoWindowOptions {
    fn into_window_options(self, cx: &App) -> WindowOptions;
}

impl IntoWindowOptions for WindowOptions {
    fn into_window_options(self, _: &App) -> WindowOptions {
        self
    }
}

impl IntoWindowOptions for WindowConfig {
    fn into_window_options(self, cx: &App) -> WindowOptions {
        let display = cx.primary_display().map(|d| {
            let s = d.bounds().size;
            (f32::from(s.width), f32::from(s.height))
        });
        let (open, min) = sizes(
            self.size,
            self.min_size,
            crate::foundation().text_scale,
            display,
        );
        WindowOptions {
            app_id: Some(self.app_id.to_string()),
            titlebar: Some(TitlebarOptions {
                title: self.title,
                ..Default::default()
            }),
            window_bounds: open.map(|(w, h)| {
                WindowBounds::Windowed(Bounds::centered(None, size(px(w), px(h)), cx))
            }),
            window_min_size: min.map(|(w, h)| size(px(w), px(h))),
            ..Default::default()
        }
    }
}

/// A width and a height in logical pixels.
type Extent = (f32, f32);

/// The size a window opens at and its smallest size, from what the application asked for, the
/// user's text size and the display's size when it is known: the first grows with the text size,
/// the second does not; neither is larger than the display, and the first is at least the second.
fn sizes(
    open: Option<Extent>,
    min: Option<Extent>,
    text: TextScale,
    display: Option<Extent>,
) -> (Option<Extent>, Option<Extent>) {
    let fit = |(w, h): Extent| match display {
        Some((dw, dh)) => (w.min(dw), h.min(dh)),
        None => (w, h),
    };
    let min = min.map(fit);
    let open = open.map(|(w, h)| {
        let (w, h) = fit((text.px(w), text.px(h)));
        match min {
            Some((mw, mh)) => (w.max(mw), h.max(mh)),
            None => (w, h),
        }
    });
    (open, min)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATMOS: Option<Extent> = Some((960.0, 680.0));
    const ATMOS_MIN: Option<Extent> = Some((800.0, 560.0));

    #[test]
    fn the_size_follows_the_text_size_and_the_smallest_size_does_not() {
        let plain = sizes(ATMOS, ATMOS_MIN, TextScale::default(), None);
        assert_eq!(plain, (ATMOS, ATMOS_MIN));
        let big = sizes(ATMOS, ATMOS_MIN, TextScale::from_base_size(18.0), None);
        assert_eq!(big, (Some((1440.0, 1020.0)), ATMOS_MIN));
    }

    #[test]
    fn neither_is_larger_than_the_display() {
        let laptop = Some((1280.0, 800.0));
        let big = sizes(ATMOS, ATMOS_MIN, TextScale::from_base_size(20.0), laptop);
        assert_eq!(big, (Some((1280.0, 800.0)), ATMOS_MIN));
        let tiny = Some((640.0, 480.0));
        let squeezed = sizes(ATMOS, ATMOS_MIN, TextScale::default(), tiny);
        assert_eq!(squeezed, (Some((640.0, 480.0)), Some((640.0, 480.0))));
    }

    #[test]
    fn the_size_it_opens_at_is_at_least_the_smallest() {
        let small = sizes(Some((400.0, 300.0)), ATMOS_MIN, TextScale::default(), None);
        assert_eq!(small, (ATMOS_MIN, ATMOS_MIN));
        assert_eq!(sizes(None, ATMOS_MIN, TextScale::default(), None).0, None);
    }

    #[gpui::test]
    fn a_config_becomes_the_options_gpui_opens_with(cx: &mut gpui::TestAppContext) {
        crate::theme::install_test_theme();
        let options = cx.update(|cx| {
            WindowConfig::new("dev.example.notes")
                .title("Notes")
                .size(960.0, 680.0)
                .min_size(800.0, 560.0)
                .into_window_options(cx)
        });
        assert_eq!(options.app_id.as_deref(), Some("dev.example.notes"));
        let title = options.titlebar.and_then(|t| t.title);
        assert_eq!(title.as_ref().map(|t| t.as_ref()), Some("Notes"));
        assert_eq!(options.window_min_size, Some(size(px(800.0), px(560.0))));
        let Some(WindowBounds::Windowed(bounds)) = options.window_bounds else {
            panic!("a windowed size");
        };
        assert_eq!(bounds.size, size(px(960.0), px(680.0)));
    }
}
