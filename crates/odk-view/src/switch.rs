//! A switch on the Rune host: a track and a thumb that says on or off, with a label beside it.

use std::rc::Rc;

use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, focus_ring};
use crate::toggle::{Change, Look, Toggle, labelled};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{States, SwitchGeometry};
use odk_theme::foundation::Density;
use rune::{El, el};

/// An on/off control. Tab reaches it and Space toggles it on its release. The thumb slides
/// between its two places (Rune's own spring, which lands at once when the user asks for
/// reduced motion), and the track's fill and the thumb's place say which it is, so state never
/// rests on colour alone.
///
/// Uncontrolled by default, starting from `default_checked` and keeping its own state in the host's
/// store; pass `checked` to control it and follow `on_change`, which hears each change once.
/// Space or Enter toggles it, on release.
pub struct Switch<T> {
    id: String,
    label: String,
    checked: Option<bool>,
    default_checked: bool,
    disabled: bool,
    density: Option<Density>,
    on_change: Option<Change<T>>,
}

impl<T: 'static> Switch<T> {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked: None,
            default_checked: false,
            disabled: false,
            density: None,
            on_change: None,
        }
    }

    /// Whether it is on, making the switch controlled.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// What it starts as, while it keeps its own state.
    pub fn default_checked(mut self, checked: bool) -> Self {
        self.default_checked = checked;
        self
    }

    /// Hears the state a press made, once for each press.
    pub fn on_change(mut self, handler: impl Fn(&mut T, bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The switch's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let on = self
            .checked
            .unwrap_or_else(|| hosted.ui.toggle_value(&self.id, self.default_checked));
        let accessible = self.accessible_at(on);
        let paint = style::paint(hosted.theme());
        let states = States::NONE.disabled(self.disabled).checked(on);
        let s = paint
            .tokens
            .switch(paint.tokens.density(self.density), states);
        let g = s.geometry;
        let (left, top) = thumb_origin(&g);
        let thumb = el()
            .absolute(left, top)
            .size(g.thumb, g.thumb)
            .radius(g.thumb_shape.corner_radius(g.thumb, g.thumb))
            .fill(ink(s.colours.default.foreground))
            // Rune springs a change of offset, and lands it at once under reduced motion.
            .offset(if on { g.travel } else { 0.0 }, 0.0)
            .a11y_hidden();
        let mut track = el()
            .size(g.width, g.height)
            .radius(g.track.corner_radius(g.width, g.height))
            .fill(ink(s.colours.default.background))
            .outline(
                g.border_width,
                -g.border_width,
                ink(s.colours.default.border),
            )
            .child(thumb)
            .a11y_hidden();
        if s.responds {
            track = track
                .hover(ink(s.colours.hover.background))
                .press(ink(s.colours.pressed.background));
        }
        let track = focus_ring(track, &s.focus);

        let (id, controlled, default) = (self.id.clone(), self.checked, self.default_checked);
        let change = self.on_change;
        let toggle: Toggle<T> = Rc::new(move |hosted: &mut Hosted<T>| {
            let next = !controlled.unwrap_or_else(|| hosted.ui.toggle_value(&id, default));
            if controlled.is_none() {
                hosted.ui.set_toggle(&id, next);
            }
            if let Some(change) = &change {
                change(&mut hosted.app, next);
            }
        });
        let look = Look {
            colour: s.label,
            role: g.label,
            gap: g.gap,
            mark: None,
            enter: true,
            stop: None,
        };
        labelled(
            &paint,
            &accessible,
            &self.id,
            self.label,
            track,
            look,
            toggle,
        )
    }
}

impl<T> Switch<T> {
    /// How it describes itself while it is `on`.
    fn accessible_at(&self, on: bool) -> Accessible {
        Accessible::new(Role::Switch, self.label.clone())
            .selected(on)
            .disabled(self.disabled)
    }
}

impl<T> Describe for Switch<T> {
    fn accessible(&self) -> Accessible {
        self.accessible_at(self.checked.unwrap_or(self.default_checked))
    }
}

/// Where the thumb rests when off, from the track's outer edge: the inset (the border and the
/// thumb gap). The track's own edge is drawn inside it, so the thumb sits on the box itself.
fn thumb_origin(g: &SwitchGeometry) -> (f32, f32) {
    (g.inset, g.inset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use odk_theme::component::States;
    use odk_theme::foundation::Density;

    /// The thumb travels inside the track: off at its inset, and on with its right edge the
    /// inset inside the track's right edge.
    #[test]
    fn the_thumb_travels_inside_the_track() {
        let theme = odk_theme::ThemeSnapshot::emergency();
        let paint = crate::style::paint(&theme);
        for density in Density::ALL {
            let g = paint.tokens.switch(density, States::NONE).geometry;
            let (left, top) = thumb_origin(&g);
            assert_eq!(left, g.inset);
            assert_eq!(top + g.thumb + g.inset, g.height, "{density:?}");
            assert_eq!(left + g.travel + g.thumb + g.inset, g.width, "{density:?}");
        }
    }
}
