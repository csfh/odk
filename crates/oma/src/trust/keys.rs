//! `oma-keys` (ADR 0060, section 3): the exact set of release keys a publisher root authorizes,
//! signed by that root. This module reads and verifies the body it is given; fetching it is the
//! install phase's (ADR 0066).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::canonical::{Reader, Writer};
use super::manifest::{ALGORITHM, key_id_field, timestamp_field};
use super::signature::{self, Context, Signature};
use super::{Error, KeyId, PublicKey, Timestamp};

/// At most this many release keys in one set.
pub const MAX_KEYS: usize = 8;
/// The largest body that is read (16 KiB).
pub const MAX_BODY: usize = 16 * 1024;

/// One authorized release key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseKey {
    pub id: KeyId,
    pub not_after: Timestamp,
    pub public_key: PublicKey,
}

/// A root's set of release keys, as its body says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmaKeys {
    pub root_public_key: PublicKey,
    pub root_key_id: KeyId,
    pub version: u64,
    /// Sorted by id, each id that of its key, one to [`MAX_KEYS`] of them.
    pub keys: Vec<ReleaseKey>,
}

/// Which root a body must be signed by.
#[derive(Debug, Clone, Copy)]
pub enum Signer<'a> {
    /// No root is pinned yet: the body is checked under the root it names, which the person is
    /// about to pin through the first-install dialog.
    FirstContact,
    /// A root is pinned: the body must name it and be signed by it. A body from any other root
    /// is a different publisher.
    Pinned(&'a PublicKey),
}

/// How a verified set's version stands against the one stored for its root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetChange {
    /// Nothing is stored for this root yet.
    First,
    /// Lower than the stored version: rejected.
    Lower,
    /// The stored version again. What that means is open (`docs/oma-trust-plan.md`, question 4).
    Same,
    /// Higher: it replaces the stored set.
    Higher,
}

impl SetChange {
    pub fn of(stored: Option<u64>, version: u64) -> Self {
        match stored {
            None => SetChange::First,
            Some(s) if version < s => SetChange::Lower,
            Some(s) if version == s => SetChange::Same,
            Some(_) => SetChange::Higher,
        }
    }
}

/// A set that [`verify`] checked under the root its [`Signer`] named. Only [`verify`] makes one:
/// a set from [`OmaKeys::parse`] alone is unverified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedKeys(OmaKeys);

impl VerifiedKeys {
    pub fn set(&self) -> &OmaKeys {
        &self.0
    }

    /// The root that signed it.
    pub fn root(&self) -> &PublicKey {
        &self.0.root_public_key
    }
}

