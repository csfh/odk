use std::path::PathBuf;

/// Why a Manifest or Bundle was rejected.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid manifest.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported manifest_version {0}, expected 1")]
    ManifestVersion(i64),
    #[error("invalid identifier {0:?}: {1}")]
    Identifier(String, &'static str),
    #[error("invalid name: {0}")]
    Name(&'static str),
    #[error("invalid {0}: {1}")]
    Field(&'static str, &'static str),
    #[error("invalid version {0:?}: {1}")]
    Version(String, semver::Error),
    #[error("invalid {field} path {path:?}: {reason}")]
    Path {
        field: &'static str,
        path: String,
        reason: &'static str,
    },
    #[error("invalid permissions: {0}")]
    Permissions(String),
}
