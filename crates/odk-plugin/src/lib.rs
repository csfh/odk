//! The Luau Plugin host of ODK applications (ADR 0003, ADR 0020, ADR 0091, ADR 0092).
//!
//! A Plugin is a directory with a `plugin.toml` (`docs/spec/plugin-manifest.md`) and Luau source.
//! The host loads each Plugin into a state of its own, holding only the capabilities the host
//! granted, with a memory limit and a time budget; a Plugin's failure is a value the host hears,
//! and never stops the host or another Plugin. See `docs/spec/plugin-api.md` for the functions a
//! Plugin can call.

mod data;
mod extend;
mod host;
mod manifest;
mod paths;

pub use data::Data;
pub use extend::{
    Contribution, MAX_BODY_BYTES, MAX_CONTRIBUTIONS, MAX_DESCRIPTION_BYTES, MAX_HEADERS,
    MAX_PENDING_REQUESTS, MAX_SUBSCRIPTIONS, MAX_URL_BYTES, NetworkRequest,
};
pub use host::{
    CallError, DEFAULT_CALL_BUDGET, DEFAULT_MEMORY_LIMIT, DEFAULT_STORAGE_LIMIT, ErrorKind, Grants,
    Host, Limits, Phase, PluginError,
};
pub use manifest::{Capability, Manifest, ManifestError};
