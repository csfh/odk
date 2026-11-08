//! The window ODK draws once GPUI has left (ADR 0089).
//!
//! This crate is the port's host. `odk` does not depend on it, and `odk-ui` still draws with
//! GPUI, until the port's last step. Nothing here links GPUI. A component's own state will live
//! in [`Store`], not in the application's model.

mod face;
mod host;
mod ink;

pub use face::Face;
pub use host::{Hosted, Store, frame, headless, start};
pub use ink::ink;
pub use odk_a11y::{Accessible, Describe, Role};
