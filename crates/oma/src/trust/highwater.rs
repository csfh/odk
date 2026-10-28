//! The high-water rule (ADR 0060, section 4): the monotonic version stored for a (root key id,
//! app id), against a file's. The semver in `manifest.toml` is not an input.

/// What is stored for an application: the highest version accepted and its payload's digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub version: u64,
    pub payload_sha256: [u8; 32],
}

/// How a file's version stands against the mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// No mark yet.
    First,
    /// A higher version: an upgrade, when the rest of ADR 0060 accepts the file.
    Higher,
    /// The marked version with the marked payload: the same file again.
    Same,
    /// A lower version: refused.
    Lower,
    /// The marked version with another payload: refused.
    SameVersionOtherPayload,
}

impl Verdict {
    pub fn of(mark: Option<&Mark>, version: u64, payload_sha256: &[u8; 32]) -> Self {
        let Some(mark) = mark else {
            return Verdict::First;
        };
        match version.cmp(&mark.version) {
            std::cmp::Ordering::Greater => Verdict::Higher,
            std::cmp::Ordering::Less => Verdict::Lower,
            std::cmp::Ordering::Equal if *payload_sha256 == mark.payload_sha256 => Verdict::Same,
            std::cmp::Ordering::Equal => Verdict::SameVersionOtherPayload,
        }
    }

    pub fn accepted(self) -> bool {
        matches!(self, Verdict::First | Verdict::Higher | Verdict::Same)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_is_refused_equal_only_with_the_stored_payload_and_higher_is_accepted() {
        let mark = Mark {
            version: 10,
            payload_sha256: [1; 32],
        };
        let cases = [
            (None, 0, [9; 32], Verdict::First),
            (Some(&mark), 9, [1; 32], Verdict::Lower),
            (Some(&mark), 0, [1; 32], Verdict::Lower),
            (Some(&mark), 10, [1; 32], Verdict::Same),
            (Some(&mark), 10, [2; 32], Verdict::SameVersionOtherPayload),
            (Some(&mark), 11, [2; 32], Verdict::Higher),
            (Some(&mark), u64::MAX, [2; 32], Verdict::Higher),
        ];
        for (mark, version, digest, expected) in cases {
            assert_eq!(Verdict::of(mark, version, &digest), expected, "{version}");
        }
        assert!(
            Verdict::First.accepted() && Verdict::Higher.accepted() && Verdict::Same.accepted()
        );
        assert!(!Verdict::Lower.accepted() && !Verdict::SameVersionOtherPayload.accepted());
    }
}
