//! An odk release (ADR 0086): the document the release root signs, which names the tarball and
//! the `install.sh` of one release by digest and length, and its monotonic version. It is
//! canonical JSON, as `oma-keys` is, and signed with Ed25519ctx in [`Context::RELEASE`].

use super::canonical::{Reader, Writer};
use super::manifest::{ALGORITHM, key_id_field, timestamp_field};
use super::signature::{self, Context, Signature};
use super::{Error, KeyId, PublicKey, Timestamp, hex, unhex};

/// The largest document that is read (16 KiB), as for `oma-keys`.
pub const MAX_BODY: usize = 16 * 1024;

/// One file of a release, by what it must be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub sha256: [u8; 32],
    pub length: u64,
}

/// A release, as its document says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub root_key_id: KeyId,
    /// The Unix seconds when it was packaged, which only goes up.
    pub version: u64,
    pub issued_at: Timestamp,
    /// The tarball's file name beside the document, such as `odk-x86_64-linux.tar.gz`.
    pub tarball: String,
    pub tarball_asset: Asset,
    /// `install.sh`, beside it.
    pub script: Asset,
}

impl Release {
    /// The canonical bytes, and only those, with every field's rule checked. The keys come in
    /// this order: `algorithm`, `root_key_id`, `version`, `issued_at`, `tarball`,
    /// `tarball_sha256`, `tarball_length`, `script_sha256`, `script_length`.
    pub fn parse(body: &[u8]) -> Result<Self, Error> {
        if body.len() > MAX_BODY {
            return Err(Error::ReleaseTooLarge { len: body.len() });
        }
        let mut r = Reader::new(body)?;
        r.begin_object()?;
        r.key("algorithm", true)?;
        let algorithm = r.string()?;
        if algorithm != ALGORITHM {
            return Err(field(
                "algorithm",
                format!("is {algorithm:?}; only \"ed25519\" is known"),
            ));
        }
        r.key("root_key_id", false)?;
        let root_key_id = key_id_field("root_key_id", &r.string()?)?;
        r.key("version", false)?;
        let version = r.u64()?;
        r.key("issued_at", false)?;
        let issued_at = timestamp_field("issued_at", &r.string()?)?;
        r.key("tarball", false)?;
        let tarball = r.string()?;
        if !plain_name(&tarball) {
            return Err(field(
                "tarball",
                format!("{tarball:?} is not a plain file name"),
            ));
        }
        r.key("tarball_sha256", false)?;
        let tarball_sha256 = digest("tarball_sha256", &r.string()?)?;
        r.key("tarball_length", false)?;
        let tarball_length = length("tarball_length", r.u64()?)?;
        r.key("script_sha256", false)?;
        let script_sha256 = digest("script_sha256", &r.string()?)?;
        r.key("script_length", false)?;
        let script_length = length("script_length", r.u64()?)?;
        r.end_object()?;
        r.finish()?;
        Ok(Self {
            root_key_id,
            version,
            issued_at,
            tarball,
            tarball_asset: Asset {
                sha256: tarball_sha256,
                length: tarball_length,
            },
            script: Asset {
                sha256: script_sha256,
                length: script_length,
            },
        })
    }

    /// The canonical bytes of this release: what the root signs.
    pub fn to_canonical(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::default();
        w.begin_object();
        w.key("algorithm", true);
        w.string(ALGORITHM)?;
        w.key("root_key_id", false);
        w.string(&self.root_key_id.to_string())?;
        w.key("version", false);
        w.u64(self.version);
        w.key("issued_at", false);
        w.string(self.issued_at.as_str())?;
        w.key("tarball", false);
        w.string(&self.tarball)?;
        w.key("tarball_sha256", false);
        w.string(&hex(&self.tarball_asset.sha256))?;
        w.key("tarball_length", false);
        w.u64(self.tarball_asset.length);
        w.key("script_sha256", false);
        w.string(&hex(&self.script.sha256))?;
        w.key("script_length", false);
        w.u64(self.script.length);
        w.end_object();
        Ok(w.finish())
    }
}

/// A document and its raw 64-byte signature, checked under `root`, the release root this odk
/// trusts. A document that names another root is refused before its signature is checked.
pub fn verify(body: &[u8], signature: &[u8], root: &PublicKey) -> Result<Release, Error> {
    let release = Release::parse(body)?;
    if release.root_key_id != root.key_id() {
        return Err(Error::OtherReleaseRoot {
            found: release.root_key_id,
            trusted: root.key_id(),
        });
    }
    let signature = Signature::from_slice(signature)?;
    signature::verify(root, Context::RELEASE, body, &signature)?;
    Ok(release)
}

