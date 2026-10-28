//! Canonical JSON as ADR 0060 section 4 defines it, for the signed manifest and `oma-keys`:
//! - one value, with objects whose keys come in the one order the document names, and no others;
//! - no whitespace between tokens, and no byte after the value;
//! - integers in decimal, without a sign and without leading zeros;
//! - strings escaped only for the quotation mark and the reverse solidus.
//!
//! A string with a code point below U+0020 cannot be read or written yet: the ADR does not choose
//! its spelling (`docs/oma-trust-plan.md`, question 5), so such a document fails closed.
//!
//! The reader is driven by the document's shape: the caller asks for the next key by name, so a
//! missing, unknown, duplicate or misplaced field is the same error, the key that was expected.

use super::Error;

/// Reads one canonical document, front to back.
pub struct Reader<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let text = std::str::from_utf8(bytes).map_err(|_| Error::NotUtf8)?;
        Ok(Self { text, at: 0 })
    }

    fn fail<T>(&self, expected: &'static str) -> Result<T, Error> {
        Err(Error::Canonical {
            at: self.at,
            expected,
        })
    }

    fn byte(&mut self, b: u8, expected: &'static str) -> Result<(), Error> {
        if self.text.as_bytes().get(self.at) == Some(&b) {
            self.at += 1;
            Ok(())
        } else {
            self.fail(expected)
        }
    }

    pub fn begin_object(&mut self) -> Result<(), Error> {
        self.byte(b'{', "'{'")
    }

    pub fn end_object(&mut self) -> Result<(), Error> {
        self.byte(b'}', "'}'")
    }

    /// The key `name` next: after a comma unless it is the object's first.
    pub fn key(&mut self, name: &'static str, first: bool) -> Result<(), Error> {
        if !first {
            self.byte(b',', "','")?;
        }
        let start = self.at;
        let found = self.string_inner();
        if !matches!(found, Ok(ref s) if s == name) {
            self.at = start;
            return Err(Error::Canonical {
                at: start,
                expected: name,
            });
        }
        self.byte(b':', "':'")
    }

    pub fn begin_array(&mut self) -> Result<(), Error> {
        self.byte(b'[', "'['")
    }

    /// Whether another element follows: `false` at the closing `]`, which is consumed.
    pub fn next_element(&mut self, first: bool) -> Result<bool, Error> {
        if self.text.as_bytes().get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(false);
        }
        if !first {
            self.byte(b',', "',' or ']'")?;
        }
        Ok(true)
    }

    pub fn string(&mut self) -> Result<String, Error> {
        self.string_inner()
    }

    fn string_inner(&mut self) -> Result<String, Error> {
        self.byte(b'"', "a string")?;
        let mut out = String::new();
        let rest = &self.text[self.at..];
        let mut chars = rest.char_indices();
        while let Some((i, c)) = chars.next() {
            let at = self.at + i;
            match c {
                '"' => {
                    self.at = at + 1;
                    return Ok(out);
                }
                '\\' => match chars.next().map(|(_, e)| e) {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    // An escaped control's spelling is open (question 5); every other escape is
                    // not canonical, since only those two characters are escaped.
                    Some('b' | 'f' | 'n' | 'r' | 't') => {
                        return Err(Error::ControlCharacter { at });
                    }
                    Some('u') => {
                        let digits: String = (0..4)
                            .filter_map(|_| chars.next().map(|(_, d)| d))
                            .collect();
                        return match u32::from_str_radix(&digits, 16) {
                            Ok(cp) if digits.len() == 4 && cp < 0x20 => {
                                Err(Error::ControlCharacter { at })
                            }
                            _ => Err(Error::Canonical {
                                at,
                                expected: "no escape but \\\" and \\\\",
                            }),
                        };
                    }
                    _ => {
                        return Err(Error::Canonical {
                            at,
                            expected: "no escape but \\\" and \\\\",
                        });
                    }
                },
                c if u32::from(c) < 0x20 => return Err(Error::ControlCharacter { at }),
                c => out.push(c),
            }
        }
        self.at = self.text.len();
        self.fail("the end of the string")
    }

    /// A non-negative integer that fits a `u64`, without a sign or leading zeros.
    pub fn u64(&mut self) -> Result<u64, Error> {
        let start = self.at;
        let digits = self.text.as_bytes()[start..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits == 0 {
            return self.fail("an integer");
        }
        if digits > 1 && self.text.as_bytes()[start] == b'0' {
            return self.fail("an integer without leading zeros");
        }
        let value = self.text[start..start + digits]
            .parse::<u64>()
            .or_else(|_| self.fail("an integer that fits 64 bits"))?;
        self.at += digits;
        Ok(value)
    }

    /// The document ends here: nothing after the value, not even a newline.
    pub fn finish(self) -> Result<(), Error> {
        if self.at == self.text.len() {
            Ok(())
        } else {
            self.fail("the end of the document")
        }
    }
}

/// Writes one canonical document, in the order the caller writes it.
#[derive(Default)]
pub struct Writer {
    out: String,
}

