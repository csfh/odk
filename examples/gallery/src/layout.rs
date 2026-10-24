//! The gallery's own layout numbers, in one place. They are the structure of the application
//! window (how wide the sidebar is, when the window counts as narrow), not tokens of the design
//! language, so they are not scattered through the views and not read from the theme. Spacing
//! between things is the foundation's.

/// The sidebar's width in a window of ordinary width.
pub const SIDEBAR_WIDTH: f32 = 248.0;
/// The sidebar's width in a narrow window.
pub const SIDEBAR_NARROW_WIDTH: f32 = 188.0;
/// A window narrower than this (logical pixels) has the narrow sidebar.
pub const NARROW_BELOW: f32 = 1000.0;
/// How wide the theme selector is.
pub const THEME_SELECT_WIDTH: f32 = 260.0;
/// The widest the page's content grows; beyond it the page is centred and its lines stay
/// readable.
pub const CONTENT_MAX_WIDTH: f32 = 1120.0;
/// A colour swatch's side.
pub const SWATCH: f32 = 16.0;
/// A wide colour swatch, as in a candidate row.
pub const SWATCH_WIDE: f32 = 56.0;
/// The label column of a table of tokens.
pub const LABEL_WIDTH: f32 = 200.0;
/// The tallest a block of code or a list of diagnostics grows before it scrolls.
pub const SCROLL_HEIGHT: f32 = 360.0;
