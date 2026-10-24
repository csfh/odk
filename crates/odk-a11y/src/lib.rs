//! The accessibility hook of ADR 0036. Every ODK component describes itself with an
//! [`Accessible`]: its role, its accessible name and its state. The pinned GPUI has no
//! accessibility tree, so nothing reads these yet; when one exists, `odk-ui` hands them to it
//! and components do not change. No GPUI here, so this builds and tests without a window.

/// What a component is, in the vocabulary screen readers use (WAI-ARIA roles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Alert,
    Button,
    /// A trail of links, such as a breadcrumb.
    Navigation,
    Link,
    /// The current page in a navigation trail.
    CurrentPage,
    /// A keyboard key or chord.
    Keys,
    /// A progress indicator: determinate with a value (a progress bar), indeterminate without
    /// one (a spinner, a skeleton), as WAI-ARIA's `progressbar` is.
    Progress,
    Tab,
    TabList,
    TabPanel,
    /// A row of options of which exactly one is on.
    RadioGroup,
    Radio,
    /// A group of toggles that can be on together.
    Group,
    ToggleButton,
    Text,
    Heading,
    Menu,
    MenuItem,
    /// A menu item with a check mark.
    MenuItemCheckbox,
    /// A menu item that is one of a set, of which one is chosen.
    MenuItemRadio,
    Separator,
    /// A modal panel, such as a sheet.
    Dialog,
    /// A panel shown while a trigger is hovered or focused.
    Tooltip,
    /// A message that appears without taking focus, such as a toast.
    Status,
    /// A box that is checked, unchecked or mixed.
    Checkbox,
    /// An on/off switch.
    Switch,
    /// A field the user types text into (WAI-ARIA `textbox`).
    TextInput,
    /// The button of a select that opens a list of options and shows the chosen one (WAI-ARIA
    /// `combobox`, select-only).
    Combobox,
    /// The list a select opens.
    Listbox,
    /// One choice in a list.
    Option,
}

/// How a component reports itself to assistive technology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accessible {
    pub role: Role,
    pub name: String,
    /// Selected or checked, for tabs, radios and toggles.
    pub selected: Option<bool>,
    pub disabled: bool,
    pub busy: bool,
    /// A heading's level, 1 being the most important.
    pub level: Option<u8>,
    /// A checkbox that is neither checked nor unchecked, standing for a mixed group.
    pub mixed: bool,
    /// What a text input holds. `None` for a masked one, which must not give its text away.
    pub value: Option<String>,
    /// A text input that takes several lines.
    pub multiline: bool,
    /// A text input that can be selected and copied but not changed.
    pub read_only: bool,
    /// Its value is wrong (WAI-ARIA `aria-invalid`).
    pub invalid: bool,
    /// It must be filled in or chosen before a form is sent (`aria-required`).
    pub required: bool,
    /// What a form says about it besides its name: its description and its message, such as
    /// the error that makes it invalid (`aria-describedby`).
    pub description: Option<String>,
}

impl Accessible {
    pub fn new(role: Role, name: impl Into<String>) -> Self {
        Self {
            role,
            name: name.into(),
            selected: None,
            disabled: false,
            busy: false,
            level: None,
            mixed: false,
            value: None,
            multiline: false,
            read_only: false,
            invalid: false,
            required: false,
            description: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn level(mut self, level: u8) -> Self {
        self.level = Some(level);
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

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Implemented by every component, so applications and tests can ask what it reports.
pub trait Describe {
    fn accessible(&self) -> Accessible;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_description() {
        let a = Accessible::new(Role::Tab, "General").selected(true);
        assert_eq!(a.role, Role::Tab);
        assert_eq!(a.selected, Some(true));
        assert!(!a.disabled && !a.busy);
    }
}
