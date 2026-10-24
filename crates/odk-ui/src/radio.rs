//! A group of radios, of which one is chosen (`docs/spec/odk-ui-components.md`, "Form
//! controls"), on the radio's component tokens: a check box's colours on a circle.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    KeyboardButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::Density;

use crate::components;
use crate::focus::{Axis, child_id, focus_handle, move_focus, part_name, roving_among};
use crate::interaction::{Pointer, focus_ring, pointer_states};
use crate::layout::{h_flex, v_flex};
use crate::theme::colour;
use crate::toggle::{Action, Label, labelled};

type ChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// The GPUI group a radio's circle gives its dot, so the dot follows the circle's pointer
/// states.
const CIRCLE_GROUP: &str = "odk-radio-circle";

struct Item {
    value: SharedString,
    label: SharedString,
    disabled: bool,
}

/// Options of which exactly one is chosen, each a circle with its label beside it.
///
/// The group is one tab stop: Tab reaches the chosen option, or the first enabled one when none
/// is. The arrow keys move to the next or previous enabled option, wrapping, and choose it, as
/// native radios do; Home and End go to the first and last. Space chooses the option that has
/// focus; Enter does not. A click on a circle or a label chooses that option and gives it
/// focus. Disabled options are skipped and cannot be chosen.
///
/// Uncontrolled by default, keeping its own choice from `default_value`; pass `value` to
/// control it and follow `on_change`, which hears each change once. The chosen circle is the
/// accent's fill with a dot, so it reads without its colour, as in a monochrome theme.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    name: SharedString,
    items: Vec<Item>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    disabled: bool,
    invalid: bool,
    required: bool,
    horizontal: bool,
    density: Option<Density>,
    pub(crate) description: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl RadioGroup {
    /// A group named `name` for assistive technology, such as "Theme"; a `FormField` gives it
    /// a visible label.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            items: Vec::new(),
            value: None,
            default_value: None,
            disabled: false,
            invalid: false,
            required: false,
            horizontal: false,
            density: None,
            description: None,
            on_change: None,
        }
    }

    /// Adds an option: `value` is what `on_change` reports and `label` what the user reads.
    pub fn item(mut self, value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.items.push(Item {
            value: value.into(),
            label: label.into(),
            disabled: false,
        });
        self
    }

    /// Disables the option with this value: it is shown but cannot be chosen or focused.
    pub fn disable(mut self, value: impl Into<SharedString>) -> Self {
        let value = value.into();
        for item in self.items.iter_mut().filter(|i| i.value == value) {
            item.disabled = true;
        }
        self
    }

    /// The chosen option's value, making the group controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// The first choice of an uncontrolled group.
    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the choice as wrong: every circle takes the danger edge, and a `!` follows the
    /// first option, so it never shows by colour alone. Say what is wrong in a `FormField`'s
    /// error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that an option must be chosen.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Lays the options out in a row rather than a column. The arrow keys work either way.
    pub fn horizontal(mut self, horizontal: bool) -> Self {
        self.horizontal = horizontal;
        self
    }

    /// The group's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// Called with the value of the option the user chose, when it differs from the chosen one.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// What the group and each option report, given the chosen value.
    pub fn item_accessibles(&self, chosen: Option<&str>) -> Vec<Accessible> {
        self.items
            .iter()
            .map(|i| {
                Accessible::new(Role::Radio, i.label.to_string())
                    .selected(chosen == Some(i.value.as_ref()))
                    .disabled(self.disabled || i.disabled)
            })
            .collect()
    }
}

impl Describe for RadioGroup {
    fn accessible(&self) -> Accessible {
        let mut a = Accessible::new(Role::RadioGroup, self.name.to_string())
            .disabled(self.disabled)
            .invalid(self.invalid)
            .required(self.required);
        a.description = self.description.as_ref().map(|d| d.to_string());
        a
    }
}

