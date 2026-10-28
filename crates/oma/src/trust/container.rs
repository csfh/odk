//! The prefix of a `.oma` file (ADR 0060, section 4): a 64-byte little-endian header, the signed
//! manifest at offset 64, its 64-byte signature right after it, zeros up to offset 1024, and the
//! EROFS image from 1024 to the end of the file.

use super::{Error, PublicKey, Signature};

pub const MAGIC: [u8; 4] = *b"OMA1";
pub const HEADER_LEN: usize = 64;
pub const SIGNATURE_LEN: usize = 64;
/// Where the image begins, and the size of everything before it.
pub const PREFIX_LEN: usize = 1024;
/// The longest manifest that fits in the prefix with its signature: `64 + M + 64 <= 1024`.
pub const MAX_MANIFEST_LEN: usize = PREFIX_LEN - HEADER_LEN - SIGNATURE_LEN;

const HEADER_VERSION: u16 = 1;
const ALGORITHM_ED25519: u16 = 1;

/// The fixed header: what it says about the rest of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub manifest_len: u32,
    pub payload_len: u64,
    pub release_key: PublicKey,
}

impl Header {
    /// The header at the start of a file. Every fixed field must hold its one value.
    pub fn parse(bytes: &[u8; HEADER_LEN]) -> Result<Self, Error> {
        if bytes[0..4] != MAGIC {
            return Err(Error::NotOma);
        }
        let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let fixed = [
            ("version", u64::from(u16_at(4)), u64::from(HEADER_VERSION)),
            ("header length", u64::from(u16_at(6)), HEADER_LEN as u64),
            (
                "algorithm",
                u64::from(u16_at(8)),
                u64::from(ALGORITHM_ED25519),
            ),
            ("reserved field at 10", u64::from(u16_at(10)), 0),
            (
                "signature length",
                u64::from(u32_at(16)),
                SIGNATURE_LEN as u64,
            ),
            ("reserved field at 28", u64::from(u32_at(28)), 0),
        ];
        for (field, value, expected) in fixed {
            if value != expected {
                return Err(Error::HeaderField {
                    field,
                    value,
                    expected,
                });
            }
        }
        let manifest_len = u32_at(12);
        if manifest_len as usize > MAX_MANIFEST_LEN {
            return Err(Error::ManifestTooLong { len: manifest_len });
        }
        Ok(Self {
            manifest_len,
            payload_len: u64::from_le_bytes(bytes[20..28].try_into().unwrap()),
            release_key: PublicKey::from_bytes(bytes[32..64].try_into().unwrap()),
        })
    }

    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..6].copy_from_slice(&HEADER_VERSION.to_le_bytes());
        out[6..8].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
        out[8..10].copy_from_slice(&ALGORITHM_ED25519.to_le_bytes());
        out[12..16].copy_from_slice(&self.manifest_len.to_le_bytes());
        out[16..20].copy_from_slice(&(SIGNATURE_LEN as u32).to_le_bytes());
        out[20..28].copy_from_slice(&self.payload_len.to_le_bytes());
        out[32..64].copy_from_slice(self.release_key.as_bytes());
        out
    }
}

/// The parsed prefix: the header, its bytes, the manifest's bytes and the signature. Nothing in
/// it has been verified.
#[derive(Debug, Clone)]
pub struct Prefix {
    pub header: Header,
    pub header_bytes: [u8; HEADER_LEN],
    pub manifest: Vec<u8>,
    pub signature: Signature,
}

impl Prefix {
    /// The first 1024 bytes of a file whose whole length is `file_len`.
    pub fn parse(prefix: &[u8; PREFIX_LEN], file_len: u64) -> Result<Self, Error> {
        let header_bytes: [u8; HEADER_LEN] = prefix[..HEADER_LEN].try_into().unwrap();
        let header = Header::parse(&header_bytes)?;
        let expected = PREFIX_LEN as u128 + u128::from(header.payload_len);
        if u128::from(file_len) != expected {
            return Err(Error::Size {
                expected,
                actual: file_len,
            });
        }
        let manifest_end = HEADER_LEN + header.manifest_len as usize;
        let signature_end = manifest_end + SIGNATURE_LEN;
        if let Some(offset) = prefix[signature_end..].iter().position(|&b| b != 0) {
            return Err(Error::Padding {
                offset: signature_end + offset,
            });
        }
        Ok(Self {
            header,
            header_bytes,
            manifest: prefix[HEADER_LEN..manifest_end].to_vec(),
            signature: Signature::from_slice(&prefix[manifest_end..signature_end])?,
        })
    }

