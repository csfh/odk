//! The signed manifest (ADR 0060, section 4): twelve fields, in one order, in canonical JSON. It is
//! a different document from the Bundle's `manifest.toml` (ADR 0006), which phase 3 checks it
//! against.

use std::fmt;

use super::canonical::{Reader, Writer};
use super::{Error, KeyId, Timestamp, hex, unhex};
use crate::fmt::Identifier;

/// A publisher's host name (ADR 0060, section 4): lowercase letters, digits, hyphens and dots, at
/// most 253 characters, not an IP address. An internationalized name is its punycode form.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Publisher(String);

impl Publisher {
    pub fn parse(host: &str) -> Result<Self, Error> {
        let bad = |reason: &str| Error::Field {
            field: "publisher",
            reason: format!("{host:?} {reason}"),
        };
        if host.is_empty() || host.len() > 253 {
            return Err(bad("must be 1 to 253 characters"));
        }
        for label in host.split('.') {
            if label.is_empty() || label.len() > 63 {
                return Err(bad(
                    "has a label that is empty or longer than 63 characters",
                ));
            }
            if !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(bad(
                    "may hold only lowercase letters, digits, hyphens and dots",
                ));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(bad("has a label that starts or ends with a hyphen"));
            }
        }
        // A name whose last label is a number is an IPv4 address to a resolver, in any of its
        // spellings (`127.1`, `2130706433`, `0x7f.1`): the WHATWG URL standard's rule.
        let last = host.rsplit('.').next().unwrap_or_default();
        let hex_number = last
            .strip_prefix("0x")
            .is_some_and(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
        if last.bytes().all(|b| b.is_ascii_digit()) || hex_number {
            return Err(bad("is an IP address, not a host name"));
        }
        Ok(Self(host.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Publisher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Publisher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Publisher({})", self.0)
    }
}

/// The one algorithm of this version.
pub const ALGORITHM: &str = "ed25519";

/// The signed manifest, field for field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedManifest {
    pub app_id: Identifier,
    /// The monotonic version the high-water mark stores, not `manifest.toml`'s semver.
    pub version: u64,
    /// The executable's path in the image, held to `manifest.toml`'s rules for `executable`.
    pub entry: String,
    /// SHA-256 of every byte from offset 1024 to the end of the file.
    pub payload_sha256: [u8; 32],
    /// SHA-256 of the 64-byte header.
    pub header_sha256: [u8; 32],
    /// The release key in the header.
    pub key_id: KeyId,
    /// The publisher root that authorizes the release key.
    pub root_key_id: KeyId,
    pub issued_at: Timestamp,
    pub expires_at: Timestamp,
    pub update_url: String,
    pub publisher: Publisher,
}

impl SignedManifest {
    /// The canonical bytes, and only those, with every field's rule checked.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(bytes)?;
        r.begin_object()?;
        r.key("algorithm", true)?;
        let algorithm = r.string()?;
        if algorithm != ALGORITHM {
            return Err(field(
                "algorithm",
                format!("is {algorithm:?}; only \"ed25519\" is known"),
            ));
        }
        r.key("app_id", false)?;
        let app_id = r
            .string()?
            .parse::<Identifier>()
            .map_err(|e| field("app_id", e.to_string()))?;
        r.key("version", false)?;
        let version = r.u64()?;
        r.key("entry", false)?;
        let entry = r.string()?;
        crate::fmt::check_relative_path("entry", &entry)
            .map_err(|e| field("entry", e.to_string()))?;
        r.key("payload_sha256", false)?;
        let payload_sha256 = digest("payload_sha256", &r.string()?)?;
        r.key("header_sha256", false)?;
        let header_sha256 = digest("header_sha256", &r.string()?)?;
        r.key("key_id", false)?;
        let key_id = key_id_field("key_id", &r.string()?)?;
        r.key("root_key_id", false)?;
        let root_key_id = key_id_field("root_key_id", &r.string()?)?;
        r.key("issued_at", false)?;
        let issued_at = timestamp_field("issued_at", &r.string()?)?;
        r.key("expires_at", false)?;
        let expires_at = timestamp_field("expires_at", &r.string()?)?;
        if expires_at <= issued_at {
            return Err(field(
                "expires_at",
                format!("{expires_at} is not later than issued_at"),
            ));
        }
        r.key("update_url", false)?;
        let update_url = r.string()?;
        r.key("publisher", false)?;
        let publisher = Publisher::parse(&r.string()?)?;
        check_update_url(&update_url, &publisher)?;
        r.end_object()?;
        r.finish()?;
        Ok(Self {
            app_id,
            version,
            entry,
            payload_sha256,
            header_sha256,
            key_id,
            root_key_id,
            issued_at,
            expires_at,
            update_url,
            publisher,
        })
    }

