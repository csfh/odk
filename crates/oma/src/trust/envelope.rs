//! A Bundle file checked against itself (ADR 0060, sections 4 and 9). Checking it against a
//! publisher is later work.
//!
//! The file checked here is a sealed copy (section 5). It holds when:
//! - the prefix is well formed and the file is exactly the prefix and the payload;
//! - the manifest is canonical;
//! - the manifest's signature verifies under the release key in the header;
//! - that key is the one the manifest names;
//! - both digests match.

use sha2::{Digest, Sha256};

use super::container::{PREFIX_LEN, Prefix};
use super::signature::{self, Context, Signature};
use super::{Error, Header, Sealed, SignedManifest};

/// A file that is signed by the release key in its own header, with its digests intact.
///
/// This is not trust. Nothing here says who that key belongs to: whether the pinned root's
/// current set authorizes it is for the pin and the key set, which only the consent and install
/// phases decide.
///
/// Only [`check`] makes one: the fields are private.
#[derive(Debug, Clone)]
pub struct SelfSigned {
    header: Header,
    manifest: SignedManifest,
    manifest_bytes: Vec<u8>,
    signature: Signature,
}

impl SelfSigned {
    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn manifest(&self) -> &SignedManifest {
        &self.manifest
    }

    /// The manifest's canonical bytes, exactly as signed.
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

/// Checks a sealed Bundle file against itself, cheapest check first: the payload is hashed only
/// once everything before it holds.
pub fn check(file: &Sealed) -> Result<SelfSigned, Error> {
    let len = file.len();
    if len < PREFIX_LEN as u64 {
        return Err(Error::Truncated { len });
    }
    let mut prefix = [0u8; PREFIX_LEN];
    file.read_exact_at(&mut prefix, 0)?;
    let prefix = Prefix::parse(&prefix, len)?;
    let manifest = SignedManifest::parse(&prefix.manifest)?;
    signature::verify(
        &prefix.header.release_key,
        Context::MANIFEST,
        &prefix.manifest,
        &prefix.signature,
    )?;
    let carried = prefix.header.release_key.key_id();
    if manifest.key_id != carried {
        return Err(Error::KeyMismatch {
            named: manifest.key_id,
            carried,
        });
    }
    if <[u8; 32]>::from(Sha256::digest(prefix.header_bytes)) != manifest.header_sha256 {
        return Err(Error::Digest { what: "header" });
    }
    if payload_sha256(file)? != manifest.payload_sha256 {
        return Err(Error::Digest { what: "payload" });
    }
    Ok(SelfSigned {
        header: prefix.header,
        manifest,
        manifest_bytes: prefix.manifest,
        signature: prefix.signature,
    })
}

/// SHA-256 of every byte from offset 1024 to the end.
fn payload_sha256(file: &Sealed) -> Result<[u8; 32], Error> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut at = PREFIX_LEN as u64;
    while at < file.len() {
        let n = usize::try_from(file.len() - at).map_or(buf.len(), |left| left.min(buf.len()));
        file.read_exact_at(&mut buf[..n], at)?;
        hasher.update(&buf[..n]);
        at += n as u64;
    }
    Ok(hasher.finalize().into())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::trust::SigningKey;
    use crate::trust::container::Header;
    use crate::trust::manifest::tests::sample;
    use std::path::Path;

    /// A release key for the tests' Bundles. A fixture, never a real key.
    pub(crate) fn release_key() -> SigningKey {
        SigningKey::from_seed(&[0x42; 32]).unwrap()
    }

    /// The bytes of a Bundle file: `payload` behind a prefix whose manifest is `edit`ed from the
    /// sample, then signed by `key`. The digests and key id are filled in before `edit` runs, so
    /// an edit can break them.
    pub(crate) fn bundle(
        payload: &[u8],
        key: &SigningKey,
        edit: impl FnOnce(&mut SignedManifest),
    ) -> Vec<u8> {
        let mut manifest = sample();
        manifest.key_id = key.public_key().key_id();
        manifest.payload_sha256 = Sha256::digest(payload).into();
        let mut header = Header {
            manifest_len: 0,
            payload_len: payload.len() as u64,
            release_key: key.public_key(),
        };
        // The header holds the manifest's length, and the manifest the header's digest, so the
        // length is settled first: it does not change with the digest's value.
        header.manifest_len = manifest.to_canonical().unwrap().len() as u32;
        manifest.header_sha256 = Sha256::digest(header.encode()).into();
        edit(&mut manifest);
        let bytes = manifest.to_canonical().unwrap();
        header.manifest_len = bytes.len() as u32;
        let signature = key.sign(Context::MANIFEST, &bytes);
        let mut file = Prefix::encode(&header, &bytes, &signature)
            .unwrap()
            .to_vec();
        file.extend_from_slice(payload);
        file
    }

