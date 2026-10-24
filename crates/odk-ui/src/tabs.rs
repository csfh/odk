//! Tabs, on component tokens: a list of tabs over the selected one's panel, the selected tab
//! marked by an indicator on the list's baseline (`docs/spec/odk-component-tokens.md`).

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{NavLook, States};
use odk_theme::foundation::{Density, MotionDuration, TypeRole};

use crate::components;
use crate::focus::{Axis, child_id, focus_handle, move_focus, part_name, roving_among};
use crate::interaction::{Pointer, RingTarget, pointer_states, progress};
use crate::layout::{StyledExt, h_flex, v_flex};
use crate::theme::{colour, mono};

type ChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type Content = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement + 'static>;

struct Tab {
    value: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    counter: Option<SharedString>,
    content: Content,
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
/// Disabled tabs are not tab stops.
///
/// Uncontrolled by default, starting on `default_value` or the first tab; pass `value` to
/// control it and follow `on_change`, which hears of each change once.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    tabs: Vec<Tab>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    on_change: Option<ChangeHandler>,
    disabled: bool,
    manual: bool,
    density: Option<Density>,
    name: SharedString,
}

impl Tabs {
    /// Tabs whose list is named `name` for assistive technology, such as "Settings
    /// sections"; the list has no visible label of its own, so the name is required.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
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
    pub fn tab<E: IntoElement>(
        mut self,
        value: impl Into<SharedString>,
        label: impl Into<SharedString>,
        content: impl FnOnce(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.tabs.push(Tab {
            value: value.into(),
            label: label.into(),
            icon: None,
            counter: None,
            content: Box::new(move |w, cx| content(w, cx).into_any_element()),
            disabled: false,
        });
        self
    }

    /// Puts a glyph before the label of the tab with this value, at the density's icon size and
    /// in the label's colour.
    pub fn icon(mut self, value: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Self {
        let (value, glyph) = (value.into(), glyph.into());
        for tab in self.tabs.iter_mut().filter(|t| t.value == value) {
            tab.icon = Some(glyph.clone());
        }
        self
    }

    /// Puts a counter, such as `"12"`, after the label of the tab with this value, in smaller
    /// text of the label's colour.
    pub fn counter(
        mut self,
        value: impl Into<SharedString>,
        counter: impl Into<SharedString>,
    ) -> Self {
        let (value, counter) = (value.into(), counter.into());
        for tab in self.tabs.iter_mut().filter(|t| t.value == value) {
            tab.counter = Some(counter.clone());
        }
        self
    }

    /// Disables the tab with this value: it cannot be selected or focused. Its panel still
    /// shows if it is the selected tab, and so does its indicator, greyed.
    pub fn disable(mut self, value: impl Into<SharedString>) -> Self {
        let value = value.into();
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

    /// The tabs' own density, whatever the application's: tabs 28, 34 or 40 px high.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    /// The selected tab, making the tabs controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// What the tab list and each tab report, given the selected value.
    pub fn item_accessibles(&self, selected: &str) -> Vec<Accessible> {
        self.tabs
            .iter()
            .map(|t| {
                let name = match &t.counter {
                    Some(counter) => format!("{} ({counter})", t.label),
                    None => t.label.to_string(),
                };
                Accessible::new(Role::Tab, name)
                    .selected(t.value == selected)
                    .disabled(self.disabled || t.disabled)
            })
            .collect()
    }
}

impl Describe for Tabs {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::TabList, self.name.to_string()).disabled(self.disabled)
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let density = c.density(self.density);
        let f = c.foundation().clone();
        let first = self
            .default_value
            .clone()
            .or_else(|| self.tabs.first().map(|t| t.value.clone()))
            .unwrap_or_default();
        let state = window.use_keyed_state(child_id(&self.id, "value"), cx, |_, _| first);
        let controlled = self.value.is_some();
        let selected = self.value.clone().unwrap_or_else(|| state.read(cx).clone());
        let index = self
            .tabs
            .iter()
            .position(|t| t.value == selected)
            .unwrap_or(0);

        let enabled: Rc<Vec<usize>> = Rc::new(
            (0..self.tabs.len())
                .filter(|i| !self.disabled && !self.tabs[*i].disabled)
                .collect(),
        );
        // The one tab in the Tab order: the selected one if it is enabled, else the first
        // enabled one, else none.
        let stop = if enabled.contains(&index) {
            Some(index)
        } else {
            enabled.first().copied()
        };
        let handles: Rc<Vec<FocusHandle>> = Rc::new(
            (0..self.tabs.len())
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
            Rc::new(self.tabs.iter().map(|t| t.value.clone()).collect());

        // Selecting: the uncontrolled state follows, then the application hears of it, once.
        let select: ChangeHandler = {
            let (state, on_change, current) =
                (state.clone(), self.on_change.clone(), selected.clone());
            Rc::new(move |value, window, cx| {
                if *value == current {
                    return;
                }
                if !controlled {
                    state.update(cx, |v, cx| {
                        *v = value.clone();
                        cx.notify();
                    });
                }
                if let Some(f) = &on_change {
                    f(value, window, cx);
                }
            })
        };

        // Tabs that do not fit scroll sideways. The list keeps the selected tab in view when
        // the selection changes, and only then, so scrolling it by hand is not undone; the
        // arrow keys also bring the tab they focus into view.
        let scroll = window.use_keyed_state(child_id(&self.id, "scroll"), cx, |_, _| {
            (ScrollHandle::new(), None::<usize>)
        });
        let (handle, shown) = scroll.read(cx).clone();
        if shown != Some(index) {
            handle.scroll_to_item(index);
            scroll.update(cx, |s, _| s.1 = Some(index));
        }
        let (keys, key_values, key_select) = (handles.clone(), values.clone(), select.clone());
        let (key_enabled, key_scroll, manual) = (enabled.clone(), handle.clone(), self.manual);
        let mut list = h_flex()
            .debug_selector(|| part_name(&self.id, "list"))
            .id(child_id(&self.id, "list"))
            .overflow_x_scroll()
            .track_scroll(&handle)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let Some(current) = keys.iter().position(|h| h.is_focused(window)) else {
                    return;
                };
                let key = event.keystroke.key.as_str();
                if let Some(next) = roving_among(key, current, &key_enabled, Axis::Horizontal) {
                    move_focus(&keys[next], window);
                    key_scroll.scroll_to_item(next);
                    if !manual {
                        key_select(&key_values[next], window, cx);
                    }
                    cx.stop_propagation();
                }
            });

        // The focused tab's ring is painted from outside the list, which clips what scrolls,
        // and over the tabs after it.
        let ring = RingTarget::default();
        let mut focus = None;
        let mut content = None;
        for (i, tab) in self.tabs.into_iter().enumerate() {
            let on = i == index;
            if on {
                content = Some(tab.content);
            }
            let usable = enabled.contains(&i);
            let focused = usable && handles[i].is_focused(window);
            let states = States::NONE.selected(on).focused(focused).disabled(!usable);
            let s = c.tab(NavLook::Text, density, states);
            let g = s.geometry;
            // The indicator fades between tabs over the foundation's `normal`; the one fading out
            // keeps the selected colour, which it no longer has a style for.
            let shown = progress(
                child_id(&self.id, format!("indicator-{i}")),
                on,
                MotionDuration::Normal,
                window,
                cx,
            );
            #[cfg(test)]
            crate::drawn::record(&part_name(&self.id, &format!("tab-{i}")), (s, shown));
            let indicator = s.indicator.or_else(|| {
                (shown > 0.0)
                    .then(|| {
                        c.tab(NavLook::Text, density, states.selected(true))
                            .indicator
                    })
                    .flatten()
            });

            let mut trigger = div()
                .id(child_id(&self.id, format!("tab-{i}")))
                .debug_selector(|| part_name(&self.id, &format!("tab-{i}")))
                .relative()
                .flex()
                .items_center()
                .flex_shrink_0()
                .whitespace_nowrap()
                .control_geometry(&g);
            if let Some(glyph) = tab.icon {
                let size = px(g.icon_size);
                trigger = trigger.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .size(size)
                        .font_family(mono())
                        .text_size(size)
                        .line_height(size)
                        .child(glyph),
                );
            }
            trigger = trigger.child(
                div()
                    .debug_selector(|| part_name(&self.id, &format!("label-{i}")))
                    .child(tab.label),
            );
            if let Some(counter) = tab.counter {
                trigger = trigger.child(
                    div()
                        .debug_selector(|| part_name(&self.id, &format!("counter-{i}")))
                        .text_role(TypeRole::Small)
                        .child(counter),
                );
            }
            if let Some(colour_of) = indicator {
                // On the list's baseline and over it, so it takes no space and reads as one line
                // with it.
                let mut line = div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(s.indicator_width))
                    .bg(colour(colour_of))
                    .opacity(shown);
                if on {
                    line = line.debug_selector(|| part_name(&self.id, "underline"));
                }
                trigger = trigger.child(line);
            }
            // Last, so hover and press keep the type role.
            trigger = pointer_states(trigger, &s.colours, s.responds, &Pointer::Own);
            if usable {
                let (value, select) = (tab.value.clone(), select.clone());
                trigger = trigger
                    .track_focus(&handles[i])
                    .cursor_pointer()
                    .on_click(move |_, window, cx| select(&value, window, cx));
            }
            if s.focus_visible {
                trigger = trigger.child(ring.probe());
                focus = Some((s.focus, g.radius));
            }
            list = list.child(trigger);
        }

        let baseline = c.tab(NavLook::Text, density, States::NONE);
        let mut strip = v_flex()
            .debug_selector(|| part_name(&self.id, "strip"))
            .relative()
            // The baseline first, so the tabs and their indicators are drawn over it.
            .child(
                div()
                    .debug_selector(|| part_name(&self.id, "baseline"))
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(f.geometry.border_width))
                    .bg(colour(baseline.baseline)),
            )
            .child(list)
            .child(ring.clip_probe());
        if let Some((focus, radius)) = focus {
            strip = strip.child(ring.ring(focus, radius, &self.id));
        }

        let panel = div()
            .debug_selector(|| part_name(&self.id, "panel"))
            .id(child_id(&self.id, "panel"))
            .pt(px(f.spacing.s3))
            .children(content.map(|build| build(window, cx)));
        v_flex().child(strip).child(panel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_selected_tab() {
        let tabs = Tabs::new("t", "Sections")
            .tab("a", "General", |_, _| div())
            .tab("b", "Advanced", |_, _| div())
            .counter("b", "3");
        let a = tabs.item_accessibles("b");
        assert_eq!(a[0].selected, Some(false));
        assert_eq!(
            (a[1].name.as_str(), a[1].selected),
            ("Advanced (3)", Some(true))
        );
    }
}
