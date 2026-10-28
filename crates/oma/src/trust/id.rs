//! A key's identity (ADR 0060, section 2). The fingerprint is SHA-256 of the 32 raw bytes of the
//! public key, and the key id is the fingerprint's first 8 bytes. Both are computed from the key,
//! never taken from a document: a document's key id is checked against the key it names.

use std::fmt;
use std::str::FromStr;

use sha2::{Digest, Sha256};

use super::{hex, unhex};

/// An Ed25519 public key: the 32 bytes the documents and the header carry.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint(Sha256::digest(self.0).into())
    }

    pub fn key_id(&self) -> KeyId {
        self.fingerprint().key_id()
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", hex(&self.0))
    }
}

/// SHA-256 of a public key, shown as 64 lowercase hex digits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn key_id(&self) -> KeyId {
        let mut id = [0u8; 8];
        id.copy_from_slice(&self.0[..8]);
        KeyId(id)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex(&self.0))
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

/// The first 8 bytes of a fingerprint, shown as 16 lowercase hex digits. Paths and documents
/// name keys by it (ADR 0060, section 2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyId([u8; 8]);

impl KeyId {
    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex(&self.0))
    }
}

impl fmt::Debug for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyId({self})")
    }
}

/// Exactly 16 lowercase hex digits.
impl FromStr for KeyId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        unhex::<8>(s).map(KeyId).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_id_is_the_first_eight_bytes_of_the_sha256_of_the_key() {
        // SHA-256 of 32 zero bytes, computed independently: `head -c 32 /dev/zero | sha256sum`.
        let key = PublicKey::from_bytes([0; 32]);
        assert_eq!(
            key.fingerprint().to_string(),
            "66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925"
        );
        assert_eq!(key.key_id().to_string(), "66687aadf862bd77");
    }

    #[test]
    fn a_key_id_is_read_only_from_sixteen_lowercase_hex_digits() {
        assert_eq!(
            "66687aadf862bd77".parse::<KeyId>().unwrap().to_string(),
            "66687aadf862bd77"
        );
        for bad in [
            "66687AADF862BD77",
            "66687aadf862bd7",
            "66687aadf862bd770",
            "",
        ] {
            assert!(bad.parse::<KeyId>().is_err(), "{bad}");
        }
    }
}
