//! odk's own releases (ADR 0086): the release root compiled into this binary, and signing a
//! release that `tools/package-release` made. Verifying one for `odk oma self-upgrade` takes the
//! root as an argument, so that tests give a fixture root and the binary only ever gives
//! [`release_root`].

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

use crate::trust::release::{self, Asset, Release};
use crate::trust::signature;
use crate::trust::{Context, KeyId, PublicKey, SigningKey, Timestamp};

/// The odk release root's public key, as 64 lowercase hex digits: `odk-oma sign-release` prints
/// the line to put here. `None` until the owner's key is in: this build then has no release root,
/// and `self-upgrade` refuses. No variable, feature or flag replaces it (ADR 0060, section 8).
const RELEASE_ROOT: Option<&str> = None;

/// The tarball's name beside `install.sh`.
pub const TARBALL: &str = "odk-x86_64-linux.tar.gz";

/// Where the running `odk-oma` sits inside the tarball.
const ODK_OMA: &str = "odk/lib/odk/odk-oma";

/// The release root compiled into this binary, if it has one.
pub fn release_root() -> Option<PublicKey> {
    RELEASE_ROOT
        .map(|hex| crate::trust::unhex::<32>(hex).expect("RELEASE_ROOT is 64 lowercase hex digits"))
        .map(PublicKey::from_bytes)
}

/// The line that compiles `key` in as the release root, for `RELEASE_ROOT` above.
pub fn constant_line(key: &PublicKey) -> String {
    format!(
        "const RELEASE_ROOT: Option<&str> = Some(\"{}\");",
        crate::trust::hex(key.as_bytes())
    )
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "this build has no release root, so it can neither sign nor accept a release (ADR 0086)"
    )]
    NoReleaseRoot,
    #[error(
        "the key on standard input is root {seed}, and this build trusts root {compiled}: a release must only ship a binary that trusts the key signing it"
    )]
    NotThisRoot { seed: KeyId, compiled: KeyId },
    #[error(
        "{}: the release's install.sh names no `{line}` (make the release with tools/package-release)", .path.display()
    )]
    NotPackaged { path: PathBuf, line: &'static str },
    #[error("{}: it is not the tarball install.sh names", .0.display())]
    Tarball(PathBuf),
    #[error(
        "the odk-oma in the tarball is not the one running; sign with the odk-oma being released"
    )]
    NotThisBinary,
    #[error("cannot read the tarball's odk-oma with tar: {0}")]
    Tar(String),
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Trust(#[from] crate::trust::Error),
    #[error("the clock is out of range")]
    Clock,
}

/// What `install.sh` says of its release, as `tools/package-release` wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub version: u64,
    pub tarball: Asset,
}

impl Named {
    /// The `release_version=`, `tarball_sha256=` and `tarball_length=` lines of `script`.
    pub fn read(path: &Path, script: &str) -> Result<Self, Error> {
        let value = |line: &'static str| {
            script
                .lines()
                .find_map(|l| l.strip_prefix(line)?.strip_prefix('='))
                .filter(|v| !v.is_empty())
                .ok_or_else(|| Error::NotPackaged {
                    path: path.to_owned(),
                    line,
                })
        };
        let bad = |line| Error::NotPackaged {
            path: path.to_owned(),
            line,
        };
        let version = value("release_version")?
            .parse()
            .map_err(|_| bad("release_version"))?;
        let sha256 = crate::trust::unhex::<32>(value("tarball_sha256")?)
            .ok_or_else(|| bad("tarball_sha256"))?;
        let length = value("tarball_length")?
            .parse()
            .map_err(|_| bad("tarball_length"))?;
        Ok(Self {
            version,
            tarball: Asset { sha256, length },
        })
    }
}

