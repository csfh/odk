//! `plugin.toml` (`docs/spec/plugin-manifest.md`, version 1): read strictly. An invalid manifest
//! means the Plugin is not loaded.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::paths::clean_relative;

/// A part of the Plugin API (ADR 0020). v1 defines exactly four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Read the state the host application chose to expose.
    AppState,
    /// Add to the host application's interface through the ODK UI layer.
    Ui,
    /// Make outbound requests through the host application.
    Network,
    /// Read and write the Plugin's own private directory, and nothing else.
    Storage,
}

impl Capability {
    pub const ALL: [Capability; 4] = [
        Capability::AppState,
        Capability::Ui,
        Capability::Network,
        Capability::Storage,
    ];

    /// The name a manifest gives it.
    pub fn name(self) -> &'static str {
        match self {
            Capability::AppState => "app.state",
            Capability::Ui => "ui",
            Capability::Network => "network",
            Capability::Storage => "storage",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// The only manifest version this reads.
const VERSION: i64 = 1;

/// What the keys of `plugin.toml` are. Any other key is an error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    plugin_manifest_version: i64,
    id: String,
    name: String,
    version: String,
    entry: String,
    capabilities: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: semver::Version,
    /// The Luau file to run, relative to the Plugin's directory.
    pub entry: String,
    /// The capabilities the Plugin needs, without repeats, in the order of [`Capability`].
    pub capabilities: Vec<Capability>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("not a valid plugin.toml: {0}")]
    Parse(String),
    #[error("plugin_manifest_version is {0}, and only 1 is read")]
    Version(i64),
    #[error(
        "`id` {0:?} is not an identifier: two or more dot-separated segments, each a lowercase letter then lowercase letters, digits or `-`, at most 128 characters"
    )]
    Id(String),
    #[error("`name` must be non-empty text with no control characters")]
    Name,
    #[error("`version` {0:?} is not a semantic version: {1}")]
    Semver(String, String),
    #[error("`entry` {0:?} must be a relative path with no `..` inside the Plugin's directory")]
    Entry(String),
    #[error(
        "`capabilities` names {0:?}, which is not a capability (app.state, ui, network, storage)"
    )]
    Capability(String),
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let raw: Raw = toml::from_str(text).map_err(|e| ManifestError::Parse(e.to_string()))?;
        if raw.plugin_manifest_version != VERSION {
            return Err(ManifestError::Version(raw.plugin_manifest_version));
        }
        if !is_identifier(&raw.id) {
            return Err(ManifestError::Id(raw.id));
        }
        if raw.name.is_empty() || raw.name.chars().any(char::is_control) {
            return Err(ManifestError::Name);
        }
        let version = semver::Version::parse(&raw.version)
            .map_err(|e| ManifestError::Semver(raw.version.clone(), e.to_string()))?;
        if clean_relative(&raw.entry).is_none() {
            return Err(ManifestError::Entry(raw.entry));
        }
        let mut capabilities = BTreeSet::new();
        for name in &raw.capabilities {
            capabilities.insert(
                Capability::parse(name).ok_or_else(|| ManifestError::Capability(name.clone()))?,
            );
        }
        Ok(Self {
            id: raw.id,
            name: raw.name,
            version,
            entry: raw.entry,
            capabilities: capabilities.into_iter().collect(),
            description: raw.description,
        })
    }
}

