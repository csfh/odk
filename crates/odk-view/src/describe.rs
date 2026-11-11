//! A component's [`Accessible`] handed to Rune's accessibility tree (ADR 0036, ADR 0089).
//!
//! A control describes itself once, with [`odk_a11y::Describe`], and draws that description:
//! its role, its name, whether it is off or wrong. Nothing here is toolkit policy, so a
//! component never names a Rune role string of its own.
//!
//! Rune has no builder for `busy` or `required` at the pinned revision, so those two fields of
//! an [`Accessible`] reach the checklist's tests but not the screen reader. They are a known gap
//! of the port, to close when Rune grows the property.

use odk_a11y::{Accessible, Role};
use rune::El;

/// Rune's role string for `role`: WAI-ARIA's name, which Rune maps to its own roles.
pub(crate) fn role_name(role: Role) -> &'static str {
    match role {
        Role::Alert => "alert",
        Role::Button => "button",
        Role::Navigation => "navigation",
        Role::Link | Role::CurrentPage => "link",
        Role::Keys => "code",
        Role::Progress => "progressbar",
        Role::Tab => "tab",
        Role::TabList => "tablist",
        Role::TabPanel => "tabpanel",
        Role::RadioGroup => "radiogroup",
        Role::Radio => "radio",
        Role::Group => "group",
        Role::ToggleButton => "toggle",
        Role::Text => "text",
        Role::Heading => "heading",
        Role::Menu => "menu",
        Role::MenuItem => "menuitem",
        Role::MenuItemCheckbox => "menuitemcheckbox",
        Role::MenuItemRadio => "menuitemradio",
        Role::Separator => "separator",
        Role::Dialog => "dialog",
        Role::Tooltip => "tooltip",
        Role::Status => "status",
        Role::Checkbox => "checkbox",
        Role::Switch => "switch",
        Role::TextInput => "textbox",
        Role::Combobox => "combobox",
        Role::Listbox => "listbox",
        Role::Option => "option",
        Role::Table => "table",
        Role::Row => "row",
        Role::Cell => "cell",
        Role::ColumnHeader => "columnheader",
        Role::Image => "image",
    }
}

/// Whether `role` is on or off, which Rune calls checked (`aria-checked` and `aria-pressed`).
fn has_checked_state(role: Role) -> bool {
    matches!(
        role,
        Role::Checkbox
            | Role::Switch
            | Role::Radio
            | Role::ToggleButton
            | Role::MenuItemCheckbox
            | Role::MenuItemRadio
    )
}

/// Whether `role` is an item of a set that is chosen, which Rune calls selected. Rune takes it
/// through `focus_selected`, which also says which item Tab visits in a roving group.
fn has_selected_state(role: Role) -> bool {
    matches!(role, Role::Tab | Role::Option | Role::Row | Role::Cell)
}

/// `element` with `accessible`'s role, name and state. A text input's own text is Rune's to
/// report, so its value is not repeated.
pub(crate) fn apply(element: El, accessible: &Accessible) -> El {
    let mut element = element
        .role(role_name(accessible.role))
        .label(accessible.name.clone())
        .disabled(accessible.disabled)
        .invalid(accessible.invalid)
        .read_only(accessible.read_only);
    if let Some(on) = accessible.selected
        && has_checked_state(accessible.role)
    {
        element = element.checked(on);
    }
    if let Some(on) = accessible.selected
        && has_selected_state(accessible.role)
    {
        element = element.focus_selected(on);
    }
    if let Some(level) = accessible.level {
        element = element.level(u32::from(level));
    }
    if accessible.mixed {
        element = element.checked_mixed();
    }
    if let Some(description) = &accessible.description {
        element = element.description(description.clone());
    }
    if accessible.role != Role::TextInput
        && let Some(value) = &accessible.value
    {
        element = element.a11y_value(value.clone());
    }
    element
}
