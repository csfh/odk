use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{States, SwitchGeometry};
use odk_theme::foundation::{Density, MotionDuration};

use crate::components;
use crate::focus::{child_id, focus_handle, part_name};
use crate::interaction::{Pointer, focus_ring, pointer_states, progress};
use crate::theme::colour;
use crate::toggle::{ChangeHandler, Label, labelled, toggle_state};

/// The GPUI group a switch's track gives its thumb, so the thumb follows the track's pointer
/// states.
const TRACK_GROUP: &str = "odk-switch-track";

/// An on/off switch with a label beside it.
///
/// Tab reaches the track; Space or Enter toggles it. A click on the track or the label toggles
/// it once, and a click on the label moves focus to the track. The thumb slides over the
/// foundation's `normal` on its curve, and at once with reduced motion; a toggle reversed partway
/// carries on from where the thumb is. The track is a pill as high as its density's icon (14, 16
/// or 18) with a border that is always there, so nothing around it moves; on and off differ by
/// the track's fill and the thumb's side, never by colour alone.
///
/// Uncontrolled by default, starting from `default_checked`; pass `checked` to control it and
/// follow `on_change`, which hears each change once.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    label: SharedString,
    checked: Option<bool>,
    default_checked: bool,
    disabled: bool,
    density: Option<Density>,
    pub(crate) description: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked: None,
            default_checked: false,
            disabled: false,
            density: None,
            description: None,
            on_change: None,
        }
    }

    /// Whether it is on, making the switch controlled.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn default_checked(mut self, checked: bool) -> Self {
        self.default_checked = checked;
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

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Describe for Switch {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::Switch, self.label.to_string())
            .selected(self.checked.unwrap_or(self.default_checked))
            .disabled(self.disabled);
        a.description = self.description.as_ref().map(|d| d.to_string());
        a
    }
}

/// Where the thumb sits, from the inside of the track's left border, `amount` of the way from
/// off to on.
fn thumb_left(g: &SwitchGeometry, amount: f32) -> f32 {
    g.inset - g.border_width + g.travel * amount
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let (on, change) = toggle_state(
            &self.id,
            self.checked,
            self.default_checked,
            |on| !on,
            self.on_change.clone(),
            window,
            cx,
        );
        let usable = !self.disabled;
        let focus = focus_handle(child_id(&self.id, "focus"), usable, window, cx);
        let focused = usable && focus.is_focused(window);
        let states = States::NONE
            .focused(focused)
            .disabled(self.disabled)
            .checked(on);
        let s = c.switch(c.density(self.density), states);
        let g = s.geometry;
        // The thumb's place, eased between off and on over `normal`. It starts where the switch is, without
        // sliding there.
        let amount = progress(
            child_id(&self.id, "thumb"),
            on,
            MotionDuration::Normal,
            window,
            cx,
        );
        #[cfg(test)]
        drawn::record(&self.id, drawn::Drawn { style: s, amount });

        let mut thumb = div()
            .id(child_id(&self.id, "thumb-face"))
            .debug_selector(|| part_name(&self.id, "thumb"))
            .absolute()
            .top(px(g.inset - g.border_width))
            .left(px(thumb_left(&g, amount)))
            .size(px(g.thumb))
            .rounded(px(g.thumb_shape.corner_radius(g.thumb, g.thumb)))
            .bg(colour(s.colours.default.foreground));
        if s.responds {
            let (hover, pressed) = (s.colours.hover.foreground, s.colours.pressed.foreground);
            thumb = thumb
                .group_hover(TRACK_GROUP, move |st| st.bg(colour(hover)))
                .group_active(TRACK_GROUP, move |st| st.bg(colour(pressed)));
        }
        // A box of fixed size, which flex layout would otherwise shrink.
        let mut track = div()
            .id(child_id(&self.id, "track"))
            .debug_selector(|| part_name(&self.id, "track"))
            .group(TRACK_GROUP)
            .relative()
            .flex_shrink_0()
            .w(px(g.width))
            .h(px(g.height))
            .rounded(px(g.track.corner_radius(g.width, g.height)))
            .border(px(g.border_width))
            .child(thumb);
        track = pointer_states(track, &s.colours, s.responds, &Pointer::Own);
        if s.focus_visible {
            // The ring is a pill around the pill.
            let out = s.focus.offset + s.focus.width;
            let radius = (g.height + 2.0 * out) / 2.0;
            track = track.child(focus_ring(&s.focus, radius, g.border_width));
        }
        if usable {
            let on_track = change.clone();
            track = track
                .track_focus(&focus)
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    on_track(window, cx);
                    // The row toggles on a click too; this one is handled.
                    cx.stop_propagation();
                });
        }
        let look = Label {
            colour: s.label,
            role: g.label,
            gap: g.gap,
            stretch: false,
            mark: None,
        };
        labelled(
            &self.id,
            self.label,
            track.into_any_element(),
            look,
            usable,
            focus,
            change,
        )
    }
}

/// What each switch drew on its last frame, for tests: its style and how far its thumb was.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use gpui::ElementId;
    use odk_theme::component::SwitchStyle;

    use crate::focus::id_path;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) struct Drawn {
        pub style: SwitchStyle,
        pub amount: f32,
    }

    thread_local! {
        static DRAWN: RefCell<HashMap<String, Drawn>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(id: &ElementId, drawn: Drawn) {
        DRAWN.with(|d| d.borrow_mut().insert(id_path(id), drawn));
    }

    pub(crate) fn last(id: &str) -> Drawn {
        DRAWN.with(|d| {
            *d.borrow()
                .get(id)
                .unwrap_or_else(|| panic!("no switch {id:?} has been drawn"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumb_travels_inside_the_track() {
        crate::theme::install_test_theme();
        for d in Density::ALL {
            let g = components().switch(d, States::NONE).geometry;
            assert_eq!(thumb_left(&g, 0.0), g.inset - g.border_width);
            // At the end, the thumb's right edge is the inset inside the track's right edge.
            assert_eq!(
                g.border_width + thumb_left(&g, 1.0) + g.thumb + g.inset,
                g.width
            );
        }
    }
}
