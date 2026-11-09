//! A button on the Rune host. Enter and Space activate on release, once per hold.

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::store::Activator;
use crate::style::{self, focus_ring, text_style};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ButtonLayout, ButtonVariant, States};
use odk_theme::foundation::Density;
use rune::{El, Key, KeyEvent, Tx, row, text};

/// Pressed on pointer release inside the control, or on Enter or Space's release.
pub struct Button<F> {
    id: String,
    label: String,
    disabled: bool,
    busy: bool,
    variant: ButtonVariant,
    density: Density,
    on_press: F,
}

impl<F> Button<F> {
    pub fn new(id: impl Into<String>, label: impl Into<String>, on_press: F) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            busy: false,
            variant: ButtonVariant::Primary,
            density: Density::Default,
            on_press,
        }
    }

    /// The variant the component layer styles: primary unless asked.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Busy keeps the button's place and ignores activation.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }
}

impl<F> Describe for Button<F> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Button, self.label.clone())
            .busy(self.busy)
            .disabled(self.disabled)
    }
}

impl<F> Button<F> {
    pub fn show<T>(self, hosted: &Hosted<T>) -> El
    where
        T: 'static,
        F: Fn(&mut T) + Clone + 'static,
    {
        let accessible = self.accessible();
        let Button {
            id,
            label,
            disabled,
            busy,
            variant,
            density,
            on_press,
        } = self;
        let paint = style::paint(hosted.theme());
        let states = States::NONE.disabled(disabled).loading(busy);
        let style = paint
            .tokens
            .button(variant, density, ButtonLayout::Text, states);
        let responds = style.responds;
        let press = on_press.clone();
        let release = on_press;
        let key_id = id.clone();
        let up_id = id.clone();
        focus_ring(apply(row().key(id), &accessible), &style.focus)
            // A disabled control leaves the Tab order (`docs/spec/odk-interaction.md`, 21). `apply`'s
            // `disabled` already takes it out in Rune; this says the rule where the button is drawn.
            .focusable(!disabled)
            .h(style.geometry.height)
            .px(style.geometry.padding_x)
            .center()
            .radius(style.geometry.radius)
            .fill(ink(style.colours.default.background))
            .hover(ink(style.colours.hover.background))
            .press(ink(style.colours.pressed.background))
            .on_press(move |hosted: &mut Hosted<T>| {
                if responds {
                    press(&mut hosted.app);
                }
            })
            .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                let Some(key) = activator(ev) else {
                    return false;
                };
                // Rune activates on Space's press and ignores Enter. Take both keys here.
                if responds && !tx.key_repeat() {
                    let slot = hosted.ui.button_mut(&key_id);
                    if slot.armed.is_none() {
                        slot.armed = Some(key);
                    }
                }
                true
            })
            .on_key_up(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                let Some(key) = activator(ev) else {
                    return false;
                };
                let slot = hosted.ui.button_mut(&up_id);
                if slot.armed != Some(key) {
                    return false;
                }
                slot.armed = None;
                if responds {
                    release(&mut hosted.app);
                }
                true
            })
            .child(
                text(label)
                    .font(text_style(paint.button))
                    .color(ink(style.colours.default.foreground))
                    .a11y_hidden(),
            )
    }
}

pub(crate) fn activator(ev: &KeyEvent) -> Option<Activator> {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return None;
    }
    match &ev.key {
        Key::Enter => Some(Activator::Enter),
        Key::Text(text) if text == " " => Some(Activator::Space),
        _ => None,
    }
}
