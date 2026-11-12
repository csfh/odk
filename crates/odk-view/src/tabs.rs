//! Tabs on the Rune host: a list of tabs over the selected one's panel, the selected tab marked
//! by an indicator on the list's baseline (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::Glyph;
use crate::ink::ink;
use crate::style::{self, inset_focus_ring};
use crate::text::plain;
use crate::toggle::{Toggle, pressable};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{NavLook, States};
use odk_theme::foundation::{Density, TypeRole};
use rune::{El, col, el, row};

/// What the application hears when a tab is selected: its value.
type Selected<T> = Rc<dyn Fn(&mut T, &str)>;

/// Builds a tab's panel, when it is the selected one.
type Content<T> = Box<dyn FnOnce(&Hosted<T>) -> El>;

struct Tab<T> {
    value: String,
    label: String,
    icon: Option<Glyph>,
    counter: Option<String>,
    content: Content<T>,
    disabled: bool,
}

/// A list of tabs above the content of the selected one. The selected tab has primary text and a
/// thin accent indicator on the list's baseline; the others are secondary text with a quiet tint
/// under the pointer; no tab has a box. A tab may have an icon before its label and a counter
/// after it.
///
/// Tab reaches the selected tab, the list's one tab stop. Left and Right move to the neighbour,
/// wrapping, and Home and End to the ends, skipping disabled tabs. By default a tab is selected
/// as focus reaches it (automatic activation); with `manual_activation` the arrows only move
/// focus, and Enter or Space selects the focused tab. Only the selected tab's content is built.
/// Disabled tabs are not tab stops. Tabs that do not fit scroll sideways.
///
/// Uncontrolled by default, starting on `default_value` or the first tab and keeping the choice
/// in the host's store; pass `value` to control it and follow `on_change`, which hears of each
/// change once.
pub struct Tabs<T> {
    id: String,
    name: String,
    tabs: Vec<Tab<T>>,
    value: Option<String>,
    default_value: Option<String>,
    on_change: Option<Selected<T>>,
    disabled: bool,
    manual: bool,
    density: Option<Density>,
}

