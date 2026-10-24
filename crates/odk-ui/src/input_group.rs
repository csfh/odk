//! An `Input` with text and a button around it, in one box (ADR 0038,
//! `docs/spec/odk-text-input.md`, "InputGroup").

use gpui::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, px, relative,
};
use odk_a11y::{Accessible, Describe};
use odk_theme::component::States;
use odk_theme::foundation::Density;

use crate::button::Size;
use crate::focus::{child_id, focus_handle, move_focus, part_name};
use crate::input::field_box;
use crate::layout::single_line;
use crate::theme::colour;
use crate::{Button, Input, components};

/// How much of the box one addon's text may take, so that long text leaves the field room
/// (ODK's own).
const ADDON_SHARE: f32 = 0.25;

/// A field with text before or after it and one button at its end, in one box.
///
/// Tab reaches the field and then the button; everything the field does with the keyboard it
/// still does (`Input` lists it), Enter calls `on_submit`, and Enter or Space press the button.
/// A press on the text around the field puts focus in it. The box, with its border and focus
/// ring, belongs to the group; it is as high as an `Input` of its density (28, 34 or 40),
/// however tall its button, and keeps its size whatever is typed or focused.
///
/// Uncontrolled by default, starting from `default_value`; pass `value` to control it and
/// follow `on_change`. The button is the application's, and takes the group's `disabled` and
/// density; it is drawn inside the box, lower than it.
#[derive(IntoElement)]
pub struct InputGroup {
    id: ElementId,
    input: Input,
    size: Size,
    density: Option<Density>,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    prefix: Option<SharedString>,
    suffix: Option<SharedString>,
    action: Option<Button>,
    /// The focus handle to give the field in place of the group's own, for a `FormField`
    /// whose label moves focus to it.
    pub(crate) focus: Option<FocusHandle>,
}

impl InputGroup {
    /// A group with no visible label of its own: `name` is what its field is called to
    /// assistive technology.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        let id = id.into();
        let mut input = Input::new(child_id(&id, "input"), name);
        input.bare = true;
        Self {
            id,
            input,
            size: Size::default(),
            density: None,
            disabled: false,
            read_only: false,
            invalid: false,
            prefix: None,
            suffix: None,
            action: None,
            focus: None,
        }
    }

    /// Text before the field, such as `https://` or a currency.
    pub fn prefix(mut self, text: impl Into<SharedString>) -> Self {
        self.prefix = Some(text.into());
        self
    }

    /// Text after the field, such as a unit or a domain.
    pub fn suffix(mut self, text: impl Into<SharedString>) -> Self {
        self.suffix = Some(text.into());
        self
    }

    /// A button at the end of the box.
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button);
        self
    }

    /// The text, making the field controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.input = self.input.value(value);
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.input = self.input.default_value(value);
        self
    }

    /// Shown, muted, while the field is empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    /// A step from the density the group would otherwise take, as for an `Input`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self.input = self.input.size(size);
        self
    }

    /// The group's own density, whatever the application's and whatever its `size`.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self.input = self.input.density(density);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.input = self.input.disabled(disabled);
        self
    }

    /// Selectable and copyable, not editable.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self.input = self.input.read_only(read_only);
        self
    }

    /// For a password: dots instead of text, and nothing copied or reported.
    pub fn masked(mut self, masked: bool) -> Self {
        self.input = self.input.masked(masked);
        self
    }

    /// Marks the value as wrong, as for an `Input`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self.input = self.input.invalid(invalid);
        self
    }

    /// Reports that the field must be filled in.
    pub fn required(mut self, required: bool) -> Self {
        self.input = self.input.required(required);
        self
    }

    /// Called with the text after each edit; not while an IME composition is running, but
    /// when it ends.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_change(handler);
        self
    }

    /// Called with the text when Enter is pressed in the field.
    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.input = self.input.on_submit(handler);
        self
    }

    pub(crate) fn describe(mut self, description: Option<SharedString>) -> Self {
        self.input.description = description;
        self
    }
}

impl Describe for InputGroup {
    fn accessible(&self) -> Accessible {
        self.input.accessible()
    }
}

impl RenderOnce for InputGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let usable = !self.disabled;
        let focus = match &self.focus {
            Some(handle) => handle.clone().tab_stop(usable),
            None => focus_handle(child_id(&self.id, "field-focus"), usable, window, cx),
        };
        let focused = usable && focus.is_focused(window);
        let density = self.input.density_in(&c);
        let states = States::NONE
            .focused(focused)
            .disabled(self.disabled)
            .read_only(self.read_only)
            .invalid(self.invalid);
        let s = c.field(density, states);

        // A press on the text around the field puts focus in it, as a label would.
        let addon = |name: &str, text: SharedString| {
            let focus = focus.clone();
            single_line(
                div()
                    .debug_selector(|| part_name(&self.id, name))
                    .min_w_0()
                    .max_w(relative(ADDON_SHARE))
                    .text_color(colour(s.addon))
                    .child(text),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                move_focus(&focus, window);
            })
        };

        let mut input = self.input;
        input.focus = Some(focus.clone());
        let has_action = self.action.is_some();
        let mut group = div()
            .id(child_id(&self.id, "box"))
            .debug_selector(|| part_name(&self.id, "box"))
            .w_full()
            // Addons sit close to the text they belong with: `space.1`, not an icon's gap.
            .gap(px(c.foundation().spacing.s1))
            .children(self.prefix.map(|text| addon("prefix", text)))
            // The field keeps at least an addon's share, so long addons give way first.
            .child(div().flex_1().min_w(relative(ADDON_SHARE)).child(input))
            .children(self.suffix.map(|text| addon("suffix", text)));
        if let Some(button) = self.action {
            group = group.child(
                div()
                    .flex_shrink_0()
                    .child(button.density(density).disabled(self.disabled).in_field()),
            );
        }
        let inset = has_action.then(|| c.foundation().geometry.thumb_gap);
        field_box(group, &s, inset)
    }
}
