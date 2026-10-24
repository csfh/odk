//! A number field: an `Input` whose text is a number, with steppers (`docs/spec/odk-text-input.md`,
//! "Search, password and number fields"). What the text means is `number.rs`'s, apart from how
//! the field is drawn.

use std::rc::Rc;

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window};
use odk_a11y::{Accessible, Describe};
use odk_theme::foundation::Density;

use crate::Input;
use crate::button::Size;
use crate::number::{Rules, parse};

type NumberHandler = Rc<dyn Fn(Option<f64>, &mut Window, &mut App) + 'static>;

/// A field for a number, with a step down and a step up button at its end.
///
/// Everything an `Input` does with the keyboard it does. Up and Down step by `step`, as the
/// buttons do, and stop at `min` and `max`; the buttons are not tab stops, since Up and Down do
/// their job, and a click on one keeps focus in the field. Typing is free: `-`,
/// `1.` or `.5` may stand on the way to a number, and nothing is clamped while it is typed.
/// Enter and leaving the field commit the text: a number is clamped to the bounds and shown
/// tidily (`2.50` as `2.5`); text that is no number is kept, and the field shows itself
/// invalid until it is one. Ctrl+Z undoes a step or a commit like any edit.
///
/// Uncontrolled by default, starting from `default_value`; pass `value` to control it and
/// follow `on_change`, which hears the number after each edit, or `None` while the text is no
/// number. A controlled value replaces the text only when it changes, so a half-typed number is
/// never overwritten.
#[derive(IntoElement)]
pub struct NumberInput {
    input: Input,
    rules: Rules,
    on_change: Option<NumberHandler>,
    on_submit: Option<NumberHandler>,
}

impl NumberInput {
    /// A number field named `name` for assistive technology, such as "Quantity".
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            input: Input::new(id, name),
            rules: Rules::default(),
            on_change: None,
            on_submit: None,
        }
    }

    /// The number, making the field controlled. A number that is not finite is no number: the
    /// field is empty.
    pub fn value(mut self, value: f64) -> Self {
        self.input.extras.number_value = Some(value.is_finite().then_some(value));
        self
    }

    /// The number the field starts with; one that is not finite leaves it empty.
    pub fn default_value(mut self, value: f64) -> Self {
        if value.is_finite() {
            self.input = self.input.default_value(self.rules.text_of(value));
        }
        self
    }

    /// The least number a commit or a step leaves. A NaN is ignored.
    pub fn min(mut self, min: f64) -> Self {
        if !min.is_nan() {
            self.rules.min = Some(min);
        }
        self
    }

    /// The greatest number a commit or a step leaves. A NaN is ignored.
    pub fn max(mut self, max: f64) -> Self {
        if !max.is_nan() {
            self.rules.max = Some(max);
        }
        self
    }

    /// How far a step goes; one by default. Its decimals are the decimals a stepped number is
    /// shown with, so `0.1` steps from 0 to exactly 1 in ten.
    pub fn step(mut self, step: f64) -> Self {
        if step > 0.0 && step.is_finite() {
            self.rules.step = step;
        }
        self
    }

    /// Shown, muted, while the field is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    /// A step from the density the field would otherwise take, as for an `Input`.
    pub fn size(mut self, size: Size) -> Self {
        self.input = self.input.size(size);
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.input = self.input.density(density);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.input = self.input.disabled(disabled);
        self
    }

    /// Selectable and copyable, not editable; it does not step.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.input = self.input.read_only(read_only);
        self
    }

    /// Marks the number as wrong, as for an `Input`. A text that is no number is shown wrong
    /// once it is committed, without this.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.input = self.input.invalid(invalid);
        self
    }

    /// Reports that the field must be filled in.
    pub fn required(mut self, required: bool) -> Self {
        self.input = self.input.required(required);
        self
    }

    /// Called with the number after each edit, step and commit, or `None` while the text is
    /// no number.
    pub fn on_change(
        mut self,
        handler: impl Fn(Option<f64>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Called with the committed number when Enter is pressed.
    pub fn on_submit(
        mut self,
        handler: impl Fn(Option<f64>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    /// The `Input` that draws it, with its rules and handlers.
    pub(crate) fn input(self) -> Input {
        let mut input = self.input;
        input.extras.number = Some(self.rules);
        if let Some(f) = self.on_change {
            input = input.on_change(move |text, window, cx| f(parse(text), window, cx));
        }
        if let Some(f) = self.on_submit {
            input = input.on_submit(move |text, window, cx| f(parse(text), window, cx));
        }
        input
    }
}

impl Describe for NumberInput {
    fn accessible(&self) -> Accessible {
        let mut a = self.input.accessible();
        if let Some(Some(v)) = self.input.extras.number_value {
            a.value = Some(self.rules.text_of(v));
        }
        a
    }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.input().render(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_that_is_not_one_never_reaches_the_field() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let n = NumberInput::new("n", "Quantity").value(bad);
            assert_eq!(n.input.extras.number_value, Some(None), "{bad}");
            assert_eq!(n.accessible().value.as_deref(), Some(""), "{bad}");
        }
        let n = NumberInput::new("n", "Quantity").value(2.5);
        assert_eq!(n.accessible().value.as_deref(), Some("2.5"));
    }

    #[test]
    fn a_bound_that_is_not_a_number_is_no_bound() {
        let n = NumberInput::new("n", "Quantity")
            .min(f64::NAN)
            .max(f64::NAN);
        assert_eq!((n.rules.min, n.rules.max), (None, None));
        let n = NumberInput::new("n", "Quantity").min(1.0).min(f64::NAN);
        assert_eq!(n.rules.min, Some(1.0), "a NaN leaves the bound as it was");
    }
}
