//! Ed25519ctx (RFC 8032), with `crrl` as ADR 0061 decides: `sign_ctx` and `verify_ctx`, and no
//! other mode. Each kind of document has its own context string (ADR 0060, section 3), so a
//! signature over one never verifies as another.

use std::fmt;

use super::{Error, PublicKey};

/// The context string of one kind of signed document. Only the constants below exist, so a
/// caller cannot verify under a context ADR 0060 does not name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Context {
    bytes: &'static [u8],
    document: &'static str,
}

impl Context {
    /// The signed manifest in a Bundle's prefix.
    pub const MANIFEST: Context = Context::new(b"odk-oma manifest v1", "signed manifest");
    /// `oma-keys`, the root's set of release keys.
    pub const KEYS: Context = Context::new(b"odk-oma keys v1", "oma-keys");
    /// A rotation to a successor root. Its body is not parsed yet (`docs/oma-trust-plan.md`, question 3).
    pub const ROTATION: Context = Context::new(b"odk-oma rotation v1", "rotation");
    /// A revocation of release keys. Its body is not parsed yet (question 2).
    pub const REVOCATION: Context = Context::new(b"odk-oma revocation v1", "revocation");
    /// An algorithm policy. Its body is not specified yet (question 6).
    pub const ALGORITHM: Context = Context::new(b"odk-oma algorithm v1", "algorithm policy");
    /// An odk release, signed by the release root (ADR 0086).
    pub const RELEASE: Context = Context::new(b"odk-oma release v1", "release");

    const fn new(bytes: &'static [u8], document: &'static str) -> Self {
        // RFC 8032: a context is at most 255 bytes, and these are never empty.
        assert!(!bytes.is_empty() && bytes.len() <= 255);
        Self { bytes, document }
    }

    pub fn as_bytes(&self) -> &'static [u8] {
        self.bytes
    }

    /// What the context signs, for an error message.
    pub fn document(&self) -> &'static str {
        self.document
    }

    /// Any context, for the RFC's own vectors.
    #[cfg(test)]
    pub(crate) const fn for_test(bytes: &'static [u8]) -> Self {
        Self::new(bytes, "test vector")
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Context({:?})", String::from_utf8_lossy(self.bytes))
    }
}

/// A raw 64-byte Ed25519 signature.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl Signature {
    /// Exactly 64 bytes, as `oma-keys.sig` and the prefix carry it.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        <[u8; 64]>::try_from(bytes)
            .map(Self)
            .map_err(|_| Error::SignatureLength { len: bytes.len() })
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature({})", super::hex(&self.0))
    }
}

/// Whether `signature` is `key`'s Ed25519ctx signature over exactly `message` in `context`.
pub fn verify(
    key: &PublicKey,
    context: Context,
    message: &[u8],
    signature: &Signature,
) -> Result<(), Error> {
    let document = context.document;
    // A key that is not a point on the curve signs nothing.
    let key =
        crrl::ed25519::PublicKey::decode(key.as_bytes()).ok_or(Error::BadSignature { document })?;
    if key.verify_ctx(&signature.0, context.bytes, message) {
        Ok(())
    } else {
        Err(Error::BadSignature { document })
    }
}

/// Plain Ed25519 over `message`, as [`SigningKey::sign_plain`] makes it.
pub fn verify_plain(key: &PublicKey, message: &[u8], signature: &Signature) -> bool {
    crrl::ed25519::PublicKey::decode(key.as_bytes())
        .is_some_and(|key| key.verify_raw(&signature.0, message))
}

/// A release key or a root, from its 32-byte seed. It signs for `odk oma bundle --sign` and for
/// the tests' fixtures; `odk-oma` itself only verifies. Its `Debug` shows the public key, never
/// the seed.
#[derive(Clone)]
pub struct SigningKey(crrl::ed25519::PrivateKey);

impl SigningKey {
    /// `None` unless `seed` is exactly 32 bytes. `crrl`'s `from_seed` would panic instead
    /// (ADR 0061).
    pub fn from_seed(seed: &[u8]) -> Option<Self> {
        crrl::ed25519::PrivateKey::decode(seed).map(Self)
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_bytes(self.0.public_key.encode())
    }

    pub fn sign(&self, context: Context, message: &[u8]) -> Signature {
        Signature(self.0.sign_ctx(context.bytes, message))
    }

