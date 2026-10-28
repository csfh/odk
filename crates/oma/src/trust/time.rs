//! The one spelling of an instant the signed documents use: `YYYY-MM-DDTHH:MM:SSZ`, UTC
//! (ADR 0060, section 4). Which clock says "now", and whether the instant itself is already
//! past, are open (`docs/oma-trust-plan.md`, question 7); nothing here compares with now.

use std::fmt;

/// An instant as a signed document spells it, kept with its seconds since 1970 so that two can
/// be ordered.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Timestamp {
    text: String,
    seconds: i64,
}

impl Timestamp {
    /// Exactly `YYYY-MM-DDTHH:MM:SSZ`, with a real date and a time of day from `00:00:00` to
    /// `23:59:59`. `None` for anything else: another offset, fractions, a lowercase `t` or `z`.
    pub fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' {
            return None;
        }
        if b[16] != b':' || b[19] != b'Z' {
            return None;
        }
        let number = |from: usize, to: usize| -> Option<i64> {
            let digits = &b[from..to];
            digits
                .iter()
                .all(u8::is_ascii_digit)
                .then(|| digits.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
        };
        let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
        let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        let seconds =
            days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
        Some(Self {
            text: text.to_owned(),
            seconds,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Seconds since 1970-01-01T00:00:00Z.
    pub fn unix_seconds(&self) -> i64 {
        self.seconds
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.seconds.cmp(&other.seconds)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({})", self.text)
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a date in the proleptic Gregorian calendar (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_spelling_reads_and_orders() {
        // `date -u -d 2026-10-27T12:34:56Z +%s` gives 1793104496.
        let t = Timestamp::parse("2026-10-27T12:34:56Z").unwrap();
        assert_eq!(t.unix_seconds(), 1_793_104_496);
        assert_eq!(
            Timestamp::parse("1970-01-01T00:00:00Z")
                .unwrap()
                .unix_seconds(),
            0
        );
        assert_eq!(
            Timestamp::parse("2000-02-29T00:00:00Z")
                .unwrap()
                .unix_seconds(),
            951_782_400
        );
        assert!(t < Timestamp::parse("2026-10-27T12:34:57Z").unwrap());
        assert_eq!(t.to_string(), "2026-10-27T12:34:56Z");
    }

    #[test]
    fn every_other_spelling_and_every_impossible_instant_is_refused() {
        for bad in [
            "2026-10-27T12:34:56+00:00",
            "2026-10-27T12:34:56.0Z",
            "2026-10-27t12:34:56Z",
            "2026-10-27T12:34:56z",
            "2026-10-27 12:34:56Z",
            "2026-10-27T12:34:56",
            "2026-1-27T12:34:56Z",
            "2026-13-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-10-00T00:00:00Z",
            "2026-10-27T24:00:00Z",
            "2026-10-27T12:60:00Z",
            "2026-10-27T12:34:60Z",
            "+026-10-27T12:34:56Z",
            "",
        ] {
            assert!(Timestamp::parse(bad).is_none(), "{bad}");
        }
    }
}
