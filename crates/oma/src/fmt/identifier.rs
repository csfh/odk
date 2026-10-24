use std::fmt;
use std::str::FromStr;

use crate::fmt::Error;

const MAX_LEN: usize = 128;

/// A reverse-DNS application Identifier such as `com.example.foo`.
///
/// Two or more segments separated by `.`; each starts with `a`-`z` and contains only
/// `a`-`z`, `0`-`9` and `-`; at most 128 characters in all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier(String);

impl Identifier {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Identifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let bad = |reason| Error::Identifier(s.to_owned(), reason);
        if s.len() > MAX_LEN {
            return Err(bad("longer than 128 characters"));
        }
        let mut segments = 0;
        for segment in s.split('.') {
            segments += 1;
            let mut chars = segment.chars();
            if !chars.next().is_some_and(|c| c.is_ascii_lowercase()) {
                return Err(bad("every segment must start with a lowercase letter"));
            }
            if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(bad("segments may contain only a-z, 0-9 and '-'"));
            }
        }
        if segments < 2 {
            return Err(bad("needs at least two segments"));
        }
        Ok(Self(s.to_owned()))
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid() {
        for ok in ["com.example.foo", "a.b", "com.example.my-app2"] {
            assert!(ok.parse::<Identifier>().is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_invalid() {
        let too_long = format!("a.{}", "b".repeat(127));
        for bad in [
            "",
            "foo",
            "com..foo",
            "com.example.",
            ".com.example",
            "Com.example",
            "com.2example",
            "com.-example",
            "com.exa_mple",
            "com.example/foo",
            "com.exämple",
            too_long.as_str(),
        ] {
            assert!(bad.parse::<Identifier>().is_err(), "{bad}");
        }
    }

    #[test]
    fn accepts_exactly_128() {
        let max = format!("a.{}", "b".repeat(126));
        assert_eq!(max.len(), 128);
        assert!(max.parse::<Identifier>().is_ok());
    }
}