/// Where the arrow keys take focus from option `current`: the next enabled option for Down or
/// Right, the previous for Up or Left, wrapping, and the first or last for Home and End.
fn target(key: &str, current: usize, enabled: &[usize]) -> Option<usize> {
    let key = match key {
        "right" => "down",
        "left" => "up",
        key => key,
    };
    roving_among(key, current, enabled, Axis::Vertical)
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let density = c.density(self.density);
        let default = self.default_value.clone();
        let state = window.use_keyed_state(child_id(&self.id, "value"), cx, |_, _| default);
        let chosen = self.value.clone().or_else(|| state.read(cx).clone());
        let controlled = self.value.is_some();

        let enabled: Rc<Vec<usize>> = Rc::new(
            (0..self.items.len())
                .filter(|i| !self.disabled && !self.items[*i].disabled)
                .collect(),
        );
        // The one option in the Tab order: the chosen one if it is enabled, else the first
        // enabled one.
        let stop = enabled
            .iter()
            .copied()
            .find(|i| chosen.as_ref() == Some(&self.items[*i].value))
            .or_else(|| enabled.first().copied());
        let handles: Rc<Vec<FocusHandle>> = Rc::new(
            (0..self.items.len())
                .map(|i| {
                    focus_handle(
                        child_id(&self.id, format!("focus-{i}")),
                        Some(i) == stop,
                        window,
                        cx,
                    )
                })
                .collect(),
        );
        let values: Rc<Vec<SharedString>> =
            Rc::new(self.items.iter().map(|i| i.value.clone()).collect());
        // Chooses option `i`, telling the application once if it changes the choice.
        let choose = {
            let (state, on_change, values, chosen) = (
                state.clone(),
                self.on_change.clone(),
                values.clone(),
                chosen.clone(),
            );
            Rc::new(move |i: usize, window: &mut Window, cx: &mut App| {
                let value = values[i].clone();
                if chosen.as_ref() == Some(&value) {
                    return;
                }
                if !controlled {
                    state.update(cx, |v, cx| {
                        *v = Some(value.clone());
                        cx.notify();
                    });
                }
                if let Some(f) = &on_change {
                    f(&value, window, cx);
                }
            })
        };

        let (keys, key_enabled, key_choose) = (handles.clone(), enabled.clone(), choose.clone());
        let mut group = if self.horizontal {
            h_flex()
                .items_start()
                .flex_wrap()
                .gap(px(c.foundation().layout.form_gap))
        } else {
            v_flex().gap(px(c.radio(density, States::NONE).geometry.gap))
        }
        .id(self.id.clone())
        .debug_selector(|| part_name(&self.id, "group"))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let Some(current) = keys.iter().position(|h| h.is_focused(window)) else {
                return;
            };
            let key = event.keystroke.key.as_str();
            if let Some(next) = target(key, current, &key_enabled) {
                key_choose(next, window, cx);
                move_focus(&keys[next], window);
                cx.stop_propagation();
            }
        });

        for (i, item) in self.items.into_iter().enumerate() {
            let usable = enabled.contains(&i);
            let checked = chosen.as_ref() == Some(&item.value);
            let focused = usable && handles[i].is_focused(window);
            let states = States::NONE
                .focused(focused)
                .disabled(!usable)
                .checked(checked)
                .invalid(self.invalid);
            let s = c.radio(density, states);
            let g = s.geometry;
            #[cfg(test)]
            drawn::record(&part_name(&self.id, &format!("radio-{i}")), s);
            let radius = g.shape.corner_radius(g.size, g.size);

            let mut circle = div()
                .id(child_id(&self.id, format!("radio-{i}")))
                .debug_selector(|| part_name(&self.id, &format!("radio-{i}")))
                .group(CIRCLE_GROUP)
                .relative()
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(g.size))
                .rounded(px(radius))
                .border(px(g.border_width));
            if s.checked {
                let mut dot = div()
                    .id(child_id(&self.id, format!("dot-{i}")))
                    .debug_selector(|| part_name(&self.id, &format!("dot-{i}")))
                    .size(px(g.dot))
                    .rounded(px(g.dot / 2.0))
                    .bg(colour(s.colours.default.foreground));
                if s.responds {
                    let (hover, pressed) =
                        (s.colours.hover.foreground, s.colours.pressed.foreground);
                    dot = dot
                        .group_hover(CIRCLE_GROUP, move |st| st.bg(colour(hover)))
                        .group_active(CIRCLE_GROUP, move |st| st.bg(colour(pressed)));
                }
                circle = circle.child(dot);
            }
            circle = pointer_states(circle, &s.colours, s.responds, &Pointer::Own);
            if s.focus_visible {
                // The ring is a circle around the circle.
                let out = s.focus.offset + s.focus.width;
                circle = circle.child(focus_ring(&s.focus, radius + out, g.border_width));
            }
            let pick: Action = {
                let choose = choose.clone();
                Rc::new(move |window, cx| choose(i, window, cx))
            };
            if usable {
                let on_circle = pick.clone();
                circle = circle.track_focus(&handles[i]).cursor_pointer().on_click(
                    move |event, window, cx| {
                        // Space chooses a radio; Enter is left to the form.
                        if let ClickEvent::Keyboard(key) = event
                            && key.button == KeyboardButton::Enter
                        {
                            return;
                        }
                        on_circle(window, cx);
                        // The row chooses on a click too; this one is handled.
                        cx.stop_propagation();
                    },
                );
            }
            let look = Label {
                colour: s.label,
                role: g.label,
                gap: g.gap,
                // A column of options stretches its rows to the group; a row of them sizes each
                // to its content.
                stretch: !self.horizontal,
                // A wrong choice is marked once, after the first option.
                mark: if i == 0 { s.invalid_mark } else { None },
            };
            group = group.child(labelled(
                &child_id(&self.id, format!("option-{i}")),
                item.label,
                circle.into_any_element(),
                look,
                usable,
                handles[i].clone(),
                pick,
            ));
        }
        // GPUI 0.2.2 measures a label that wraps inside two boxes that size to their content at
        // its unwrapped width, so what follows would overlap it (`docs/checks.md`); only one of
        // the two sizes to its content here. A column of options is as wide as its options,
        // never wider than its column, and stretches its rows to it; a row of them takes its
        // column's width to wrap in, and each option is as wide as its own content.
        if self.horizontal {
            return group.w_full().min_w_0();
        }
        // In a row of other controls it keeps its options' width, since its stretched rows have
        // no least width of their own to keep it from being squeezed.
        group.style().align_self = Some(gpui::AlignSelf::Start);
        group.flex_shrink_0().max_w_full()
    }
}