impl<T: 'static> Tabs<T> {
    /// Tabs whose list is named `name` for assistive technology, such as "Settings sections"; the
    /// list has no visible label of its own, so the name is required.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tabs: Vec::new(),
            value: None,
            default_value: None,
            on_change: None,
            disabled: false,
            manual: false,
            density: None,
        }
    }

    /// Adds a tab; `content` builds its panel when it is the selected one.
    pub fn tab(
        mut self,
        value: impl Into<String>,
        label: impl Into<String>,
        content: impl FnOnce(&Hosted<T>) -> El + 'static,
    ) -> Self {
        self.tabs.push(Tab {
            value: value.into(),
            label: label.into(),
            icon: None,
            counter: None,
            content: Box::new(content),
            disabled: false,
        });
        self
    }

    /// Puts an icon before the label of the tab with this value, at the density's icon size and in
    /// the label's colour: one of ODK's icons or a character.
    pub fn icon(mut self, value: &str, glyph: impl Into<Glyph>) -> Self {
        let glyph = glyph.into();
        for tab in self.tabs.iter_mut().filter(|t| t.value == value) {
            tab.icon = Some(glyph.clone());
        }
        self
    }

    /// Puts a counter, such as `"12"`, after the label of the tab with this value, in smaller text
    /// of the label's colour.
    pub fn counter(mut self, value: &str, counter: impl Into<String>) -> Self {
        let counter = counter.into();
        for tab in self.tabs.iter_mut().filter(|t| t.value == value) {
            tab.counter = Some(counter.clone());
        }
        self
    }

    /// Disables the tab with this value: it cannot be selected or focused. Its panel still shows
    /// if it is the selected tab, and so does its indicator, greyed.
    pub fn disable(mut self, value: &str) -> Self {
        for tab in self.tabs.iter_mut().filter(|t| t.value == value) {
            tab.disabled = true;
        }
        self
    }

    /// Disables every tab; the selected panel still shows.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The arrow keys move focus without selecting; Enter or Space selects the focused tab. For
    /// tabs whose panels are slow to build, so moving through them does not build each one.
    pub fn manual_activation(mut self, manual: bool) -> Self {
        self.manual = manual;
        self
    }

    /// The tabs' own density, whatever the application's.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The selected tab, making the tabs controlled.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// Hears the tab the user selected, when it is not the selected one.
    pub fn on_change(mut self, handler: impl Fn(&mut T, &str) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn show(self, hosted: &Hosted<T>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted);
        let density = paint.tokens.density(self.density);
        let foundation = paint.tokens.foundation().clone();
        let first = self
            .default_value
            .clone()
            .or_else(|| self.tabs.first().map(|t| t.value.clone()))
            .unwrap_or_default();
        let selected = self
            .value
            .clone()
            .or_else(|| hosted.ui.choice(&self.id).map(str::to_string))
            .unwrap_or_else(|| first.clone());
        let index = self
            .tabs
            .iter()
            .position(|t| t.value == selected)
            .unwrap_or(0);
        let enabled: Vec<usize> = (0..self.tabs.len())
            .filter(|i| !self.disabled && !self.tabs[*i].disabled)
            .collect();
        // The one tab in the Tab order: the selected one if it is enabled, else the first
        // enabled one, else none.
        let stop = if enabled.contains(&index) {
            Some(index)
        } else {
            enabled.first().copied()
        };

        let (controlled, manual) = (self.value.clone(), self.manual);
        let mut list = apply(row().key(self.id.clone()), &accessible)
            .focus_group()
            .scroll_x();
        let mut content = None;
        for (i, tab) in self.tabs.into_iter().enumerate() {
            let on = i == index;
            let usable = enabled.contains(&i);
            let s = paint.tokens.tab(
                NavLook::Text,
                density,
                States::NONE.selected(on).disabled(!usable),
            );
            let g = s.geometry;
            let name = match &tab.counter {
                Some(counter) => format!("{} ({counter})", tab.label),
                None => tab.label.clone(),
            };
            // Rune presses a `tab` on focus, which is automatic activation. Manual activation
            // needs the arrows to move without selecting, so its tabs are toggle buttons.
            let role = if manual {
                Role::ToggleButton
            } else {
                Role::Tab
            };
            let tab_accessible = Accessible::new(role, name).selected(on).disabled(!usable);
            let key = format!("{}-{}", self.id, tab.value);
            let mut trigger = apply(row().key(key.clone()), &tab_accessible)
                .h(g.height)
                .px(g.padding_x)
                .gap(g.gap)
                .center_y()
                .fill(ink(s.colours.default.background))
                .focusable(usable)
                .focus_selected(Some(i) == stop)
                .no_focus_ring();
            if s.responds {
                trigger = trigger
                    .hover(ink(s.colours.hover.background))
                    .press(ink(s.colours.pressed.background));
            }
            let foreground = s.colours.default.foreground;
            if let Some(glyph) = tab.icon {
                trigger = trigger.child(glyph.slot(g.icon_size, ink(foreground)));
            }
            trigger = trigger.child(plain(&paint, tab.label, g.type_role, foreground));
            if let Some(counter) = tab.counter {
                trigger = trigger.child(plain(&paint, counter, TypeRole::Small, foreground));
            }
            // The selected tab's indicator is on the list's baseline and over it, so it takes no
            // space and reads as one line with it. A disabled selected tab's is greyed.
            if let Some(indicator) = s.indicator {
                trigger = trigger.child(
                    el().h(s.indicator_width)
                        .pin(Some(0.0), None, Some(0.0), Some(0.0))
                        .fill(ink(indicator))
                        .a11y_hidden(),
                );
            }
            // Inside the tab: the list scrolls, so it clips a ring drawn outside.
            trigger = inset_focus_ring(trigger, &s.focus);
            if on {
                content = Some(tab.content);
            }
            if usable {
                let (id, value) = (self.id.clone(), tab.value.clone());
                let (controlled, first) = (controlled.clone(), first.clone());
                let change = self.on_change.clone();
                let select: Toggle<T> = Rc::new(move |hosted: &mut Hosted<T>| {
                    let current = controlled
                        .clone()
                        .or_else(|| hosted.ui.choice(&id).map(str::to_string))
                        .unwrap_or_else(|| first.clone());
                    if current == value {
                        return;
                    }
                    if controlled.is_none() {
                        hosted.ui.set_choice(&id, &value);
                    }
                    if let Some(change) = &change {
                        change(&mut hosted.app, &value);
                    }
                });
                trigger = pressable(trigger, &key, true, select);
            }
            list = list.child(trigger);
        }

        let baseline = paint
            .tokens
            .tab(NavLook::Text, density, States::NONE)
            .baseline;
        let strip = col()
            // The baseline first, so the tabs and their indicators are drawn over it.
            .child(
                el().h(foundation.geometry.border_width)
                    .pin(Some(0.0), None, Some(0.0), Some(0.0))
                    .fill(ink(baseline))
                    .a11y_hidden(),
            )
            .child(list);
        let mut panel = col().pt(foundation.spacing.s3);
        if let Some(build) = content {
            panel = panel.child(build(hosted));
        }
        col().child(strip).child(panel)
    }
}

impl<T> Describe for Tabs<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::TabList, self.name.clone()).disabled(self.disabled)
    }
}
