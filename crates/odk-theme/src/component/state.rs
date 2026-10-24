//! The states a control can be in, and the one place their precedence is decided
//! (`docs/spec/odk-component-tokens.md`, "States").

use crate::semantic::State;

/// The states a control is in. Each is its own flag, so any combination can be described, and a
/// component reads only the flags it has: a button has no `checked`, a tab no `invalid`. The
/// precedence between them is decided by the methods here and nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct States {
    /// The pointer is over it.
    pub hovered: bool,
    /// The pointer, or a key for a button, is pressing it.
    pub pressed: bool,
    /// It has keyboard focus.
    pub focused: bool,
    pub disabled: bool,
    /// It is busy: it ignores presses but keeps its look and its place in the Tab order.
    pub loading: bool,
    /// It is the chosen one of its siblings: a tab, a toggle that is on, a select's option.
    pub selected: bool,
    /// The keyboard is on it in a list. Not always the selected item.
    pub highlighted: bool,
    /// A check box that is ticked, a switch that is on.
    pub checked: bool,
    /// A check box that is neither ticked nor clear. It wins over `checked`.
    pub indeterminate: bool,
    /// Its value is wrong.
    pub invalid: bool,
    /// Its value can be read and selected but not changed.
    pub read_only: bool,
    /// Its popup is open.
    pub expanded: bool,
}

/// A check box's mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Check {
    Unchecked,
    Checked,
    Indeterminate,
}

impl States {
    /// No flag set: a control at rest.
    pub const NONE: Self = Self {
        hovered: false,
        pressed: false,
        focused: false,
        disabled: false,
        loading: false,
        selected: false,
        highlighted: false,
        checked: false,
        indeterminate: false,
        invalid: false,
        read_only: false,
        expanded: false,
    };

    pub const fn hovered(mut self, on: bool) -> Self {
        self.hovered = on;
        self
    }

    pub const fn pressed(mut self, on: bool) -> Self {
        self.pressed = on;
        self
    }

    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }

    pub const fn loading(mut self, on: bool) -> Self {
        self.loading = on;
        self
    }

    pub const fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub const fn highlighted(mut self, on: bool) -> Self {
        self.highlighted = on;
        self
    }

    pub const fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }

    pub const fn indeterminate(mut self, on: bool) -> Self {
        self.indeterminate = on;
        self
    }

    pub const fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }

    pub const fn read_only(mut self, on: bool) -> Self {
        self.read_only = on;
        self
    }

    pub const fn expanded(mut self, on: bool) -> Self {
        self.expanded = on;
        self
    }

    /// The state whose colours are drawn now, from the pointer flags and the rest.
    pub const fn state(self) -> State {
        let pointer = if self.pressed {
            State::Pressed
        } else if self.hovered {
            State::Hover
        } else {
            State::Default
        };
        self.at(pointer)
    }

    /// The state drawn while the pointer is in `pointer` (default, hover or pressed), given the
    /// other flags:
    ///
    /// 1. disabled wins over everything, and the pointer changes nothing;
    /// 2. a loading control keeps its resting colours, since it ignores presses;
    /// 3. otherwise pressed wins over hover.
    pub const fn at(self, pointer: State) -> State {
        if self.disabled {
            State::Disabled
        } else if self.loading {
            State::Default
        } else {
            pointer
        }
    }

    /// Whether the pointer gets hover and pressed colours and a press acts: neither disabled nor
    /// loading.
    pub const fn responds(self) -> bool {
        !self.disabled && !self.loading
    }

    /// Whether the focus indicator is drawn: focused and not disabled. Focus is drawn over
    /// whatever state the colours are in, and a loading control keeps it.
    pub const fn shows_focus(self) -> bool {
        self.focused && !self.disabled
    }

    /// A check box's mark: indeterminate wins over checked.
    pub const fn check(self) -> Check {
        if self.indeterminate {
            Check::Indeterminate
        } else if self.checked {
            Check::Checked
        } else {
            Check::Unchecked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_wins_and_suppresses_the_pointer() {
        for pointer in [State::Default, State::Hover, State::Pressed] {
            assert_eq!(States::NONE.disabled(true).at(pointer), State::Disabled);
        }
        let all = States::NONE
            .hovered(true)
            .pressed(true)
            .focused(true)
            .loading(true)
            .disabled(true);
        assert_eq!(all.state(), State::Disabled);
        assert!(!all.responds());
        assert!(!all.shows_focus(), "a disabled control draws no focus");
    }

    #[test]
    fn pressed_wins_over_hover() {
        assert_eq!(States::NONE.hovered(true).state(), State::Hover);
        assert_eq!(States::NONE.pressed(true).state(), State::Pressed);
        assert_eq!(
            States::NONE.hovered(true).pressed(true).state(),
            State::Pressed
        );
        assert_eq!(States::NONE.state(), State::Default);
    }

    #[test]
    fn loading_keeps_its_resting_look_and_its_focus() {
        let loading = States::NONE.loading(true).hovered(true).pressed(true);
        assert_eq!(loading.state(), State::Default);
        assert!(!loading.responds());
        assert!(loading.focused(true).shows_focus());
    }

    #[test]
    fn focus_is_independent_of_the_pointer() {
        for s in [
            States::NONE,
            States::NONE.hovered(true),
            States::NONE.pressed(true),
        ] {
            assert_eq!(s.focused(true).state(), s.state());
            assert!(s.focused(true).shows_focus());
            assert!(!s.shows_focus());
        }
    }

    #[test]
    fn indeterminate_wins_over_checked() {
        assert_eq!(States::NONE.check(), Check::Unchecked);
        assert_eq!(States::NONE.checked(true).check(), Check::Checked);
        assert_eq!(
            States::NONE.checked(true).indeterminate(true).check(),
            Check::Indeterminate
        );
        assert_eq!(States::default(), States::NONE);
    }
}
