//! The window ODK draws once GPUI has left (ADR 0089).
//!
//! This crate is the port's host. `odk` does not depend on it, and `odk-ui` still draws with
//! GPUI, until the port's last step. Nothing here links GPUI. A component's own state lives in
//! [`Store`]: an open list, a caret and a held key.

mod button;
mod face;
mod host;
mod ink;
mod input;
mod select;
mod store;
mod style;

pub use button::Button;
pub use face::Face;
pub use host::{Hosted, frame, headless, start};
pub use ink::ink;
pub use input::Input;
pub use odk_a11y::{Accessible, Describe, Role};
pub use select::Select;
pub use store::Store;
