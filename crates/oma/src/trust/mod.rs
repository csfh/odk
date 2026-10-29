//! The bytes ADR 0060 trusts: the 64-byte header and the prefix of a `.oma` file, the signed
//! manifest, the root-signed `oma-keys` set, key ids, Ed25519ctx (ADR 0061), the sealed copy a
//! file is checked in, and the high-water rule.
//!
//! Nothing here decides that a publisher is trusted. [`check`] says a file is signed by the
//! release key in its own header and that its digests hold; whether that key is the pinned
//! root's is for the pin and the key set, which the consent and install phases add
//! (`docs/oma-trust-plan.md`). The `binfmt_misc` entry calls it and still refuses the file;
//! `odk oma install <file>` calls it at first contact with a root (`flow::install_file`).

pub mod canonical;
pub mod container;
pub mod envelope;
pub mod highwater;
pub mod id;
pub mod keys;
pub mod manifest;
pub mod sealed;
pub mod signature;
pub mod time;

pub use container::{Header, Prefix};
pub use envelope::{Checked, SelfSigned, check, stamp};
pub use highwater::{Mark, Verdict};
pub use id::{Fingerprint, KeyId, PublicKey};
pub use keys::{OmaKeys, ReleaseKey, SetChange, Signer, VerifiedKeys};
pub use manifest::{Publisher, SignedManifest};
pub use sealed::Sealed;
pub use signature::{Context, Signature, SigningKey};
pub use time::Timestamp;

/// Why a file or a document was not accepted. Every variant fails closed (ADR 0060, section 9).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("does not start with OMA1, so it is not a Bundle file")]
    NotOma,
    #[error("is {len} bytes, shorter than the 1024-byte prefix")]
    Truncated { len: u64 },
    #[error("the header's {field} is {value}, and only {expected} is accepted")]
    HeaderField {
        field: &'static str,
        value: u64,
        expected: u64,
    },
    #[error(
        "the signed manifest is {len} bytes; with its signature it must fit in the prefix (at most 896)"
    )]
    ManifestTooLong { len: u32 },
    #[error("byte {offset} of the prefix is not zero")]
    Padding { offset: usize },
    #[error("the file is {actual} bytes, and its header says {expected}")]
    Size { expected: u128, actual: u64 },
    #[error("not a canonical document: {expected} expected at byte {at}")]
    Canonical { at: usize, expected: &'static str },
    #[error(
        "not a canonical document: a control character at byte {at}, whose spelling ADR 0060 does not settle yet (docs/oma-trust-plan.md, question 5)"
    )]
    ControlCharacter { at: usize },
    #[error("not a canonical document: it is not UTF-8")]
    NotUtf8,
    #[error("{field} {reason}")]
    Field { field: &'static str, reason: String },
    #[error("the signature of the {document} does not verify")]
    BadSignature { document: &'static str },
    #[error("a signature is {len} bytes; it must be 64")]
    SignatureLength { len: usize },
    #[error("oma-keys is {len} bytes; at most 16384 are read")]
    KeysTooLarge { len: usize },
    #[error("oma-keys names root {found}, and the pinned root is {pinned}")]
    DifferentRoot { found: KeyId, pinned: KeyId },
    #[error("the manifest names release key {named}, and the header carries {carried}")]
    KeyMismatch { named: KeyId, carried: KeyId },
    #[error("the {what} digest does not match the manifest")]
    Digest { what: &'static str },
    #[error("is a symbolic link; a Bundle file is opened without following one")]
    Symlink,
    #[error("is a {kind}, not a regular file")]
    NotRegular { kind: &'static str },
    #[error("the sealed copy does not carry every seal (it has {seals:#x})")]
    Seals { seals: u32 },
    #[error("the store {reason}")]
    Store { reason: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<rustix::io::Errno> for Error {
    fn from(e: rustix::io::Errno) -> Self {
        Error::Io(e.into())
    }
}

/// The lowercase hex of `bytes`, as ADR 0060 spells digests and key ids.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0xf)] as char);
    }
    out
}

/// `N` bytes from exactly `2 * N` lowercase hex digits, and nothing else.
pub(crate) fn unhex<const N: usize>(s: &str) -> Option<[u8; N]> {
    let digits = s.as_bytes();
    if digits.len() != 2 * N {
        return None;
    }
    let value = |d: u8| match d {
        b'0'..=b'9' => Some(d - b'0'),
        b'a'..=b'f' => Some(d - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; N];
    for (i, pair) in digits.as_chunks::<2>().0.iter().enumerate() {
        out[i] = value(pair[0])? << 4 | value(pair[1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lowercase_and_unhex_takes_nothing_else() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(unhex::<3>("00abff"), Some([0x00, 0xab, 0xff]));
        for bad in ["00ABFF", "00abf", "00abfff", "00abfg", "+0abff"] {
            assert_eq!(unhex::<3>(bad), None, "{bad}");
        }
    }
}