/// The syntax of an Identifier (`docs/spec/manifest.md`): two or more segments separated by `.`,
/// each starting with a lowercase letter `a` to `z` and holding only lowercase letters, digits
/// and `-`; at most 128 characters. ODK does not depend on `oma`, so the rule is read here too.
fn is_identifier(text: &str) -> bool {
    const LONGEST: usize = 128;
    if text.len() > LONGEST {
        return false;
    }
    let segments: Vec<&str> = text.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|segment| {
            let mut chars = segment.chars();
            chars.next().is_some_and(|c| c.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
plugin_manifest_version = 1
id = "com.example.notes.word-count"
name = "Word count"
version = "0.2.0"
description = "Shows a live word count"
entry = "main.luau"
capabilities = ["app.state", "ui"]
"#;

    #[test]
    fn the_specs_example_is_read() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!(m.id, "com.example.notes.word-count");
        assert_eq!(m.version, semver::Version::new(0, 2, 0));
        assert_eq!(m.capabilities, [Capability::AppState, Capability::Ui]);
        assert_eq!(m.description.as_deref(), Some("Shows a live word count"));
    }

    fn with(from: &str, to: &str) -> Result<Manifest, ManifestError> {
        assert!(GOOD.contains(from), "{from}");
        Manifest::parse(&GOOD.replace(from, to))
    }

    #[test]
    fn a_key_the_spec_does_not_list_is_an_error() {
        let text = format!("{GOOD}\nextra = 1\n");
        assert!(matches!(
            Manifest::parse(&text),
            Err(ManifestError::Parse(_))
        ));
    }

    #[test]
    fn a_missing_required_key_is_an_error() {
        let text = GOOD.replace("name = \"Word count\"\n", "");
        assert!(matches!(
            Manifest::parse(&text),
            Err(ManifestError::Parse(_))
        ));
    }

    #[test]
    fn only_version_one_is_read() {
        assert_eq!(
            with("plugin_manifest_version = 1", "plugin_manifest_version = 2"),
            Err(ManifestError::Version(2))
        );
        assert!(matches!(
            with(
                "plugin_manifest_version = 1",
                "plugin_manifest_version = \"1\""
            ),
            Err(ManifestError::Parse(_))
        ));
    }

    #[test]
    fn an_id_follows_the_identifier_syntax() {
        for bad in [
            "word-count",
            "Com.example.x",
            "com.Example.x",
            "com.example_x.y",
            "com..x",
            "com.1x.y",
            "com.example.",
        ] {
            assert!(
                matches!(
                    with("com.example.notes.word-count", bad),
                    Err(ManifestError::Id(_))
                ),
                "{bad}"
            );
        }
        let long = format!("com.{}", "a".repeat(125));
        assert!(matches!(
            with("com.example.notes.word-count", &long),
            Err(ManifestError::Id(_))
        ));
        let longest = format!("com.{}", "a".repeat(124));
        assert!(with("com.example.notes.word-count", &longest).is_ok());
    }

    #[test]
    fn a_name_is_text_without_control_characters() {
        assert_eq!(with("Word count", ""), Err(ManifestError::Name));
        assert_eq!(with("Word count", "Word\\ncount"), Err(ManifestError::Name));
    }

    #[test]
    fn a_version_is_semver() {
        assert!(matches!(
            with("0.2.0", "0.2"),
            Err(ManifestError::Semver(..))
        ));
        assert!(with("0.2.0", "1.0.0-rc.1+build.5").is_ok());
    }

    #[test]
    fn an_entry_stays_inside_the_plugin_directory() {
        for bad in [
            "../main.luau",
            "/etc/passwd",
            "a/../../b.luau",
            "",
            "./",
            "a//b.luau",
        ] {
            assert!(
                matches!(with("main.luau", bad), Err(ManifestError::Entry(_))),
                "{bad:?}"
            );
        }
        assert!(with("main.luau", "src/main.luau").is_ok());
    }

    #[test]
    fn only_the_four_capabilities_are_known_and_repeats_are_one() {
        assert_eq!(
            with("[\"app.state\", \"ui\"]", "[\"files\"]"),
            Err(ManifestError::Capability("files".into()))
        );
        let m = with(
            "[\"app.state\", \"ui\"]",
            "[\"storage\", \"ui\", \"storage\"]",
        )
        .unwrap();
        assert_eq!(m.capabilities, [Capability::Ui, Capability::Storage]);
        assert!(
            with("[\"app.state\", \"ui\"]", "[]")
                .unwrap()
                .capabilities
                .is_empty()
        );
    }
}
