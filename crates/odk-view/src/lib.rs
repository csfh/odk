//! The window ODK draws once GPUI has left (ADR 0089).
//!
//! This crate is the port's host. `odk` does not depend on it, and `odk-ui` still draws with
//! GPUI, until the port's last step. Nothing here links GPUI. A component's own state lives in
//! [`Store`]: an open list, a caret and a held key.

mod badge;
mod button;
mod checkbox;
mod describe;
mod empty;
mod face;
mod host;
mod icon;
mod ink;
mod input;
mod item;
mod item_group;
mod kbd;
mod layout;
mod select;
mod separator;
mod skeleton;
mod status;
mod store;
mod style;
mod switch;
mod text;
mod toggle;
mod tone;

pub use badge::Badge;
pub use button::Button;
pub use checkbox::Checkbox;
pub use empty::Empty;
pub use face::Face;
pub use host::{Hosted, frame, headless, start};
pub use icon::{Glyph, Icon};
pub use ink::ink;
pub use input::Input;
pub use item::Item;
pub use item_group::ItemGroup;
pub use kbd::Kbd;
pub use layout::{Layout, Space};
pub use odk_a11y::{Accessible, Describe, Role};
pub use select::Select;
pub use separator::Separator;
pub use skeleton::Skeleton;
pub use status::{Status, StatusIndicator};
pub use store::Store;
pub use switch::Switch;
pub use text::{Flow, Heading, Text};
pub use tone::Tone;
