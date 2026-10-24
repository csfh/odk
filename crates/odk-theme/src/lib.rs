//! Omarchy's current theme as ODK's design tokens (ADR 0019, ADR 0030, ADR 0040).
//!
//! The four layers of ADR 0040 are below; `docs/spec/odk-theme-tokens.md` says which of
//! Omarchy's files each is made from. This crate has no GPUI dependency, so it builds and
//! tests without a window.
//!
//! [`Palette`] is the source layer of ADR 0040 (`docs/spec/odk-source-palette.md`): a theme's
//! `colors.toml` as the theme wrote it. [`ThemeSource`] finds a theme, [`ThemeManager`] loads
//! it, keeps the last one that loaded and publishes a [`ThemeSnapshot`] when it changes, and
//! [`ThemeWatcher`] says when to reload.
//!
//! [`foundation`] is the foundation layer of ADR 0040 (`docs/spec/odk-foundation-tokens.md`):
//! ODK's own geometry, density, spacing, type and motion.
//!
//! [`semantic`] is the semantic layer of ADR 0040 (`docs/spec/odk-semantic-tokens.md`): colours
//! by role and emphasis, made from a [`Palette`] by an adaptive resolver that works in OKLCH
//! ([`Oklch`]) and checks contrast as drawn. Every [`ThemeSnapshot`] carries them; components
//! draw them through the component layer.
//!
//! [`component`] is the component layer of ADR 0040 (`docs/spec/odk-component-tokens.md`): each
//! component's style by variant, density and state, its colours named from the semantic layer and
//! its geometry derived from the foundation. Every snapshot's semantic colours have passed its
//! checks. Every component draws it.

mod color;
pub mod component;
mod compositor;
mod discovery;
mod error;
pub mod foundation;
mod load;
mod manager;
mod oklch;
mod palette;
mod resolve;
mod scale;
pub mod semantic;
mod watch;

pub use color::{
    Color, ColorError, LARGE_TEXT_CONTRAST, NON_TEXT_CONTRAST, TEXT_CONTRAST, drawn_contrast,
    linear_to_srgb, srgb_to_linear,
};
pub use compositor::{Hyprland, ReloadWatcher};
pub use discovery::{ThemeFiles, ThemeSource, current_theme_dir, theme_name};
pub use error::Error;
pub use manager::{Failure, Fallback, Reload, ThemeDiagnostics, ThemeManager, ThemeSnapshot};
pub use oklch::{JND, Oklab, Oklch, hue_distance};
pub use palette::{Mode, Palette, PaletteKey};
pub use scale::{TextScale, user_reduces_motion};
pub use watch::ThemeWatcher;
