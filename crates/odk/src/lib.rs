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
pub use odk_ui as ui;
/// The pinned GPUI, for the raw escape hatch (ADR 0004). Applications never depend on `gpui`
/// themselves, so they cannot end up on a different version.
pub use odk_ui::gpui;

/// What an application needs in scope to write a view.
pub mod prelude {
    pub use odk_ui::prelude::*;
}
