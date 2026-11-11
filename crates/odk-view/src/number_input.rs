//! A number field on the Rune host: text that commits to a number on Enter or a step
//! (`docs/spec/odk-text-input.md`, rules in `number.rs`).

use std::rc::Rc;

use crate::button::{Button, Press};
use crate::host::Hosted;
use crate::icon::Icon;
use crate::input::{Input, KeyHook};
use crate::number::{Rules, parse};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::ButtonVariant;
use rune::{El, Key, KeyEvent, Tx};

type Changed<T> = Rc<dyn Fn(&mut T, Option<f64>)>;
type Committed<T> = Rc<dyn Fn(&mut T, f64)>;
type Step<T> = Rc<dyn Fn(&mut Hosted<T>, f64)>;

/// A field for a number. Typing is free: `-`, `1.` and `.` are allowed on the way, and the
/// application hears the number after each edit, or `None` while the text is no number. Enter and
/// the steppers commit: the number is clamped to `min` and `max`, tidied (`2.50` to `2.5`) and
/// `on_commit` hears it once for each change. Up and Down step by `step`. Text that is no number
/// stays as it is, so nothing typed is lost, and the field is invalid, described as "Enter a
/// number". The steppers are not tab stops and a press on one leaves focus in the field.
pub struct NumberInput<T> {
    id: String,
    name: String,
    rules: Rules,
    placeholder: Option<String>,
    default_value: Option<f64>,
    read_only: bool,
    invalid: bool,
    required: bool,
    disabled: bool,
    on_change: Option<Changed<T>>,
    on_commit: Option<Committed<T>>,
    on_submit: Option<Changed<T>>,
}

impl<T: 'static> NumberInput<T> {
    /// `name` is the field's accessible name, such as "Quantity".
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            rules: Rules::default(),
            placeholder: None,
            default_value: None,
            read_only: false,
            invalid: false,
            required: false,
            disabled: false,
            on_change: None,
            on_commit: None,
            on_submit: None,
        }
    }

    /// The number the field starts with; a number that is not finite is none.
    pub fn default_value(mut self, value: f64) -> Self {
        self.default_value = value.is_finite().then_some(value);
        self
    }

    /// The lowest a commit or a step goes. A NaN is ignored.
    pub fn min(mut self, min: f64) -> Self {
        self.rules.min = (!min.is_nan()).then_some(min);
        self
    }

    /// The highest a commit or a step goes. A NaN is ignored.
    pub fn max(mut self, max: f64) -> Self {
        self.rules.max = (!max.is_nan()).then_some(max);
        self
    }

    /// How far a step moves: above zero, and finite, or it stays 1.
    pub fn step(mut self, step: f64) -> Self {
        if step.is_finite() && step > 0.0 {
            self.rules.step = step;
        }
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// A field that shows its number and does not step or edit it.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Hears the number after each edit, or `None` while the text is no number.
    pub fn on_change(mut self, f: impl Fn(&mut T, Option<f64>) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Hears the settled number, once for each change: after Enter or a step.
    pub fn on_commit(mut self, f: impl Fn(&mut T, f64) + 'static) -> Self {
        self.on_commit = Some(Rc::new(f));
        self
    }

    /// Hears Enter, with the number as the field commits it, or `None` for text that is none.
    pub fn on_submit(mut self, f: impl Fn(&mut T, Option<f64>) + 'static) -> Self {
        self.on_submit = Some(Rc::new(f));
        self
    }
}

impl<T> Describe for NumberInput<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::TextInput, self.name.clone())
            .invalid(self.invalid)
            .required(self.required)
            .disabled(self.disabled)
    }
}

/// What committing the field's text does: the text becomes what `rules` make of it, and the
/// application hears the number if it is not the one it last heard from a commit.
fn commit<T: 'static>(
    hosted: &mut Hosted<T>,
    id: &str,
    rules: &Rules,
    on_change: &Option<Changed<T>>,
    on_commit: &Option<Committed<T>>,
) -> Option<f64> {
    let text = hosted.ui.input_text(id);
    let tidy = rules.commit(&text)?;
    if tidy != text {
        hosted.ui.set_input_text(id, &tidy);
        let value = parse(&tidy);
        if let Some(on_change) = on_change {
            on_change(&mut hosted.app, value);
        }
    }
    settle(hosted, id, &tidy, on_commit)
}