/// What each radio drew on its last frame, for tests, by the circle's part name.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use odk_theme::component::RadioStyle;

    thread_local! {
        static DRAWN: RefCell<HashMap<String, RadioStyle>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(part: &str, style: RadioStyle) {
        DRAWN.with(|d| d.borrow_mut().insert(part.to_owned(), style));
    }

    pub(crate) fn last(part: &str) -> RadioStyle {
        DRAWN.with(|d| {
            *d.borrow()
                .get(part)
                .unwrap_or_else(|| panic!("no radio {part:?} has been drawn"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_move_both_ways_and_skip_disabled_options() {
        // Options 0, 2 and 3 are enabled; 1 is disabled.
        let enabled = [0, 2, 3];
        assert_eq!(target("down", 0, &enabled), Some(2));
        assert_eq!(target("right", 0, &enabled), Some(2));
        assert_eq!(target("up", 0, &enabled), Some(3), "wraps");
        assert_eq!(target("left", 2, &enabled), Some(0));
        assert_eq!(target("end", 0, &enabled), Some(3));
        assert_eq!(target("home", 3, &enabled), Some(0));
        assert_eq!(target("space", 0, &enabled), None);
    }

    #[test]
    fn reports_the_group_and_each_option() {
        let g = RadioGroup::new("g", "Theme")
            .item("light", "Light")
            .item("dark", "Dark")
            .disable("dark")
            .required(true);
        let a = g.accessible();
        assert_eq!((a.role, a.name.as_str()), (Role::RadioGroup, "Theme"));
        assert!(a.required);
        let items = g.item_accessibles(Some("light"));
        assert_eq!(
            (items[0].role, items[0].selected),
            (Role::Radio, Some(true))
        );
        assert_eq!(items[1].selected, Some(false));
        assert!(items[1].disabled);
    }
}