fn field(field: &'static str, reason: String) -> Error {
    Error::Field { field, reason }
}

fn digest(name: &'static str, s: &str) -> Result<[u8; 32], Error> {
    unhex::<32>(s).ok_or_else(|| field(name, format!("{s:?} is not 64 lowercase hex digits")))
}

fn length(name: &'static str, n: u64) -> Result<u64, Error> {
    if n == 0 {
        return Err(field(name, "is 0".to_owned()));
    }
    Ok(n)
}

/// Letters, digits, `.`, `_` and `-`, not starting with `.`: a name beside the document, never a
/// path.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::SigningKey;

    // Fixture keys, never real ones.
    fn root() -> SigningKey {
        SigningKey::from_seed(&[0x51; 32]).unwrap()
    }

    fn release() -> Release {
        Release {
            root_key_id: root().public_key().key_id(),
            version: 1_800_000_000,
            issued_at: Timestamp::from_unix_seconds(1_800_000_000).unwrap(),
            tarball: "odk-x86_64-linux.tar.gz".to_owned(),
            tarball_asset: Asset {
                sha256: [0xab; 32],
                length: 13_282_572,
            },
            script: Asset {
                sha256: [0xcd; 32],
                length: 9_000,
            },
        }
    }

    #[test]
    fn a_release_is_canonical_and_verifies_under_its_root_only() {
        let body = release().to_canonical().unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(
            text.starts_with(r#"{"algorithm":"ed25519","root_key_id":""#),
            "{text}"
        );
        assert!(text.ends_with(r#""script_length":9000}"#), "{text}");
        assert_eq!(Release::parse(&body).unwrap(), release());

        let signature = root().sign(Context::RELEASE, &body).0;
        let key = root().public_key();
        assert_eq!(verify(&body, &signature, &key).unwrap(), release());

        // Signed in another context, it is not a release.
        let keys = root().sign(Context::KEYS, &body).0;
        assert!(matches!(
            verify(&body, &keys, &key),
            Err(Error::BadSignature { .. })
        ));
        // Another root is refused by name, whatever signed it.
        let other = SigningKey::from_seed(&[0x52; 32]).unwrap();
        let signature = other.sign(Context::RELEASE, &body).0;
        assert!(matches!(
            verify(&body, &signature, &other.public_key()),
            Err(Error::OtherReleaseRoot { .. })
        ));
        // A changed byte does not verify.
        let mut changed = body.clone();
        let at = text.find("9000").unwrap();
        changed[at] = b'8';
        let signature = root().sign(Context::RELEASE, &body).0;
        assert!(matches!(
            verify(&changed, &signature, &key),
            Err(Error::BadSignature { .. })
        ));
    }

    #[test]
    fn a_path_a_zero_length_and_another_order_are_refused() {
        let mut bad = release();
        bad.tarball = "../odk.tar.gz".to_owned();
        let body = bad.to_canonical().unwrap();
        assert!(matches!(
            Release::parse(&body),
            Err(Error::Field {
                field: "tarball",
                ..
            })
        ));
        let mut bad = release();
        bad.script.length = 0;
        let body = bad.to_canonical().unwrap();
        assert!(matches!(
            Release::parse(&body),
            Err(Error::Field {
                field: "script_length",
                ..
            })
        ));
        let text = String::from_utf8(release().to_canonical().unwrap()).unwrap();
        let swapped = text.replacen(r#""version""#, r#""versionx""#, 1);
        assert!(Release::parse(swapped.as_bytes()).is_err());
        assert!(matches!(
            Release::parse(&vec![b' '; MAX_BODY + 1]),
            Err(Error::ReleaseTooLarge { .. })
        ));
    }

    #[test]
    fn a_plain_signature_is_what_openssl_checks_and_not_a_release() {
        let message = b"#!/bin/sh\n";
        let signature = root().sign_plain(message);
        assert!(signature::verify_plain(
            &root().public_key(),
            message,
            &signature
        ));
        assert!(!signature::verify_plain(
            &root().public_key(),
            b"#!/bin/bash\n",
            &signature
        ));
        assert!(
            signature::verify(&root().public_key(), Context::RELEASE, message, &signature).is_err()
        );
    }
}
