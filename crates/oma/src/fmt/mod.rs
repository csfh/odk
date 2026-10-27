//! The `.oma` Bundle format: parsing the Manifest and validating a Bundle directory.
//!
//! Normative rules live in `docs/spec/manifest.md`. This module reads and checks; it never
//! runs anything from a Bundle.

mod bundle;
mod error;
mod identifier;
mod manifest;
mod wording;

pub use bundle::Bundle;
pub use error::Error;
pub use identifier::Identifier;
pub use manifest::{Access, Daemon, FsGrant, Manifest, Permissions, UserDir, precedence};
pub use semver::Version;
pub use wording::permission_lines;
