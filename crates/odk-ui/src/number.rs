//! The numbers of a `NumberInput`, apart from how they are drawn: what a field's text means,
//! a step up or down, and the text a committed value is shown as. No GPUI.
//!
//! The text being typed is not a number until it is committed (Enter, leaving the field or a
//! step), so `-`, `1.` and `.` are allowed on the way. Only a commit clamps to the bounds.

/// The rules a number field keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rules {
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// How far a step moves. Always above zero.
    pub step: f64,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            step: 1.0,
        }
    }
}

/// The most decimals a number is shown with, so floating-point noise never shows.
const MAX_DECIMALS: usize = 10;

/// What `text` means as a number: an optional sign, then digits with at most one decimal point,
/// at least one digit in all. Anything else, such as `-`, `1e5`, `inf` or an empty text, is no
/// number, and so is one too big to hold. Space around it is ignored.
pub(crate) fn parse(text: &str) -> Option<f64> {
    let t = text.trim();
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    let mut dots = 0;
    let mut any_digit = false;
    for c in digits.chars() {
        match c {
            '0'..='9' => any_digit = true,
            '.' => dots += 1,
            _ => return None,
        }
    }
    if !any_digit || dots > 1 {
        return None;
    }
    let value: f64 = t.parse().ok().filter(|v: &f64| v.is_finite())?;
    // `-0` is zero.
    Some(if value == 0.0 { 0.0 } else { value })
}

/// How many decimals `text` has after its point.
fn decimals_in(text: &str) -> usize {
    text.trim()
        .split_once('.')
        .map_or(0, |(_, after)| after.len())
        .min(MAX_DECIMALS)
}

/// How many decimals `step` has: 0.1 has one, 0.25 two, 5 none.
fn decimals_of(step: f64) -> usize {
    let text = format!("{step}");
    decimals_in(&text)
}

impl Rules {
    /// `value` inside the bounds.
    pub(crate) fn clamp(&self, value: f64) -> f64 {
        let value = self.min.map_or(value, |min| value.max(min));
        self.max.map_or(value, |max| value.min(max))
    }

    /// The decimals a value is shown with: as many as the step has, or as many as were typed.
    fn decimals(&self, typed: &str) -> usize {
        decimals_of(self.step).max(decimals_in(typed))
    }

    /// The text a committed value is shown as, with `decimals` places at most and no trailing
    /// zeros.
    fn show(value: f64, decimals: usize) -> String {
        let text = format!("{value:.decimals$}");
        let text = if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            text
        };
        if text == "-0" { "0".to_owned() } else { text }
    }

    /// What committing `text` makes of it: a number clamped to the bounds and shown again, or
    /// `None` for a text that is no number, which is kept as it is so nothing typed is lost.
    /// An empty text stays empty.
    pub(crate) fn commit(&self, text: &str) -> Option<String> {
        let value = parse(text)?;
        Some(Self::show(self.clamp(value), self.decimals(text)))
    }

    /// The text after `steps` steps from `text` (up when positive), clamped. A text that is no
    /// number starts from the lower bound, or zero inside the bounds. The result is rounded to
    /// the step's decimals, so ten steps of 0.1 from 0 are exactly 1.
    pub(crate) fn step(&self, text: &str, steps: f64) -> String {
        let from = parse(text).unwrap_or_else(|| self.clamp(0.0));
        let decimals = self.decimals(text);
        let scale = 10f64.powi(decimals as i32);
        let to = ((from + steps * self.step) * scale).round() / scale;
        Self::show(self.clamp(to), decimals)
    }

    /// The text `value` is shown as, given from outside.
    pub(crate) fn text_of(&self, value: f64) -> String {
        Self::show(value, decimals_of(self.step).max(decimals_of(value)))
    }

    /// Whether a step up or down can still move from `text`.
    pub(crate) fn can_step(&self, text: &str, up: bool) -> bool {
        match (parse(text), up) {
            (Some(v), true) => self.max.is_none_or(|max| v < max),
            (Some(v), false) => self.min.is_none_or(|min| v > min),
            (None, _) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_typed_numbers_are_no_number_yet() {
        for text in [
            "", " ", "-", "+", ".", "-.", "1..2", "1e5", "inf", "NaN", "1,5", "--1",
        ] {
            assert_eq!(parse(text), None, "{text:?}");
        }
        assert_eq!(parse("1."), Some(1.0));
        assert_eq!(parse(".5"), Some(0.5));
        assert_eq!(parse("-0"), Some(0.0));
        assert!(parse("-0").unwrap().is_sign_positive(), "-0 is zero");
        assert_eq!(parse(" -12.25 "), Some(-12.25));
        assert_eq!(parse("+3"), Some(3.0));
    }

    #[test]
    fn a_number_too_big_to_hold_is_no_number() {
        let huge = "9".repeat(400);
        assert_eq!(parse(&huge), None);
        assert_eq!(Rules::default().commit(&huge), None, "kept as typed");
    }

    #[test]
    fn a_commit_clamps_and_tidies_and_keeps_what_is_no_number() {
        let r = Rules {
            min: Some(-5.0),
            max: Some(10.0),
            step: 1.0,
        };
        assert_eq!(r.commit("12").as_deref(), Some("10"));
        assert_eq!(r.commit("-9").as_deref(), Some("-5"));
        assert_eq!(r.commit("1.").as_deref(), Some("1"));
        assert_eq!(
            r.commit("2.50").as_deref(),
            Some("2.5"),
            "typed decimals stay"
        );
        assert_eq!(r.commit("-0").as_deref(), Some("0"));
        assert_eq!(r.commit("-"), None);
        assert_eq!(r.commit(""), None);
        assert_eq!(r.commit("abc"), None);
    }

    #[test]
    fn steps_round_to_the_step_and_stop_at_the_bounds() {
        let tenth = Rules {
            min: Some(0.0),
            max: Some(1.0),
            step: 0.1,
        };
        let mut text = "0".to_owned();
        for _ in 0..10 {
            text = tenth.step(&text, 1.0);
        }
        assert_eq!(text, "1", "no floating-point drift");
        assert_eq!(tenth.step("1", 1.0), "1", "held at the top");
        assert_eq!(tenth.step("0.05", -1.0), "0", "held at the bottom");
        assert!(!tenth.can_step("1", true) && tenth.can_step("1", false));

        let whole = Rules::default();
        assert_eq!(whole.step("", 1.0), "1", "no number starts from zero");
        assert_eq!(whole.step("-", -1.0), "-1");
        assert_eq!(whole.step("2.5", 1.0), "3.5", "typed decimals stay");
        let above = Rules {
            min: Some(5.0),
            ..Rules::default()
        };
        assert_eq!(above.step("", 1.0), "6", "starts inside the bounds");
    }

    #[test]
    fn values_from_outside_are_shown_without_noise() {
        let r = Rules {
            step: 0.01,
            ..Rules::default()
        };
        assert_eq!(r.text_of(0.1 + 0.2), "0.3");
        assert_eq!(r.text_of(-0.0), "0");
        assert_eq!(Rules::default().text_of(42.0), "42");
        assert_eq!(Rules::default().text_of(2.5), "2.5");
    }
}