    /// The 1024 bytes of a prefix with this header, manifest and signature. `None` when the
    /// manifest's length is not the header's, or it does not fit.
    pub fn encode(
        header: &Header,
        manifest: &[u8],
        signature: &Signature,
    ) -> Option<[u8; PREFIX_LEN]> {
        if manifest.len() > MAX_MANIFEST_LEN || manifest.len() != header.manifest_len as usize {
            return None;
        }
        let mut out = [0u8; PREFIX_LEN];
        out[..HEADER_LEN].copy_from_slice(&header.encode());
        out[HEADER_LEN..HEADER_LEN + manifest.len()].copy_from_slice(manifest);
        out[HEADER_LEN + manifest.len()..HEADER_LEN + manifest.len() + SIGNATURE_LEN]
            .copy_from_slice(&signature.0);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(manifest_len: u32, payload_len: u64) -> Header {
        Header {
            manifest_len,
            payload_len,
            release_key: PublicKey::from_bytes([0x5a; 32]),
        }
    }

    #[test]
    fn the_header_is_at_adr_0060s_offsets() {
        let bytes = header(0x0102, 0x0807_0605_0403_0201).encode();
        let mut golden = [0u8; 64];
        golden[0..4].copy_from_slice(b"OMA1");
        golden[4] = 1; // version
        golden[6] = 64; // header length
        golden[8] = 1; // algorithm: ed25519
        golden[12] = 0x02; // manifest length, little-endian
        golden[13] = 0x01;
        golden[16] = 64; // signature length
        golden[20..28].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // payload length
        golden[32..64].copy_from_slice(&[0x5a; 32]); // release public key
        assert_eq!(bytes, golden);
        assert_eq!(
            Header::parse(&golden).unwrap(),
            header(0x0102, 0x0807_0605_0403_0201)
        );
    }

    #[test]
    fn every_fixed_field_holds_its_one_value() {
        let good = header(10, 0).encode();
        for (at, value) in [
            (4, 2),
            (6, 63),
            (8, 2),
            (10, 1),
            (11, 1),
            (16, 63),
            (28, 1),
            (31, 1),
        ] {
            let mut bytes = good;
            bytes[at] = value;
            assert!(
                matches!(Header::parse(&bytes), Err(Error::HeaderField { .. })),
                "byte {at} = {value}"
            );
        }
    }

    #[test]
    fn an_elf_a_script_and_anything_else_without_the_magic_is_not_a_bundle() {
        for start in [&b"\x7fELF"[..], b"#!/b", b"oma1", b"\0\0\0\0"] {
            let mut bytes = header(10, 0).encode();
            bytes[..4].copy_from_slice(start);
            assert!(matches!(Header::parse(&bytes), Err(Error::NotOma)));
        }
    }

    fn prefix(manifest_len: usize) -> [u8; PREFIX_LEN] {
        let manifest = vec![b'x'; manifest_len];
        let h = header(manifest_len as u32, 4096);
        Prefix::encode(&h, &manifest, &Signature([0x33; 64])).unwrap()
    }

    #[test]
    fn a_manifest_of_896_bytes_fits_and_897_does_not() {
        let p = Prefix::parse(&prefix(896), 1024 + 4096).unwrap();
        assert_eq!(p.manifest.len(), 896);
        assert_eq!(p.signature, Signature([0x33; 64]));

        let mut bytes = prefix(896);
        bytes[12..16].copy_from_slice(&897u32.to_le_bytes());
        assert!(matches!(
            Prefix::parse(&bytes, 1024 + 4096),
            Err(Error::ManifestTooLong { len: 897 })
        ));
        assert!(Prefix::encode(&header(897, 0), &[b'x'; 897], &Signature([0; 64])).is_none());
    }

    #[test]
    fn the_file_is_exactly_the_prefix_and_the_payload() {
        let bytes = prefix(100);
        assert!(Prefix::parse(&bytes, 1024 + 4096).is_ok());
        for len in [1024 + 4095, 1024 + 4097, 1024, 0] {
            assert!(
                matches!(Prefix::parse(&bytes, len), Err(Error::Size { .. })),
                "{len}"
            );
        }
        // A payload length that would overflow is a size mismatch, not a panic.
        let mut huge = bytes;
        huge[20..28].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            Prefix::parse(&huge, u64::MAX),
            Err(Error::Size { .. })
        ));
    }

    #[test]
    fn every_byte_after_the_signature_is_zero() {
        let good = prefix(100);
        for at in [64 + 100 + 64, 700, 1023] {
            let mut bytes = good;
            bytes[at] = 1;
            assert!(matches!(
                Prefix::parse(&bytes, 1024 + 4096),
                Err(Error::Padding { offset }) if offset == at
            ));
        }
    }
}
