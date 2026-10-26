//! The `oma` library: what the `oma` command and other tools, such as the Flea file
//! explorer, share. Today that is the Bundle format in [`fmt`] and assembling a Bundle in [`bundle`].

pub mod bundle;
pub mod consent;
pub mod flow;
pub mod fmt;
pub mod grants;
pub mod install;
pub mod layout;
pub mod omarchy;
pub mod sandbox;
pub mod userdirs;

#[cfg(test)]
mod testutil;