    /// The canonical bytes of this manifest: what is signed and what goes in the prefix.
    pub fn to_canonical(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::default();
        w.begin_object();
        let strings: [(&str, String); 12] = [
            ("algorithm", ALGORITHM.to_owned()),
            ("app_id", self.app_id.to_string()),
            ("version", String::new()),
            ("entry", self.entry.clone()),
            ("payload_sha256", hex(&self.payload_sha256)),
            ("header_sha256", hex(&self.header_sha256)),
            ("key_id", self.key_id.to_string()),
            ("root_key_id", self.root_key_id.to_string()),
            ("issued_at", self.issued_at.to_string()),
            ("expires_at", self.expires_at.to_string()),
            ("update_url", self.update_url.clone()),
            ("publisher", self.publisher.to_string()),
        ];
        for (i, (name, value)) in strings.iter().enumerate() {
            w.key(name, i == 0);
            if *name == "version" {
                w.u64(self.version);
            } else {
                w.string(value)?;
            }
        }
        w.end_object();
        Ok(w.finish())
    }
}

fn field(field: &'static str, reason: String) -> Error {
    Error::Field { field, reason }
}

fn digest(name: &'static str, s: &str) -> Result<[u8; 32], Error> {
    unhex::<32>(s).ok_or_else(|| field(name, format!("{s:?} is not 64 lowercase hex digits")))
}

pub(crate) fn key_id_field(name: &'static str, s: &str) -> Result<KeyId, Error> {
    s.parse()
        .map_err(|()| field(name, format!("{s:?} is not 16 lowercase hex digits")))
}

pub(crate) fn timestamp_field(name: &'static str, s: &str) -> Result<Timestamp, Error> {
    Timestamp::parse(s).ok_or_else(|| field(name, format!("{s:?} is not YYYY-MM-DDTHH:MM:SSZ")))
}