impl Writer {
    pub fn begin_object(&mut self) {
        self.out.push('{');
    }

    pub fn end_object(&mut self) {
        self.out.push('}');
    }

    pub fn key(&mut self, name: &str, first: bool) {
        if !first {
            self.out.push(',');
        }
        // Key names are the documents' own, plain ASCII with nothing to escape.
        self.out.push('"');
        self.out.push_str(name);
        self.out.push_str("\":");
    }

    pub fn begin_array(&mut self) {
        self.out.push('[');
    }

    pub fn element(&mut self, first: bool) {
        if !first {
            self.out.push(',');
        }
    }

    pub fn end_array(&mut self) {
        self.out.push(']');
    }

    /// Fails for a string with a code point below U+0020, whose spelling is open (question 5).
    pub fn string(&mut self, s: &str) -> Result<(), Error> {
        if let Some(at) = s.chars().position(|c| u32::from(c) < 0x20) {
            return Err(Error::ControlCharacter { at });
        }
        self.out.push('"');
        for c in s.chars() {
            if c == '"' || c == '\\' {
                self.out.push('\\');
            }
            self.out.push(c);
        }
        self.out.push('"');
        Ok(())
    }

    pub fn u64(&mut self, n: u64) {
        self.out.push_str(&n.to_string());
    }

    pub fn finish(self) -> Vec<u8> {
        self.out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `{"a":"x","n":1}`, read as a document of exactly those two keys.
    fn read_pair(bytes: &[u8]) -> Result<(String, u64), Error> {
        let mut r = Reader::new(bytes)?;
        r.begin_object()?;
        r.key("a", true)?;
        let a = r.string()?;
        r.key("n", false)?;
        let n = r.u64()?;
        r.end_object()?;
        r.finish()?;
        Ok((a, n))
    }

    #[test]
    fn the_canonical_form_reads_and_writes_back_byte_for_byte() {
        for doc in [
            &br#"{"a":"x","n":1}"#[..],
            br#"{"a":"","n":0}"#,
            br#"{"a":"a \"quoted\" \\ and  spaces","n":18446744073709551615}"#,
            "{\"a\":\"caf\u{e9} \u{1f600} \u{7f}\",\"n\":7}".as_bytes(),
        ] {
            let (a, n) = read_pair(doc).unwrap();
            let mut w = Writer::default();
            w.begin_object();
            w.key("a", true);
            w.string(&a).unwrap();
            w.key("n", false);
            w.u64(n);
            w.end_object();
            assert_eq!(w.finish(), doc, "{}", String::from_utf8_lossy(doc));
        }
    }

    #[test]
    fn anything_but_the_one_form_is_refused() {
        for doc in [
            &b"\xef\xbb\xbf{\"a\":\"x\",\"n\":1}"[..],
            br#" {"a":"x","n":1}"#,
            br#"{ "a":"x","n":1}"#,
            br#"{"a" :"x","n":1}"#,
            br#"{"a": "x","n":1}"#,
            br#"{"a":"x" ,"n":1}"#,
            br#"{"a":"x","n":1} "#,
            b"{\"a\":\"x\",\"n\":1}\n",
            br#"{"a":"x","n":1}{}"#,
            br#"{"n":1,"a":"x"}"#,
            br#"{"a":"x"}"#,
            br#"{"a":"x","a":"y","n":1}"#,
            br#"{"a":"x","n":1,"z":2}"#,
            br#"{"A":"x","n":1}"#,
            br#"{"a":"x","n":-1}"#,
            br#"{"a":"x","n":-0}"#,
            br#"{"a":"x","n":01}"#,
            br#"{"a":"x","n":00}"#,
            br#"{"a":"x","n":1.0}"#,
            br#"{"a":"x","n":1e3}"#,
            br#"{"a":"x","n":18446744073709551616}"#,
            br#"{"a":"x","n":"1"}"#,
            br#"{"a":"\/","n":1}"#,
            br#"{"a":"\u0041","n":1}"#,
            br#"{"a":"\u00e9","n":1}"#,
            br#"{"a":"\x","n":1}"#,
            br#"{"a":x,"n":1}"#,
            br#"{"a":"x,"n":1}"#,
            b"{\"a\":\"\xff\",\"n\":1}",
        ] {
            assert!(read_pair(doc).is_err(), "{}", String::from_utf8_lossy(doc));
        }
    }

    #[test]
    fn a_control_character_is_left_to_question_5_whichever_way_it_is_spelled() {
        for doc in [
            &br#"{"a":"\n","n":1}"#[..],
            br#"{"a":"\t","n":1}"#,
            br#"{"a":"\u000a","n":1}"#,
            br#"{"a":"\u001F","n":1}"#,
            b"{\"a\":\"\x01\",\"n\":1}",
        ] {
            assert!(
                matches!(read_pair(doc), Err(Error::ControlCharacter { .. })),
                "{}",
                String::from_utf8_lossy(doc)
            );
        }
        assert!(matches!(
            Writer::default().string("line\nbreak"),
            Err(Error::ControlCharacter { at: 4 })
        ));
    }
}