    /// Plain Ed25519, with no context: what OpenSSL 3 `pkeyutl` verifies, for `install.sh`
    /// (ADR 0060, section 8). Nothing `odk-oma` reads is signed this way.
    pub fn sign_plain(&self, message: &[u8]) -> Signature {
        Signature(self.0.sign_raw(message))
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SigningKey({:?})", self.public_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::unhex;
    use sha2::{Digest, Sha512};

    /// The Ed25519ctx test vectors of RFC 8032, section 7.2, as published at
    /// <https://www.rfc-editor.org/rfc/rfc8032.txt>: secret key, public key, message, context,
    /// signature.
    const RFC_8032_7_2: [(&str, &str, &str, &[u8], &str); 4] = [
        (
            "0305334e381af78f141cb666f6199f57bc3495335a256a95bd2a55bf546663f6",
            "dfc9425e4f968f7f0c29f0259cf5f9aed6851c2bb4ad8bfb860cfee0ab248292",
            "f726936d19c800494e3fdaff20b276a8",
            b"foo",
            "55a4cc2f70a54e04288c5f4cd1e45a7bb520b36292911876cada7323198dd87a8b36950b95130022907a7fb7c4e9b2d5f6cca685a587b4b21f4b888e4e7edb0d",
        ),
        (
            "0305334e381af78f141cb666f6199f57bc3495335a256a95bd2a55bf546663f6",
            "dfc9425e4f968f7f0c29f0259cf5f9aed6851c2bb4ad8bfb860cfee0ab248292",
            "f726936d19c800494e3fdaff20b276a8",
            b"bar",
            "fc60d5872fc46b3aa69f8b5b4351d5808f92bcc044606db097abab6dbcb1aee3216c48e8b3b66431b5b186d1d28f8ee15a5ca2df6668346291c2043d4eb3e90d",
        ),
        (
            "0305334e381af78f141cb666f6199f57bc3495335a256a95bd2a55bf546663f6",
            "dfc9425e4f968f7f0c29f0259cf5f9aed6851c2bb4ad8bfb860cfee0ab248292",
            "508e9e6882b979fea900f62adceaca35",
            b"foo",
            "8b70c1cc8310e1de20ac53ce28ae6e7207f33c3295e03bb5c0732a1d20dc64908922a8b052cf99b7c4fe107a5abb5b2c4085ae75890d02df26269d8945f84b0b",
        ),
        (
            "ab9c2853ce297ddab85c993b3ae14bcad39b2c682beabc27d6d4eb20711d6560",
            "0f1d1274943b91415889152e893d80e93275a1fc0b65fd71b4b0dda10ad7d772",
            "f726936d19c800494e3fdaff20b276a8",
            b"foo",
            "21655b5f1aa965996b3f97b3c849eafba922a0a62992f73b3d1b73106a84ad85e9b86a7b6005ea868337ff2d20a7f5fbd4cd10b0be49a68da2b2e0dc0ad8960f",
        ),
    ];

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn the_rfc_8032_ed25519ctx_vectors_sign_and_verify() {
        for (secret, public, message, context, signature) in RFC_8032_7_2 {
            let key = SigningKey::from_seed(&bytes(secret)).unwrap();
            assert_eq!(key.public_key().as_bytes(), &unhex::<32>(public).unwrap());
            let context = Context::for_test(context);
            let signed = key.sign(context, &bytes(message));
            assert_eq!(signed.0.to_vec(), bytes(signature), "{context:?}");
            verify(&key.public_key(), context, &bytes(message), &signed).unwrap();
        }
    }

    #[test]
    fn plain_ed25519_ed25519ph_and_another_context_do_not_verify() {
        let (secret, _, message, context, _) = RFC_8032_7_2[0];
        let raw = crrl::ed25519::PrivateKey::decode(&bytes(secret)).unwrap();
        let key = SigningKey(raw).public_key();
        let message = bytes(message);
        let context = Context::for_test(context);
        let plain = Signature(raw.sign_raw(&message));
        let prehashed = Signature(raw.sign_ph(context.as_bytes(), &Sha512::digest(&message)));
        for (mode, signature) in [("plain", plain), ("ph", prehashed)] {
            assert!(
                verify(&key, context, &message, &signature).is_err(),
                "{mode} verified as ctx"
            );
        }

        let manifest = SigningKey(raw).sign(Context::MANIFEST, b"{}");
        verify(&key, Context::MANIFEST, b"{}", &manifest).unwrap();
        for other in [
            Context::KEYS,
            Context::ROTATION,
            Context::REVOCATION,
            Context::ALGORITHM,
        ] {
            assert!(verify(&key, other, b"{}", &manifest).is_err(), "{other:?}");
        }
    }

    #[test]
    fn a_signature_over_other_bytes_or_by_another_key_does_not_verify() {
        let a = SigningKey::from_seed(&[1; 32]).unwrap();
        let b = SigningKey::from_seed(&[2; 32]).unwrap();
        let signed = a.sign(Context::MANIFEST, b"{\"a\":1}");
        assert!(verify(&a.public_key(), Context::MANIFEST, b"{\"a\":2}", &signed).is_err());
        assert!(verify(&b.public_key(), Context::MANIFEST, b"{\"a\":1}", &signed).is_err());
        // 32 bytes that are not a point on the curve are no key.
        let not_a_point = PublicKey::from_bytes([0xff; 32]);
        assert!(verify(&not_a_point, Context::MANIFEST, b"{\"a\":1}", &signed).is_err());
    }

    #[test]
    fn a_seed_is_exactly_32_bytes_and_debug_never_shows_it() {
        assert!(SigningKey::from_seed(&[7; 31]).is_none());
        assert!(SigningKey::from_seed(&[7; 33]).is_none());
        let key = SigningKey::from_seed(&[7; 32]).unwrap();
        let shown = format!("{key:?}");
        assert!(!shown.contains(&crate::trust::hex(&[7; 32])), "{shown}");
        assert!(shown.contains(&crate::trust::hex(key.public_key().as_bytes())));
    }

    #[test]
    fn a_signature_is_exactly_64_bytes() {
        assert!(Signature::from_slice(&[0; 64]).is_ok());
        for len in [0, 63, 65] {
            assert!(matches!(
                Signature::from_slice(&vec![0; len]),
                Err(Error::SignatureLength { len: l }) if l == len
            ));
        }
    }
}