impl OmaKeys {
    /// Reads a body and checks every rule of section 3, but not its signature: see [`verify`]
    /// for a set that can be trusted. An empty set,
    /// a duplicate id, an id that is not its key's and a set out of order are each a failed
    /// document, never a set that lists nothing.
    pub fn parse(body: &[u8]) -> Result<Self, Error> {
        if body.len() > MAX_BODY {
            return Err(Error::KeysTooLarge { len: body.len() });
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
        r.key("root_public_key", false)?;
        let root_public_key = public_key("root_public_key", &r.string()?)?;
        r.key("root_key_id", false)?;
        let root_key_id = key_id_field("root_key_id", &r.string()?)?;
        if root_key_id != root_public_key.key_id() {
            return Err(field(
                "root_key_id",
                format!("{root_key_id} is not the key id of root_public_key"),
            ));
        }
        r.key("version", false)?;
        let version = r.u64()?;
        r.key("keys", false)?;
        r.begin_array()?;
        let mut keys: Vec<ReleaseKey> = Vec::new();
        while r.next_element(keys.is_empty())? {
            if keys.len() == MAX_KEYS {
                return Err(field(
                    "keys",
                    format!("lists more than {MAX_KEYS} release keys"),
                ));
            }
            r.begin_object()?;
            r.key("id", true)?;
            let id = key_id_field("keys.id", &r.string()?)?;
            r.key("not_after", false)?;
            let not_after = timestamp_field("keys.not_after", &r.string()?)?;
            r.key("public_key", false)?;
            let public_key = public_key("keys.public_key", &r.string()?)?;
            r.end_object()?;
            if id != public_key.key_id() {
                return Err(field(
                    "keys.id",
                    format!("{id} is not the key id of its public_key"),
                ));
            }
            // Strictly increasing ids: sorted, with no id twice.
            if keys.last().is_some_and(|previous| previous.id >= id) {
                return Err(field(
                    "keys",
                    format!("{id} is out of order or listed twice"),
                ));
            }
            keys.push(ReleaseKey {
                id,
                not_after,
                public_key,
            });
        }
        if keys.is_empty() {
            return Err(field("keys", "is empty".to_owned()));
        }
        r.end_object()?;
        r.finish()?;
        Ok(Self {
            root_public_key,
            root_key_id,
            version,
            keys,
        })
    }

    /// The canonical body of this set: what the root signs and the publisher serves.
    pub fn to_canonical(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::default();
        w.begin_object();
        w.key("algorithm", true);
        w.string(ALGORITHM)?;
        w.key("root_public_key", false);
        w.string(&STANDARD.encode(self.root_public_key.as_bytes()))?;
        w.key("root_key_id", false);
        w.string(&self.root_key_id.to_string())?;
        w.key("version", false);
        w.u64(self.version);
        w.key("keys", false);
        w.begin_array();
        for (i, key) in self.keys.iter().enumerate() {
            w.element(i == 0);
            w.begin_object();
            w.key("id", true);
            w.string(&key.id.to_string())?;
            w.key("not_after", false);
            w.string(key.not_after.as_str())?;
            w.key("public_key", false);
            w.string(&STANDARD.encode(key.public_key.as_bytes()))?;
            w.end_object();
        }
        w.end_array();
        w.end_object();
        Ok(w.finish())
    }

    /// The entry for `key`, if this set authorizes it. Whether it is revoked or past its
    /// `not_after` is the install phase's to ask.
    pub fn release_key(&self, key: &PublicKey) -> Option<&ReleaseKey> {
        self.keys.iter().find(|k| k.public_key == *key)
    }
}

/// A body and its raw 64-byte signature (`oma-keys.sig`), checked under the root `signer` says.
pub fn verify(body: &[u8], signature: &[u8], signer: Signer<'_>) -> Result<VerifiedKeys, Error> {
    let keys = OmaKeys::parse(body)?;
    let signature = Signature::from_slice(signature)?;
    let root = match signer {
        Signer::FirstContact => keys.root_public_key,
        Signer::Pinned(pinned) => {
            if keys.root_public_key != *pinned {
                return Err(Error::DifferentRoot {
                    found: keys.root_key_id,
                    pinned: pinned.key_id(),
                });
            }
            *pinned
        }
    };
    signature::verify(&root, Context::KEYS, body, &signature)?;
    Ok(VerifiedKeys(keys))
}

fn field(field: &'static str, reason: String) -> Error {
    Error::Field { field, reason }
}

/// Standard base64 of exactly 32 bytes, in its one spelling: decoded, then encoded again to the
/// same text.
fn public_key(name: &'static str, s: &str) -> Result<PublicKey, Error> {
    let bad = || {
        field(
            name,
            format!("{s:?} is not the standard base64 of 32 bytes"),
        )
    };
    let bytes = STANDARD.decode(s).map_err(|_| bad())?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| bad())?;
    if STANDARD.encode(bytes) != s {
        return Err(bad());
    }
    Ok(PublicKey::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::SigningKey;

    fn root() -> SigningKey {
        SigningKey::from_seed(&[0x11; 32]).unwrap()
    }

    fn release(seed: u8) -> ReleaseKey {
        let public_key = SigningKey::from_seed(&[seed; 32]).unwrap().public_key();
        ReleaseKey {
            id: public_key.key_id(),
            not_after: Timestamp::parse("2027-01-01T00:00:00Z").unwrap(),
            public_key,
        }
    }

    /// A set of `n` release keys, sorted as the document requires.
    fn set(n: u8) -> OmaKeys {
        let mut keys: Vec<ReleaseKey> = (0..n).map(|i| release(0x40 + i)).collect();
        keys.sort_by_key(|k| k.id);
        let root_public_key = root().public_key();
        OmaKeys {
            root_public_key,
            root_key_id: root_public_key.key_id(),
            version: 3,
            keys,
        }
    }

    fn body(keys: &OmaKeys) -> String {
        String::from_utf8(keys.to_canonical().unwrap()).unwrap()
    }

    #[test]
    fn a_set_reads_and_writes_back_byte_for_byte() {
        for n in [1, 2, 8] {
            let doc = body(&set(n));
            assert_eq!(OmaKeys::parse(doc.as_bytes()).unwrap(), set(n));
            assert_eq!(body(&OmaKeys::parse(doc.as_bytes()).unwrap()), doc);
        }
        let one = body(&set(1));
        assert!(
            one.starts_with(r#"{"algorithm":"ed25519","root_public_key":""#),
            "{one}"
        );
        assert!(one.contains(r#","version":3,"keys":[{"id":""#), "{one}");
    }

    #[test]
    fn an_empty_set_too_many_keys_and_a_set_out_of_order_are_failed_documents() {
        let mut empty = set(1);
        empty.keys.clear();
        let mut nine = set(8);
        nine.keys.push(release(0x60));
        let mut twice = set(2);
        twice.keys[1] = twice.keys[0].clone();
        let mut reversed = set(2);
        reversed.keys.reverse();
        for keys in [empty, nine, twice, reversed] {
            assert!(
                OmaKeys::parse(body(&keys).as_bytes()).is_err(),
                "{}",
                body(&keys)
            );
        }
    }

    #[test]
    fn every_id_is_its_keys_and_every_key_is_32_bytes_of_standard_base64() {
        let good = body(&set(1));
        let key = STANDARD.encode(set(1).keys[0].public_key.as_bytes());
        let id = set(1).keys[0].id.to_string();
        let root_id = set(1).root_key_id.to_string();
        let urlsafe = key.replace('+', "-").replace('/', "_");
        for (from, to) in [
            (id.as_str(), "0000000000000000"),
            (root_id.as_str(), "0000000000000000"),
            (key.as_str(), STANDARD.encode([0u8; 31]).as_str()),
            (key.as_str(), STANDARD.encode([0u8; 33]).as_str()),
            (key.as_str(), key.trim_end_matches('=')),
            (key.as_str(), urlsafe.as_str()),
            (r#""algorithm":"ed25519""#, r#""algorithm":"Ed25519""#),
            (r#","version":3"#, r#","version":03"#),
        ] {
            if from == to {
                continue;
            }
            let doc = good.replacen(from, to, 1);
            assert_ne!(doc, good, "{from}");
            assert!(OmaKeys::parse(doc.as_bytes()).is_err(), "{from} -> {to}");
        }
    }

    #[test]
    fn a_body_in_any_but_the_canonical_form_is_a_failed_document() {
        let good = body(&set(2));
        let entry_end = good.find("\"},{").unwrap() + 1;
        let unknown_in_entry = format!("{},\"extra\":1{}", &good[..entry_end], &good[entry_end..]);
        for doc in [
            format!("{good}\n"),
            format!("{good} "),
            format!("{good}{{}}"),
            good.replacen(r#"{"algorithm""#, r#"{"extra":1,"algorithm""#, 1),
            good.replacen("]}", r#"],"extra":1}"#, 1),
            unknown_in_entry,
            good.replacen(r#""keys":["#, r#""keys": ["#, 1),
            good.replacen(r#","version""#, r#", "version""#, 1),
            good.replacen("[{", "[ {", 1),
        ] {
            assert_ne!(doc, good);
            assert!(OmaKeys::parse(doc.as_bytes()).is_err(), "{doc}");
        }
    }

    #[test]
    fn a_body_over_16_kib_is_not_read() {
        let mut big = body(&set(1)).into_bytes();
        big.resize(MAX_BODY + 1, b' ');
        assert!(matches!(
            OmaKeys::parse(&big),
            Err(Error::KeysTooLarge { .. })
        ));
    }

    #[test]
    fn the_root_signs_the_exact_body_under_the_keys_context() {
        let doc = body(&set(2));
        let signed = root().sign(Context::KEYS, doc.as_bytes());
        let pinned = root().public_key();
        let first = verify(doc.as_bytes(), &signed.0, Signer::FirstContact).unwrap();
        assert_eq!(first.set(), &set(2));
        assert_eq!(first.root(), &pinned);
        let again = verify(doc.as_bytes(), &signed.0, Signer::Pinned(&pinned)).unwrap();
        assert_eq!(again.set(), &set(2));

        let as_manifest = root().sign(Context::MANIFEST, doc.as_bytes());
        assert!(verify(doc.as_bytes(), &as_manifest.0, Signer::FirstContact).is_err());
        assert!(verify(doc.as_bytes(), &signed.0[..63], Signer::FirstContact).is_err());
        let changed = doc.replacen("\"version\":3", "\"version\":4", 1);
        assert!(verify(changed.as_bytes(), &signed.0, Signer::FirstContact).is_err());
    }

    #[test]
    fn a_body_from_another_root_is_another_publisher_even_when_it_verifies() {
        let other = SigningKey::from_seed(&[0x22; 32]).unwrap();
        let mut theirs = set(1);
        theirs.root_public_key = other.public_key();
        theirs.root_key_id = other.public_key().key_id();
        let doc = body(&theirs);
        let signed = other.sign(Context::KEYS, doc.as_bytes());
        // On first contact it is simply that root's set.
        assert!(verify(doc.as_bytes(), &signed.0, Signer::FirstContact).is_ok());
        // Once a root is pinned, a set from another root is not applied to it.
        let pinned = root().public_key();
        assert!(matches!(
            verify(doc.as_bytes(), &signed.0, Signer::Pinned(&pinned)),
            Err(Error::DifferentRoot { .. })
        ));
    }

    #[test]
    fn a_set_signed_by_another_key_that_names_the_pinned_root_does_not_verify() {
        let doc = body(&set(1));
        let forged = SigningKey::from_seed(&[0x22; 32])
            .unwrap()
            .sign(Context::KEYS, doc.as_bytes());
        let pinned = root().public_key();
        assert!(matches!(
            verify(doc.as_bytes(), &forged.0, Signer::Pinned(&pinned)),
            Err(Error::BadSignature { .. })
        ));
        assert!(verify(doc.as_bytes(), &forged.0, Signer::FirstContact).is_err());
    }

    #[test]
    fn a_lower_version_is_rejected_a_higher_replaces_and_the_same_is_left_open() {
        assert_eq!(SetChange::of(None, 0), SetChange::First);
        assert_eq!(SetChange::of(Some(3), 2), SetChange::Lower);
        assert_eq!(SetChange::of(Some(3), 3), SetChange::Same);
        assert_eq!(SetChange::of(Some(3), 4), SetChange::Higher);
    }

    #[test]
    fn a_set_authorizes_exactly_its_own_keys() {
        let keys = set(2);
        assert!(keys.release_key(&keys.keys[1].public_key).is_some());
        assert!(keys.release_key(&release(0x70).public_key).is_none());
        assert!(keys.release_key(&keys.root_public_key).is_none());
    }
}