    fn sealed(dir: &Path, bytes: &[u8]) -> Sealed {
        let path = dir.join("App.oma");
        std::fs::write(&path, bytes).unwrap();
        Sealed::memfd(&path).unwrap()
    }

    #[test]
    fn a_file_signed_by_the_key_in_its_header_with_its_digests_holds() {
        let dir = tempfile::tempdir().unwrap();
        let payload = vec![7u8; 200_000];
        let file = bundle(&payload, &release_key(), |_| {});
        let checked = check(&sealed(dir.path(), &file)).unwrap();
        assert_eq!(checked.header().release_key, release_key().public_key());
        assert_eq!(checked.manifest().app_id.as_str(), "dev.csfh.notes");
        assert_eq!(checked.header().payload_len, 200_000);
        assert_eq!(
            checked.manifest_bytes(),
            checked.manifest().to_canonical().unwrap()
        );
    }

    #[test]
    fn a_file_shorter_than_the_prefix_or_cut_short_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = bundle(b"payload", &release_key(), |_| {});
        assert!(matches!(
            check(&sealed(dir.path(), &file[..1023])),
            Err(Error::Truncated { len: 1023 })
        ));
        assert!(matches!(
            check(&sealed(dir.path(), &file[..file.len() - 1])),
            Err(Error::Size { .. })
        ));
        assert!(matches!(
            check(&sealed(dir.path(), b"")),
            Err(Error::Truncated { len: 0 })
        ));
    }

    #[test]
    fn a_changed_payload_is_a_digest_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = bundle(b"the payload", &release_key(), |_| {});
        *file.last_mut().unwrap() ^= 1;
        assert!(matches!(
            check(&sealed(dir.path(), &file)),
            Err(Error::Digest { what: "payload" })
        ));
    }

    #[test]
    fn a_manifest_that_does_not_name_the_headers_digest_or_key_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let header = bundle(b"x", &release_key(), |m| m.header_sha256 = [0; 32]);
        assert!(matches!(
            check(&sealed(dir.path(), &header)),
            Err(Error::Digest { what: "header" })
        ));
        let other = SigningKey::from_seed(&[0x43; 32])
            .unwrap()
            .public_key()
            .key_id();
        let key = bundle(b"x", &release_key(), |m| m.key_id = other);
        assert!(matches!(
            check(&sealed(dir.path(), &key)),
            Err(Error::KeyMismatch { .. })
        ));
    }

    #[test]
    fn a_signature_by_another_key_or_over_other_bytes_does_not_verify() {
        let dir = tempfile::tempdir().unwrap();
        // Signed by another key than the one the header carries.
        let mut file = bundle(b"x", &release_key(), |_| {});
        let other = SigningKey::from_seed(&[0x43; 32]).unwrap();
        file[32..64].copy_from_slice(other.public_key().as_bytes());
        assert!(matches!(
            check(&sealed(dir.path(), &file)),
            Err(Error::BadSignature { .. })
        ));

        // One byte of the signed manifest changed: still canonical, no longer signed.
        let file = bundle(b"x", &release_key(), |_| {});
        let at = file[64..1024]
            .windows(9)
            .position(|w| w == b"\"version\"")
            .unwrap()
            + 64
            + 10;
        let mut changed = file.clone();
        changed[at] = b'8';
        assert!(matches!(
            check(&sealed(dir.path(), &changed)),
            Err(Error::BadSignature { .. })
        ));

        // A key that is no point on the curve verifies nothing.
        let mut file = bundle(b"x", &release_key(), |_| {});
        file[32..64].copy_from_slice(&[0xff; 32]);
        assert!(check(&sealed(dir.path(), &file)).is_err());
    }

    #[test]
    fn a_header_that_says_another_algorithm_fails_closed_before_any_signature() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = bundle(b"x", &release_key(), |_| {});
        file[8] = 2;
        assert!(matches!(
            check(&sealed(dir.path(), &file)),
            Err(Error::HeaderField {
                field: "algorithm",
                ..
            })
        ));
    }
}
