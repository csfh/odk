use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, KeyboardButton, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{Check, CheckboxStyle, ComponentTokens, States};
use odk_theme::foundation::Density;

use crate::components;
use crate::focus::{child_id, focus_handle, part_name};
use crate::interaction::{Pointer, focus_ring, pointer_states};
use crate::theme::mono;
use crate::toggle::{ChangeHandler, Label, labelled, toggle_state};

/// The marks of a ticked and a mixed box. ODK has no icon set yet, so they are characters.
const TICK: &str = "✓";
const DASH: &str = "–";

/// A box that is checked or not, with a label beside it.
///
/// Tab reaches the box and Space toggles it; Enter does not, as for a native checkbox. A click
/// on the box or on the label toggles it once, and a click on the label moves focus to the
/// box. The box is a square of its density's icon size (14, 16 or 18) with the application's
/// corners and a border that is always there, so checking, focusing or pressing it moves
/// nothing; its focus ring lies outside it, and a long label wraps beside it.
///
/// Uncontrolled by default, starting from `default_checked`; pass `checked` to control it and
/// follow `on_change`, which hears each change once. `indeterminate` shows a dash for a mixed
/// group; a press then checks it. A ticked box shows `✓` and a mixed one `–`, never only a fill.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: SharedString,
    checked: Option<bool>,
    default_checked: bool,
    indeterminate: bool,
    disabled: bool,
    invalid: bool,
    required: bool,
    density: Option<Density>,
    pub(crate) description: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked: None,
            default_checked: false,
            indeterminate: false,
            disabled: false,
            invalid: false,
            required: false,
            density: None,
            description: None,
            on_change: None,
        }
    }

    /// Whether it is checked, making the checkbox controlled.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn default_checked(mut self, checked: bool) -> Self {
        self.default_checked = checked;
        self
    }

    /// Shows a dash, for a box that stands for a group that is partly checked.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the choice as wrong: the danger edge and ring, and a `!` after the label, so it
    /// never shows by colour alone. Say what is wrong in a `FormField`'s error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that the box must be checked, such as an agreement.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// The box's own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// What the box draws with, given whether it is checked and has focus.
    fn style(&self, c: &ComponentTokens, checked: bool, focused: bool) -> CheckboxStyle {
        let states = States::NONE
            .focused(focused)
            .disabled(self.disabled)
            .checked(checked)
            .indeterminate(self.indeterminate)
            .invalid(self.invalid);
        c.checkbox(c.density(self.density), states)
    }
}

impl Describe for Checkbox {
    fn accessible(&self) -> Accessible {
        let checked = self.checked.unwrap_or(self.default_checked);
        let mut a = Accessible::new(Role::Checkbox, self.label.to_string())
            .selected(checked && !self.indeterminate)
            .mixed(self.indeterminate)
            .disabled(self.disabled)
            .invalid(self.invalid)
            .required(self.required);
        a.description = self.description.as_ref().map(|d| d.to_string());
        a
    }
}

/// What a press changes a checkbox to: on, from off or from mixed; off, from on.
fn pressed(checked: bool, indeterminate: bool) -> bool {
    indeterminate || !checked
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let indeterminate = self.indeterminate;
        let (checked, change) = toggle_state(
            &self.id,
            self.checked,
            self.default_checked,
            |on| pressed(on, indeterminate),
            self.on_change.clone(),
            window,
            cx,
        );
        let usable = !self.disabled;
        let focus = focus_handle(child_id(&self.id, "focus"), usable, window, cx);
        let focused = usable && focus.is_focused(window);
        let s = self.style(&c, checked, focused);
        let g = s.geometry;
        #[cfg(test)]
        drawn::record(&self.id, s);
        let glyph = match s.check {
            Check::Indeterminate => DASH,
            Check::Checked => TICK,
            Check::Unchecked => "",
        };
        let radius = g.shape.corner_radius(g.size, g.size);

        // A box of fixed size, which flex layout would otherwise shrink. The mark is text in
        // the box's foreground, so it follows the pointer states with the fill.
        let mut check = div()
            .id(child_id(&self.id, "box"))
            .debug_selector(|| part_name(&self.id, "box"))
            .relative()
            .flex_shrink_0()
            .size(px(g.size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius))
            .border(px(g.border_width))
            .font_family(mono())
            .text_size(px(g.mark))
            .line_height(px(g.mark))
            .child(glyph);
        check = pointer_states(check, &s.colours, s.responds, &Pointer::Own);
        if s.focus_visible {
            check = check.child(focus_ring(&s.focus, radius, g.border_width));
        }
        if usable {
            let on_box = change.clone();
            check =
                check
                    .track_focus(&focus)
                    .cursor_pointer()
                    .on_click(move |event, window, cx| {
                        // Space toggles a checkbox; Enter does not (WAI-ARIA).
                        if let ClickEvent::Keyboard(key) = event
                            && key.button == KeyboardButton::Enter
                        {
                            return;
                        }
                        on_box(window, cx);
                        // The row toggles on a click too; this one is handled.
                        cx.stop_propagation();
                    });
        }

        let look = Label {
            colour: s.label,
            role: g.label,
            gap: g.gap,
            stretch: false,
            mark: s.invalid_mark,
        };
        labelled(
            &self.id,
            self.label,
            check.into_any_element(),
            look,
            usable,
            focus,
            change,
        )
    }
}

/// What each check box drew on its last frame, for tests.
#[cfg(test)]
pub(crate) mod drawn {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use gpui::ElementId;
    use odk_theme::component::CheckboxStyle;

    use crate::focus::id_path;

    thread_local! {
        static DRAWN: RefCell<HashMap<String, CheckboxStyle>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn record(id: &ElementId, style: CheckboxStyle) {
        DRAWN.with(|d| d.borrow_mut().insert(id_path(id), style));
    }

    pub(crate) fn last(id: &str) -> CheckboxStyle {
        DRAWN.with(|d| {
            *d.borrow()
                .get(id)
                .unwrap_or_else(|| panic!("no check box {id:?} has been drawn"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::pressed;

    #[test]
    fn a_press_checks_from_off_or_mixed_and_unchecks_from_on() {
        assert!(pressed(false, false));
        assert!(!pressed(true, false));
        assert!(pressed(false, true));
        assert!(pressed(true, true));
    }
}