/// Tells the application a number that differs from the last one committed.
fn settle<T: 'static>(
    hosted: &mut Hosted<T>,
    id: &str,
    text: &str,
    on_commit: &Option<Committed<T>>,
) -> Option<f64> {
    let value = parse(text)?;
    let key = format!("{id}-committed");
    if hosted.ui.choice(&key) != Some(text)
        && let Some(on_commit) = on_commit
    {
        on_commit(&mut hosted.app, value);
    }
    hosted.ui.set_choice(&key, text);
    Some(value)
}

impl<T: 'static> NumberInput<T> {
    pub fn show(self, hosted: &Hosted<T>) -> El {
        let NumberInput {
            id,
            name,
            rules,
            placeholder,
            default_value,
            read_only,
            invalid,
            required,
            disabled,
            on_change,
            on_commit,
            on_submit,
        } = self;
        let idle = disabled || read_only;
        let text = hosted.ui.input_text(&id);
        let not_a_number = !text.trim().is_empty() && parse(&text).is_none();

        // A step: from the text, clamped, as a commit.
        let step: Step<T> = {
            let (id, on_change, on_commit) = (id.clone(), on_change.clone(), on_commit.clone());
            Rc::new(move |hosted: &mut Hosted<T>, direction: f64| {
                let text = hosted.ui.input_text(&id);
                let next = rules.step(&text, direction);
                if next != text {
                    hosted.ui.set_input_text(&id, &next);
                    if let Some(on_change) = &on_change {
                        on_change(&mut hosted.app, parse(&next));
                    }
                }
                settle(hosted, &id, &next, &on_commit);
            })
        };
        let hook: KeyHook<T> = {
            let (id, step) = (id.clone(), step.clone());
            let (on_change, on_commit, on_submit) =
                (on_change.clone(), on_commit.clone(), on_submit);
            Rc::new(
                move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx, up: bool| {
                    if idle {
                        return false;
                    }
                    let plain = !ev.mods.shift && !ev.mods.ctrl && !ev.mods.alt && !ev.mods.logo;
                    match ev.key {
                        Key::Up | Key::Down if !up => {
                            step(hosted, if ev.key == Key::Up { 1.0 } else { -1.0 });
                            true
                        }
                        // Enter commits on its release, once for each hold, as a button acts.
                        Key::Enter if plain && !up => {
                            if !tx.key_repeat() {
                                hosted.ui.arm_input_submit(&id);
                            }
                            true
                        }
                        Key::Enter if plain && hosted.ui.take_input_submit(&id) => {
                            let value = commit(hosted, &id, &rules, &on_change, &on_commit);
                            if let Some(on_submit) = &on_submit {
                                on_submit(&mut hosted.app, value);
                            }
                            true
                        }
                        _ => false,
                    }
                },
            )
        };

        let (on_type, mark) = (on_change, not_a_number || invalid);
        let mut input = Input::new(
            id.clone(),
            name,
            move |app: &mut T, text: String| {
                if let Some(on_type) = &on_type {
                    on_type(app, parse(&text));
                }
            },
            |_: &mut T| {},
        )
        .invalid(mark)
        .required(required)
        .disabled(disabled || read_only);
        if not_a_number {
            input = input.described(Some("Enter a number".into()));
        }
        if let Some(placeholder) = placeholder {
            input = input.placeholder(placeholder);
        }
        if let Some(value) = default_value {
            input = input.default_value(rules.text_of(value));
        }
        if !idle {
            for (suffix, icon, label, direction) in [
                ("decrease", Icon::Minus, "Decrease", -1.0),
                ("increase", Icon::Plus, "Increase", 1.0),
            ] {
                let can = rules.can_step(&text, direction > 0.0);
                let step = step.clone();
                let act: Press<T> = Rc::new(move |hosted: &mut Hosted<T>| step(hosted, direction));
                input = input.trailing(
                    Button::icon(format!("{id}-{suffix}"), icon, label, ())
                        .variant(ButtonVariant::Ghost)
                        .in_field()
                        .tab_stop(false)
                        .disabled(!can)
                        .show_hosted(hosted, act),
                );
            }
        }
        input.show_with(hosted, Some(hook))
    }
}