/// Signs the release in `dir`: `install.sh` and the tarball beside it, as `tools/package-release`
/// left them. `root` is the release root's seed, `compiled` the root the shipped binary trusts
/// and `running` that binary. Writes `release` and `release.sig` (Ed25519ctx) and
/// `install.sh.sig` (plain Ed25519, for OpenSSL), each checked once written. Nothing is written
/// unless the tarball is the one the script names, its `odk-oma` is `running`, and `compiled` is
/// `root`'s public key.
pub fn sign(
    dir: &Path,
    root: &SigningKey,
    compiled: Option<&PublicKey>,
    running: &Path,
    now: i64,
) -> Result<Release, Error> {
    let compiled = compiled.ok_or(Error::NoReleaseRoot)?;
    let public = root.public_key();
    if *compiled != public {
        return Err(Error::NotThisRoot {
            seed: public.key_id(),
            compiled: compiled.key_id(),
        });
    }
    let script_path = dir.join("install.sh");
    let script = read(&script_path)?;
    let text = String::from_utf8_lossy(&script);
    let named = Named::read(&script_path, &text)?;
    let tarball_path = dir.join(TARBALL);
    let tarball = read(&tarball_path)?;
    if asset(&tarball) != named.tarball {
        return Err(Error::Tarball(tarball_path));
    }
    let shipped = Command::new("tar")
        .arg("-xzOf")
        .arg(&tarball_path)
        .arg(ODK_OMA)
        .output()
        .map_err(|e| Error::Tar(e.to_string()))?;
    if !shipped.status.success() {
        return Err(Error::Tar(
            String::from_utf8_lossy(&shipped.stderr).trim().to_owned(),
        ));
    }
    if Sha256::digest(&shipped.stdout) != Sha256::digest(read(running)?) {
        return Err(Error::NotThisBinary);
    }

    let release = Release {
        root_key_id: public.key_id(),
        version: named.version,
        issued_at: Timestamp::from_unix_seconds(now).ok_or(Error::Clock)?,
        tarball: TARBALL.to_owned(),
        tarball_asset: named.tarball,
        script: asset(&script),
    };
    let body = release.to_canonical()?;
    let release_sig = root.sign(Context::RELEASE, &body);
    release::verify(&body, &release_sig.0, &public)?;
    let script_sig = root.sign_plain(&script);
    if !signature::verify_plain(&public, &script, &script_sig) {
        return Err(crate::trust::Error::BadSignature {
            document: "install.sh",
        }
        .into());
    }
    write(&dir.join("release"), &body)?;
    write(&dir.join("release.sig"), &release_sig.0)?;
    write(&dir.join("install.sh.sig"), &script_sig.0)?;
    Ok(release)
}

/// `key` as a PEM public key, which OpenSSL 3 `pkeyutl` takes to verify `install.sh.sig`.
pub fn pem(key: &PublicKey) -> String {
    // The DER of an Ed25519 SubjectPublicKeyInfo, RFC 8410, before its 32 bytes.
    const PREFIX: [u8; 12] = [
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    let mut der = PREFIX.to_vec();
    der.extend_from_slice(key.as_bytes());
    format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
        STANDARD.encode(der)
    )
}

fn asset(bytes: &[u8]) -> Asset {
    Asset {
        sha256: Sha256::digest(bytes).into(),
        length: bytes.len() as u64,
    }
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    fs::read(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    fs::write(path, bytes).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_build_has_no_release_root_until_the_owners_key_is_in() {
        // When the owner's key is in, this says so: `release_root` also panics on a constant that
        // is not 64 lowercase hex digits, here rather than in a release.
        assert!(release_root().is_none());
        assert!(RELEASE_ROOT.is_none_or(|hex| crate::trust::unhex::<32>(hex).is_some()));
    }

    #[test]
    fn the_printed_line_compiles_the_key_in() {
        let key = SigningKey::from_seed(&[0x63; 32]).unwrap().public_key();
        let line = constant_line(&key);
        let hex = line
            .strip_prefix("const RELEASE_ROOT: Option<&str> = Some(\"")
            .and_then(|rest| rest.strip_suffix("\");"))
            .unwrap();
        assert_eq!(
            crate::trust::unhex::<32>(hex).map(PublicKey::from_bytes),
            Some(key)
        );
    }

    #[test]
    fn the_script_names_its_release_only_when_every_line_is_filled() {
        let path = Path::new("install.sh");
        let sha = "ab".repeat(32);
        let packaged = format!("x=1\nrelease_version=17\ntarball_sha256={sha}\ntarball_length=9\n");
        assert_eq!(
            Named::read(path, &packaged).unwrap(),
            Named {
                version: 17,
                tarball: Asset {
                    sha256: [0xab; 32],
                    length: 9
                }
            }
        );
        let repository = "release_version=\ntarball_sha256=\ntarball_length=\n";
        assert!(matches!(
            Named::read(path, repository),
            Err(Error::NotPackaged {
                line: "release_version",
                ..
            })
        ));
        let short = packaged.replace(&sha, "ab");
        assert!(matches!(
            Named::read(path, &short),
            Err(Error::NotPackaged {
                line: "tarball_sha256",
                ..
            })
        ));
    }
}