/// `https://` and the publisher, then nothing, or a path, query or fragment of visible ASCII. No
/// user, no port, and no other host (ADR 0060, section 4).
fn check_update_url(url: &str, publisher: &Publisher) -> Result<(), Error> {
    let bad = |reason: &str| field("update_url", format!("{url:?} {reason}"));
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| bad("does not start with https://"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains(['@', ':']) {
        return Err(bad("names a user or a port"));
    }
    if authority != publisher.as_str() {
        return Err(bad(&format!("is not on the publisher's host, {publisher}")));
    }
    if !tail.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(bad("holds a character a URL does not"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A manifest every field of which is valid.
    pub(crate) fn sample() -> SignedManifest {
        SignedManifest {
            app_id: "dev.csfh.notes".parse().unwrap(),
            version: 7,
            entry: "bin/notes".to_owned(),
            payload_sha256: [0xab; 32],
            header_sha256: [0xcd; 32],
            key_id: "0123456789abcdef".parse().unwrap(),
            root_key_id: "fedcba9876543210".parse().unwrap(),
            issued_at: Timestamp::parse("2026-10-27T00:00:00Z").unwrap(),
            expires_at: Timestamp::parse("2027-10-27T00:00:00Z").unwrap(),
            update_url: "https://apps.csfh.dev/notes/latest.oma".to_owned(),
            publisher: Publisher::parse("apps.csfh.dev").unwrap(),
        }
    }

    const CANONICAL: &str = concat!(
        r#"{"algorithm":"ed25519","app_id":"dev.csfh.notes","version":7,"entry":"bin/notes","#,
        r#""payload_sha256":"abababababababababababababababababababababababababababababababab","#,
        r#""header_sha256":"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd","#,
        r#""key_id":"0123456789abcdef","root_key_id":"fedcba9876543210","#,
        r#""issued_at":"2026-10-27T00:00:00Z","expires_at":"2027-10-27T00:00:00Z","#,
        r#""update_url":"https://apps.csfh.dev/notes/latest.oma","publisher":"apps.csfh.dev"}"#
    );

    #[test]
    fn the_twelve_fields_in_their_order_are_the_canonical_document() {
        assert_eq!(
            String::from_utf8(sample().to_canonical().unwrap()).unwrap(),
            CANONICAL
        );
        assert_eq!(
            SignedManifest::parse(CANONICAL.as_bytes()).unwrap(),
            sample()
        );
    }

    fn with(from: &str, to: &str) -> String {
        assert!(CANONICAL.contains(from), "{from}");
        CANONICAL.replacen(from, to, 1)
    }

    #[test]
    fn a_field_out_of_place_extra_missing_or_doubled_is_refused() {
        for doc in [
            with(
                r#""version":7,"entry":"bin/notes","#,
                r#""entry":"bin/notes","version":7,"#,
            ),
            with(
                r#","publisher":"apps.csfh.dev"}"#,
                r#","publisher":"apps.csfh.dev","extra":1}"#,
            ),
            with(r#","publisher":"apps.csfh.dev"}"#, "}"),
            with(r#""version":7,"#, r#""version":7,"version":7,"#),
            with("{", " {"),
            format!("{CANONICAL}\n"),
        ] {
            assert!(SignedManifest::parse(doc.as_bytes()).is_err(), "{doc}");
        }
    }

    #[test]
    fn each_field_is_held_to_its_rule() {
        for (from, to) in [
            (r#""ed25519""#, r#""ed448""#),
            (r#""dev.csfh.notes""#, r#""Notes""#),
            (r#""version":7"#, r#""version":-7"#),
            (r#""bin/notes""#, r#""/bin/notes""#),
            (r#""bin/notes""#, r#""../notes""#),
            (r#""bin/notes""#, r#""""#),
            (
                "abababababababababababababababababababababababababababababababab",
                "ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB",
            ),
            (
                "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
                "cdcd",
            ),
            ("0123456789abcdef", "0123456789abcde"),
            ("fedcba9876543210", "FEDCBA9876543210"),
            ("2026-10-27T00:00:00Z", "2026-10-27T00:00:00+00:00"),
            ("2027-10-27T00:00:00Z", "2026-10-27T00:00:00Z"),
            ("2027-10-27T00:00:00Z", "2026-10-26T23:59:59Z"),
        ] {
            let doc = with(from, to);
            assert!(
                SignedManifest::parse(doc.as_bytes()).is_err(),
                "{from} -> {to}"
            );
        }
    }

    #[test]
    fn the_update_url_is_https_on_the_publishers_host_with_no_user_or_port() {
        for url in [
            "http://apps.csfh.dev/notes/latest.oma",
            "https://apps.csfh.dev:443/notes/latest.oma",
            "https://me@apps.csfh.dev/notes/latest.oma",
            "https://other.csfh.dev/notes/latest.oma",
            "https://apps.csfh.dev.evil.example/notes",
            "https://APPS.csfh.dev/notes",
            "https://apps.csfh.dev/notes latest.oma",
            "https://apps.csfh.dev/notes/l\u{e9}test.oma",
            "apps.csfh.dev/notes",
        ] {
            let doc = with("https://apps.csfh.dev/notes/latest.oma", url);
            assert!(SignedManifest::parse(doc.as_bytes()).is_err(), "{url}");
        }
        for url in [
            "https://apps.csfh.dev",
            "https://apps.csfh.dev/",
            "https://apps.csfh.dev?channel=stable",
            "https://apps.csfh.dev#latest",
        ] {
            let doc = with("https://apps.csfh.dev/notes/latest.oma", url);
            assert!(SignedManifest::parse(doc.as_bytes()).is_ok(), "{url}");
        }
    }

    #[test]
    fn a_publisher_is_a_host_name_and_never_an_address() {
        for ok in [
            "apps.csfh.dev",
            "localhost",
            "xn--bcher-kva.example",
            "a1.b-2.c3",
        ] {
            assert!(Publisher::parse(ok).is_ok(), "{ok}");
        }
        let long = format!("{}.dev", "a".repeat(250));
        let label = format!("{}.dev", "a".repeat(64));
        for bad in [
            "",
            "Apps.csfh.dev",
            "apps.csfh.dev.",
            ".apps.csfh.dev",
            "apps..csfh.dev",
            "-apps.csfh.dev",
            "apps-.csfh.dev",
            "apps_csfh.dev",
            "https://apps.csfh.dev",
            "apps.csfh.dev:443",
            "b\u{fc}cher.example",
            "127.0.0.1",
            "127.1",
            "2130706433",
            "0x7f.1",
            "10.0.0.0x1",
            "foo.0x",
            long.as_str(),
            label.as_str(),
        ] {
            assert!(Publisher::parse(bad).is_err(), "{bad}");
        }
    }
}
