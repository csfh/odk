//! A button on the Rune host. Enter and Space activate on release, once per hold.

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Glyph;
use crate::ink::ink;
use crate::store::Activator;
use crate::style::{self, focus_ring, text_style};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ButtonLayout, ButtonVariant, States};
use odk_theme::foundation::Density;
use rune::{El, Key, KeyEvent, Tx, row, text};
use std::rc::Rc;

/// What a press does, with the host: a field's own buttons reach the store through it.
pub(crate) type Press<T> = Rc<dyn Fn(&mut Hosted<T>)>;

/// Pressed on pointer release inside the control, or on Enter or Space's release.
pub struct Button<F> {
    id: String,
    label: String,
    leading: Option<Glyph>,
    trailing: Option<Glyph>,
    name: Option<String>,
    disabled: bool,
    busy: bool,
    variant: ButtonVariant,
    density: Density,
    /// A button inside a field's box: sized by the field's action tokens, and a press on it
    /// leaves focus, caret and selection where they are.
    in_field: bool,
    /// Whether Tab reaches it. A field's clear button is not a stop.
    tab_stop: bool,
    on_press: F,
}

impl<F> Button<F> {
    pub fn new(id: impl Into<String>, label: impl Into<String>, on_press: F) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            leading: None,
            trailing: None,
            name: None,
            disabled: false,
            busy: false,
            variant: ButtonVariant::Primary,
            density: Density::Default,
            in_field: false,
            tab_stop: true,
            on_press,
        }
    }

    /// A button that is an icon alone: a square as wide as it is high. It has no text to be named
    /// by, so it takes its accessible name here. The icon is one of ODK's (`Icon::Close`) or a
    /// character.
    pub fn icon(
        id: impl Into<String>,
        icon: impl Into<Glyph>,
        name: impl Into<String>,
        on_press: F,
    ) -> Self {
        Self::new(id, "", on_press)
            .leading_icon(icon)
            .accessible_name(name)
    }

    /// An icon before the label, at the density's icon size and gap.
    pub fn leading_icon(mut self, icon: impl Into<Glyph>) -> Self {
        self.leading = Some(icon.into());
        self
    }

    /// An icon after the label, such as `Icon::ArrowRight` for an action that goes on.
    pub fn trailing_icon(mut self, icon: impl Into<Glyph>) -> Self {
        self.trailing = Some(icon.into());
        self
    }

    /// The accessible name, where the label does not give one: an icon-only button's.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// What the button holds: an icon with no label is an icon-only button; otherwise the label
    /// leads.
    fn layout(&self) -> ButtonLayout {
        match (&self.leading, &self.trailing, self.label.is_empty()) {
            (Some(_), None, true) | (None, Some(_), true) => ButtonLayout::IconOnly,
            (Some(_), _, _) => ButtonLayout::LeadingIcon,
            (None, Some(_), _) => ButtonLayout::TrailingIcon,
            (None, None, _) => ButtonLayout::Text,
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

    /// Inside a field's box (`field_action`): as high as the field less its edges.
    pub(crate) fn in_field(mut self) -> Self {
        self.in_field = true;
        self
    }

    /// Whether Tab reaches the button; a pointer still presses it.
    pub(crate) fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
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
        let name = match &self.name {
            Some(name) if self.label.is_empty() => name.clone(),
            _ => self.label.clone(),
        };
        Accessible::new(Role::Button, name)
            .busy(self.busy)
            .disabled(self.disabled)
    }
}

impl<F> Button<F> {
    pub fn show<T>(self, hosted: &Hosted<T>) -> El
    where
        T: 'static,
        F: Fn(&mut T) + 'static,
    {
        let Button {
            id,
            label,
            leading,
            trailing,
            name,
            disabled,
            busy,
            variant,
            density,
            in_field,
            tab_stop,
            on_press,
        } = self;
        let rest = Button {
            id,
            label,
            leading,
            trailing,
            name,
            disabled,
            busy,
            variant,
            density,
            in_field,
            tab_stop,
            on_press: (),
        };
        rest.show_hosted(
            hosted,
            Rc::new(move |hosted: &mut Hosted<T>| on_press(&mut hosted.app)),
        )
    }
}

impl Button<()> {
    /// As [`Button::show`], for a press that needs the host and not only the application.
    pub(crate) fn show_hosted<T: 'static>(self, hosted: &Hosted<T>, on_press: Press<T>) -> El {
        let accessible = self.accessible();
        let layout = self.layout();
        let Button {
            id,
            label,
            leading,
            trailing,
            name: _,
            disabled,
            busy,
            variant,
            density,
            in_field,
            tab_stop,
            on_press: _,
        } = self;
        let paint = style::paint(hosted.theme());
        let states = States::NONE.disabled(disabled).loading(busy);
        let style = if in_field {
            paint.tokens.field_action(variant, density, layout, states)
        } else {
            paint.tokens.button(variant, density, layout, states)
        };
        let responds = style.responds;
        let press = on_press.clone();
        let release = on_press;
        let key_id = id.clone();
        let up_id = id.clone();
        let geometry = style.geometry;
        let ink_of = ink(style.colours.default.foreground);
        let mut button = focus_ring(apply(row().key(id), &accessible), &style.focus)
            // A disabled control leaves the Tab order (`docs/spec/odk-interaction.md`, 21). `apply`'s
            // `disabled` already takes it out in Rune; this says the rule where the button is drawn.
            .focusable(!disabled && tab_stop)
            .h(geometry.height)
            .px(geometry.padding_x)
            .gap(geometry.gap)
            .center()
            .radius(style.geometry.radius)
            .fill(ink(style.colours.default.background))
            .hover(ink(style.colours.hover.background))
            .press(ink(style.colours.pressed.background))
            .on_press(move |hosted: &mut Hosted<T>| {
                if responds {
                    press(hosted);
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
                    release(hosted);
                }
                true
            });
        if in_field {
            button = button.preserve_focus_on_press(true);
        }
        // An icon-only button is a square as wide as it is high.
        if let Some(width) = style.width {
            button = button.w(width);
        }
        if let Some(glyph) = leading {
            button = button.child(glyph.slot(geometry.icon_size, ink_of));
        }
        if layout != ButtonLayout::IconOnly {
            button = button.child(
                text(label)
                    .font(text_style(paint.button))
                    .color(ink_of)
                    .a11y_hidden(),
            );
        }
        if let Some(glyph) = trailing {
            button = button.child(glyph.slot(geometry.icon_size, ink_of));
        }
        button
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
