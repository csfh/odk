//! A determinate progress bar on the Rune host: a bar in a Role's colour on its track
//! (`docs/spec/odk-component-tokens.md`).

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style;
use crate::text::{Flow, words};
use crate::tone::Tone;
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::semantic;
use rune::{El, col, el, row};

/// How much of the work is done, from 0 to 1, whatever it is given: a value past either end is
/// that end, and a value or a most that is not a number, or a most that is not above zero, is
/// nothing done. So no value can break the bar.
pub(crate) fn fraction(value: f64, max: f64) -> f32 {
    if !max.is_finite() || max <= 0.0 || value.is_nan() {
        return 0.0;
    }
    (value / max).clamp(0.0, 1.0) as f32
}

/// The value as it is shown: whole per cent, never `100%` before the work is done nor `0%` once
/// it has started.
pub(crate) fn percent(fraction: f32) -> String {
    let at = (fraction * 100.0).round() as u32;
    let at = match at {
        100 if fraction < 1.0 => 99,
        0 if fraction > 0.0 => 1,
        at => at,
    };
    format!("{at}%")
}

/// A bar that shows how much of a piece of work is done: the bar in the accent's colour (or the
/// tone's) on a track of its tint, as wide as its column. Optionally its name above it and its
/// value at the end of that line, so the amount never rests on the bar's width alone; its
/// accessible value is always the per cent, or the text `value_text` gives. A new value moves
/// the bar on Rune's spring, and at once with reduced motion. A paused bar is grey, a disabled
/// one greyer. For work of unknown length, use a `Spinner`.
pub struct Progress {
    id: String,
    name: String,
    value: f64,
    max: f64,
    tone: Option<Tone>,
    paused: bool,
    disabled: bool,
    show_label: bool,
    show_value: bool,
    value_text: Option<String>,
}

impl Progress {
    /// A bar named `name`, such as "Uploading photos", at nothing done of 100.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: 0.0,
            max: 100.0,
            tone: None,
            paused: false,
            disabled: false,
            show_label: false,
            show_value: false,
            value_text: None,
        }
    }

    /// How much is done, out of `max` (100 by default). Values outside 0 to `max`, infinities and
    /// NaN are safe: see `fraction`.
    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    /// How much there is to do, such as a count of files.
    pub fn max(mut self, max: f64) -> Self {
        self.max = max;
        self
    }

    /// A status for the work, such as `Tone::Success` once it is done or `Tone::Danger` when it
    /// failed, in place of the accent.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// Work that has stopped for now: the bar turns grey and keeps its value.
    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Shows the name above the bar.
    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// Shows the value, `42%` or `value_text`, at the end of the label's line.
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// The value in words, such as "3 of 7 files", shown and reported in place of the per cent.
    pub fn value_text(mut self, text: impl Into<String>) -> Self {
        self.value_text = Some(text.into());
        self
    }

    fn done(&self) -> f32 {
        fraction(self.value, self.max)
    }

    fn shown_value(&self) -> String {
        self.value_text
            .clone()
            .unwrap_or_else(|| percent(self.done()))
    }

    pub fn show<T>(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let role = if self.paused {
            semantic::Role::Neutral
        } else {
            self.tone.map_or(semantic::Role::Accent, Into::into)
        };
        let s = paint
            .tokens
            .progress(role, States::NONE.disabled(self.disabled));
        let done = self.done();
        let value = self.shown_value();

        let mut column = apply(col().key(self.id.clone()), &accessible)
            .range_value(f64::from(done) * 100.0, 0.0, 100.0)
            .w_full()
            .shrink()
            .gap(s.gap);
        if self.show_label || self.show_value {
            let mut line = row().gap(s.gap);
            if self.show_label {
                line = line.child(
                    words(&paint, self.name.clone(), s.type_role, s.label, Flow::End).grow(1.0),
                );
            }
            if self.show_value {
                line = line.child(words(&paint, value, s.type_role, s.value, Flow::End));
            }
            column = column.child(line);
        }
        column.child(
            el().key(format!("{}-track", self.id))
                .w_full()
                .h(s.height)
                .radius(s.radius)
                .fill(ink(s.track))
                .a11y_hidden()
                .child(
                    el().key(format!("{}-bar", self.id))
                        .w_frac(done)
                        .h(s.height)
                        .radius(s.radius)
                        .fill(ink(s.bar))
                        .a11y_hidden(),
                ),
        )
    }
}

impl Describe for Progress {
    fn accessible(&self) -> Accessible {
        let working = !self.paused && !self.disabled && self.done() < 1.0;
        Accessible::new(Role::Progress, self.name.clone())
            .value(self.shown_value())
            .busy(working)
            .disabled(self.disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_value_can_break_the_bar() {
        assert_eq!(fraction(50.0, 100.0), 0.5);
        assert_eq!(fraction(-5.0, 100.0), 0.0);
        assert_eq!(fraction(500.0, 100.0), 1.0);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!((0.0..=1.0).contains(&fraction(bad, 100.0)), "{bad}");
        }
        assert_eq!(fraction(5.0, 0.0), 0.0);
        assert_eq!(fraction(5.0, -3.0), 0.0);
        assert_eq!(fraction(5.0, f64::NAN), 0.0);
    }

    #[test]
    fn a_value_never_reads_as_done_before_it_is_nor_as_nothing_once_started() {
        assert_eq!(percent(0.0), "0%");
        assert_eq!(percent(0.004), "1%");
        assert_eq!(percent(0.996), "99%");
        assert_eq!(percent(1.0), "100%");
        assert_eq!(percent(0.42), "42%");
    }
}
