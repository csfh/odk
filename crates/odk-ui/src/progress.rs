//! A determinate progress bar, on component tokens: a bar in a Role's colour on its track
//! (`docs/spec/odk-component-tokens.md`).

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px, relative,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::MotionDuration;
use odk_theme::semantic;

use crate::focus::{child_id, part_name};
use crate::interaction::tween;
use crate::layout::{StyledExt, h_flex, single_line, v_flex};
use crate::theme::colour;
use crate::{Tone, components};

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
/// accessible value is always the per cent, or the text `value_text` gives. A new value moves the
/// bar over the foundation's `normal` on its curve, at once with reduced motion. A paused bar is
/// grey, a disabled one greyer. For work of unknown length, use a `Spinner`.
#[derive(IntoElement)]
pub struct Progress {
    id: ElementId,
    name: SharedString,
    value: f64,
    max: f64,
    tone: Option<Tone>,
    paused: bool,
    disabled: bool,
    show_label: bool,
    show_value: bool,
    value_text: Option<SharedString>,
}

impl Progress {
    /// A bar named `name`, such as "Uploading photos", at nothing done of 100.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
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

    /// How much is done, out of `max` (100 by default). Values outside 0 to `max`, infinities
    /// and NaN are safe: see `fraction`.
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
    pub fn value_text(mut self, text: impl Into<SharedString>) -> Self {
        self.value_text = Some(text.into());
        self
    }

    fn done(&self) -> f32 {
        fraction(self.value, self.max)
    }

    fn shown_value(&self) -> SharedString {
        self.value_text
            .clone()
            .unwrap_or_else(|| percent(self.done()).into())
    }
}

impl Describe for Progress {
    fn accessible(&self) -> Accessible {
        let working = !self.paused && !self.disabled && self.done() < 1.0;
        Accessible::new(Role::Progress, self.name.to_string())
            .value(self.shown_value().to_string())
            .busy(working)
            .disabled(self.disabled)
    }
}

impl RenderOnce for Progress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let role = if self.paused {
            semantic::Role::Neutral
        } else {
            self.tone.map_or(semantic::Role::Accent, Into::into)
        };
        let s = components().progress(role, States::NONE.disabled(self.disabled));
        let done = self.done();
        let shown = tween(
            child_id(&self.id, "value"),
            done,
            MotionDuration::Normal,
            window,
            cx,
        );
        #[cfg(test)]
        crate::drawn::record(&crate::focus::id_path(&self.id), (s, done, shown));
        let value = self.shown_value();

        let mut column = v_flex()
            .debug_selector(|| part_name(&self.id, "progress"))
            .w_full()
            .min_w_0()
            .gap(px(s.gap));
        if self.show_label || self.show_value {
            let mut line = h_flex()
                .debug_selector(|| part_name(&self.id, "line"))
                .gap(px(s.gap))
                .text_role(s.type_role);
            line = line.child(
                single_line(div().flex_1().min_w_0().text_color(colour(s.label)))
                    .children(self.show_label.then_some(self.name)),
            );
            if self.show_value {
                line = line.child(
                    div()
                        .debug_selector(|| part_name(&self.id, "value"))
                        .flex_shrink_0()
                        .text_color(colour(s.value))
                        .child(value),
                );
            }
            column = column.child(line);
        }
        let radius = px(s.radius);
        column.child(
            div()
                .debug_selector(|| part_name(&self.id, "track"))
                .relative()
                .w_full()
                .flex_shrink_0()
                .h(px(s.height))
                .rounded(radius)
                .bg(colour(s.track))
                .child(
                    div()
                        .debug_selector(|| part_name(&self.id, "bar"))
                        .absolute()
                        .top_0()
                        .left_0()
                        .bottom_0()
                        .w(relative(shown))
                        .rounded(radius)
                        .bg(colour(s.bar)),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_value_is_a_fraction_from_0_to_1() {
        assert_eq!(fraction(42.0, 100.0), 0.42);
        assert_eq!(fraction(0.0, 100.0), 0.0);
        assert_eq!(fraction(100.0, 100.0), 1.0);
        assert_eq!(fraction(3.0, 7.0), (3.0f64 / 7.0) as f32);
        assert_eq!(fraction(-5.0, 100.0), 0.0, "below nothing");
        assert_eq!(fraction(250.0, 100.0), 1.0, "past done");
        assert_eq!(fraction(f64::NAN, 100.0), 0.0);
        assert_eq!(fraction(f64::INFINITY, 100.0), 1.0);
        assert_eq!(fraction(f64::NEG_INFINITY, 100.0), 0.0);
        assert_eq!(fraction(5.0, 0.0), 0.0, "nothing to do");
        assert_eq!(fraction(5.0, -10.0), 0.0);
        assert_eq!(fraction(5.0, f64::NAN), 0.0);
        assert_eq!(fraction(5.0, f64::INFINITY), 0.0);
    }

    #[test]
    fn the_value_is_whole_per_cent_and_never_rounds_to_an_end() {
        assert_eq!(percent(0.42), "42%");
        assert_eq!(percent(0.0), "0%");
        assert_eq!(percent(1.0), "100%");
        assert_eq!(percent(0.996), "99%", "not done yet");
        assert_eq!(percent(0.001), "1%", "started");
        assert_eq!(percent(1.0 / 3.0), "33%");
    }

    #[test]
    fn reports_its_value_and_whether_it_is_working() {
        let p = Progress::new("p", "Uploading").value(30.0);
        let a = p.accessible();
        assert_eq!(
            (a.role, a.name.as_str(), a.value.as_deref(), a.busy),
            (Role::Progress, "Uploading", Some("30%"), true)
        );
        let done = Progress::new("p", "Uploading").value(100.0).accessible();
        assert!(!done.busy);
        let paused = Progress::new("p", "Uploading")
            .value(30.0)
            .paused(true)
            .accessible();
        assert!(!paused.busy);
        let files = Progress::new("p", "Copying")
            .value(3.0)
            .max(7.0)
            .value_text("3 of 7 files")
            .accessible();
        assert_eq!(files.value.as_deref(), Some("3 of 7 files"));
    }
}
