//! The Omarchy Development Kit. Applications depend on this crate only and reach everything
//! through its modules (ADR 0028).

pub use odk_a11y as a11y;
/// The database (ADR 0049). Present only with the `db` feature, which is off by default.
#[cfg(feature = "db")]
pub use odk_db as db;

/// The Plugin host (ADR 0091, ADR 0092). Present only with the `plugin` feature, which is off by
/// default.
#[cfg(feature = "plugin")]
pub use odk_plugin as plugin;
pub use odk_theme as theme;
/// The interface: components, the host that draws them and follows the user's theme, and the
/// window (ADR 0089, ADR 0094). It is `odk-view`, on Rune.
pub use odk_view as ui;

/// What an application needs in scope to write a view.
pub mod prelude {
    pub use odk_view::prelude::*;
}
