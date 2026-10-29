//! The `oma` library: what the `oma` command and other tools, such as the Flea file
//! explorer, share. Today that is the Bundle format in [`fmt`] and assembling a Bundle in [`bundle`].

pub mod bundle;
pub mod consent;
pub mod daemon;
pub mod dev;
pub mod entry;
pub mod fetch;
pub mod flow;
pub mod fmt;
pub mod grants;
pub mod image;
pub mod install;
pub mod layout;
pub mod sandbox;
pub mod trust;
pub mod userdirs;
pub mod view;

#[cfg(test)]
mod testutil;
