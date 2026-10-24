use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClickEvent, CursorStyle, ElementId, Entity, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, RenderOnce, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Window, div, point, px,
};
use odk_a11y::{Accessible, Describe, Role};

use odk_theme::component::States;
use odk_theme::foundation::Density;

use crate::button::Size;
use crate::components;
use crate::focus::{child_id, focus_handle, part_name};
use crate::input::{field_box, field_glyph};
use crate::layout::{bounds_probe, h_flex, single_line};
use crate::menu_list::{Fit, Menu, MenuItem, Surface, Typeahead, surface};
use crate::theme::colour;

type ChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// How long a pause ends what the user has typed so far, so the next key starts a new search.
/// ODK's own (ADR 0035); it matches the 1 s that browsers and toolkits use.
const TYPEAHEAD_RESET: Duration = Duration::from_millis(1000);

/// The indicator at the box's end. ODK has no icon set yet, so it is a character.
const INDICATOR: &str = "▾";

struct Item {
    value: SharedString,
    label: SharedString,
    disabled: bool,
}

/// A box that shows the chosen option and opens a list of options below itself. It is one tab
/// stop. Enter, Space, Down or Up opens the list with focus on the chosen option (the first
/// when none is chosen); typing the start of an option's label opens it on the first match.
/// In the list, Up and Down move between options and wrap, Home and End jump, typing moves to
/// the next option whose label starts with what was typed, and Enter or Space chooses the
/// option and closes the list. Escape closes it without choosing, Tab or a click outside
/// closes it too, and focus goes back to the box. Disabled options are skipped. A `Select`
/// with no option to choose does not open.
///
/// Uncontrolled by default, keeping its own choice from `default_value`; pass `value` to
/// control it and follow `on_change`. A value that matches no option shows the placeholder.
/// The box is a field's: as high as an `Input` or a `Button` of its density, with the same
/// corners, edge and type, and it keeps its focused edge while its list is open.
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    name: SharedString,
    items: Vec<Item>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    placeholder: Option<SharedString>,
    size: Size,
    density: Option<Density>,
    disabled: bool,
    invalid: bool,
    required: bool,
    pub(crate) description: Option<SharedString>,
    /// The focus handle to use in place of the select's own, for a `FormField` whose label
    /// moves focus to it.
    pub(crate) focus: Option<FocusHandle>,
    on_change: Option<ChangeHandler>,
}

impl Select {
    /// A select named `name` for assistive technology, such as "Country"; the box shows the
    /// chosen option, not what it is for, so the name is required.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            items: Vec::new(),
            value: None,
            default_value: None,
            placeholder: None,
            size: Size::default(),
            density: None,
            disabled: false,
            invalid: false,
            required: false,
            description: None,
            focus: None,
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

    /// Shown in place of the chosen option while none is.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// A step from the density the select would otherwise take, as for an `Input`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The select's own density, for its box and its list's options.
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the choice as wrong: the danger edge and ring, as for an `Input`, and a `!` at the
    /// end. Say what is wrong in a `FormField`'s error.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Reports that something must be chosen. A `FormField` draws the mark beside its label.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// The chosen option's value, making the select controlled.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// The first choice of an uncontrolled select.
    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// Called with the value of the option the user chose, when it differs from the chosen one.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn label_of(&self, value: &str) -> Option<&SharedString> {
        self.items
            .iter()
            .find(|i| i.value == value)
            .map(|i| &i.label)
    }
}

impl Describe for Select {
    /// The box reports the label of the option it shows, as it was given to `value` or
    /// `default_value`.
    fn accessible(&self) -> Accessible {
        let shown = self.value.as_ref().or(self.default_value.as_ref());
        let mut a = Accessible::new(Role::Combobox, self.name.to_string())
            .disabled(self.disabled)
            .invalid(self.invalid)
            .required(self.required);
        a.description = self.description.as_ref().map(|d| d.to_string());
        a.value = shown
            .and_then(|v| self.label_of(v))
            .map(|label| label.to_string());
        a
    }
}

impl Select {
    /// What the list and each option report, given the chosen value.
    pub fn item_accessibles(&self, selected: Option<&str>) -> Vec<Accessible> {
        let mut all = vec![Accessible::new(Role::Listbox, self.name.to_string())];
        all.extend(self.items.iter().map(|i| {
            Accessible::new(Role::Option, i.label.to_string())
                .selected(selected == Some(i.value.as_ref()))
                .disabled(self.disabled || i.disabled)
        }));
        all
    }
}

/// What a select keeps between frames.
struct SelectState {
    open: bool,
    /// The choice of an uncontrolled select.
    value: Option<SharedString>,
    trigger: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
    /// The option that has focus once the list opens.
    focus: Option<usize>,
    typed: Typed,
}

/// The characters typed since the last pause.
#[derive(Default)]
pub(crate) struct Typed {
    text: String,
    at: Option<Instant>,
}

impl Typed {
    /// Adds `ch` to what was typed, starting afresh when the last key was more than
    /// [`TYPEAHEAD_RESET`] before `now`.
    pub(crate) fn push(&mut self, ch: &str, now: Instant) -> &str {
        let fresh = self
            .at
            .is_none_or(|at| now.saturating_duration_since(at) > TYPEAHEAD_RESET);
        if fresh {
            self.text.clear();
        }
        self.text.push_str(ch);
        self.at = Some(now);
        &self.text
    }
}

/// The option a search for `query` goes to, among the `enabled` options (indices into
/// `labels`, in order): the first one at or after `from` whose label starts with `query`,
/// ignoring case, wrapping round. One key alone, or the same key repeated, goes to the next
/// option after `from` that starts with it instead, so pressing a letter again steps through
/// its options.
pub(crate) fn find(
    labels: &[SharedString],
    enabled: &[usize],
    from: Option<usize>,
    query: &str,
) -> Option<usize> {
    let query = query.to_lowercase();
    let first = query.chars().next()?;
    let repeated = query.chars().count() > 1 && query.chars().all(|c| c == first);
    let (needle, after) = if repeated {
        (first.to_string(), true)
    } else {
        (query.clone(), query.chars().count() == 1)
    };
    let at = from.and_then(|f| enabled.iter().position(|e| *e == f));
    let begin = match at {
        Some(at) if after => at + 1,
        Some(at) => at,
        None => 0,
    };
    (0..enabled.len())
        .map(|k| enabled[(begin + k) % enabled.len()])
        .find(|i| labels[*i].to_lowercase().starts_with(&needle))
}

/// What a key adds to a search: a printable character, on its own or with Shift. Space is
/// not one, since it chooses.
pub(crate) fn typed_char(event: &KeyDownEvent) -> Option<String> {
    let ks = &event.keystroke;
    let m = &ks.modifiers;
    if m.control || m.alt || m.platform || m.function {
        return None;
    }
    let text = ks
        .key_char
        .as_deref()
        .or((ks.key.chars().count() == 1).then_some(ks.key.as_str()))?;
    (!text.is_empty() && text.chars().all(|c| !c.is_control() && !c.is_whitespace()))
        .then(|| text.to_string())
}

/// Opens the list on `focus` if it was closed and there is something to choose, closes it if it was open.
fn toggle(
    state: &Entity<SelectState>,
    was_open: bool,
    can_open: bool,
    focus: Option<usize>,
    cx: &mut App,
) {
    state.update(cx, |s, cx| {
        s.open = !was_open && can_open;
        s.focus = focus;
        cx.notify();
    });
}

/// Adds the key to what was typed and finds where the search goes.
fn search(
    state: &Entity<SelectState>,
    labels: &[SharedString],
    enabled: &[usize],
    event: &KeyDownEvent,
    from: Option<usize>,
    cx: &mut App,
) -> Option<usize> {
    let ch = typed_char(event)?;
    let now = cx.background_executor().now();
    let query = state.update(cx, |s, _| s.typed.push(&ch, now).to_string());
    find(labels, enabled, from, &query)
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let id = self.id.clone();
        let usable = !self.disabled;
        let focus = match &self.focus {
            Some(handle) => handle.clone().tab_stop(usable),
            None => focus_handle(child_id(&id, "focus"), usable, window, cx),
        };
        let state = window.use_keyed_state(child_id(&id, "state"), cx, |_, _| SelectState {
            open: false,
            value: self.default_value.clone(),
            trigger: Rc::default(),
            scroll: ScrollHandle::new(),
            focus: None,
            typed: Typed::default(),
        });
        let controlled = self.value.is_some();
        let (open, trigger, scroll, first, kept) = {
            let s = state.read(cx);
            (
                s.open,
                s.trigger.clone(),
                s.scroll.clone(),
                s.focus,
                s.value.clone(),
            )
        };
        let current = self.value.clone().or(kept);
        let selected = current
            .as_ref()
            .and_then(|v| self.items.iter().position(|i| i.value == *v));

        // A select with nothing to choose, or a disabled one, does not open.
        let enabled: Rc<Vec<usize>> = Rc::new(if usable {
            (0..self.items.len())
                .filter(|i| !self.items[*i].disabled)
                .collect()
        } else {
            Vec::new()
        });
        let open = open && !enabled.is_empty();
        let labels: Rc<Vec<SharedString>> =
            Rc::new(self.items.iter().map(|i| i.label.clone()).collect());

        let choose = {
            let values: Vec<SharedString> = self.items.iter().map(|i| i.value.clone()).collect();
            let (state, on_change) = (state.clone(), self.on_change.clone());
            move |i: usize, window: &mut Window, cx: &mut App| {
                let value = values[i].clone();
                state.update(cx, |s, cx| {
                    if !controlled {
                        s.value = Some(value.clone());
                    }
                    s.typed = Typed::default();
                    cx.notify();
                });
                if current.as_ref() != Some(&value)
                    && let Some(f) = &on_change
                {
                    f(&value, window, cx);
                }
            }
        };
        let choose = Rc::new(choose);
        let mut menu = Menu::new();
        for (i, item) in self.items.iter().enumerate() {
            let choose = choose.clone();
            menu = menu.item(
                MenuItem::new(item.label.clone())
                    .checked(selected == Some(i))
                    .disabled(item.disabled)
                    .on_select(move |window, cx| choose(i, window, cx)),
            );
        }

        let b = trigger.get();
        let close: crate::overlay::Action = {
            let state = state.clone();
            Rc::new(move |_, cx| {
                state.update(cx, |s, cx| {
                    s.open = false;
                    s.typed = Typed::default();
                    cx.notify();
                })
            })
        };
        let typeahead: Typeahead = {
            let (state, labels, enabled) = (state.clone(), labels.clone(), enabled.clone());
            Rc::new(move |event, from, cx| search(&state, &labels, &enabled, event, from, cx))
        };
        let density = self
            .density
            .unwrap_or_else(|| self.size.density(c.density(None)));
        let overlay = surface(
            &id,
            menu,
            Surface {
                fit: Fit::Below(b),
                at: point(
                    b.origin.x,
                    b.origin.y + b.size.height + px(c.foundation().spacing.s1),
                ),
                open,
                close,
                // At least as wide as the box it opens from.
                min_width: b.size.width,
                focus: first,
                scroll,
                typeahead: Some(typeahead),
                density: Some(density),
            },
            window,
            cx,
        );

        let focused = usable && focus.is_focused(window);
        let states = States::NONE
            .focused(focused)
            .disabled(!usable)
            .expanded(open)
            .invalid(self.invalid);
        let style = c.select(density, states);
        let s = style.trigger;
        let g = s.geometry;
        #[cfg(test)]
        crate::input::drawn::record(&child_id(&id, "trigger"), s);
        let (shown, shown_ink) = match selected {
            Some(i) => (self.items[i].label.clone(), s.current.foreground),
            None => (self.placeholder.clone().unwrap_or_default(), s.placeholder),
        };

        let mut field = h_flex()
            .id(child_id(&id, "trigger"))
            .debug_selector(|| part_name(&id, "trigger"))
            .w_full()
            .gap(px(g.gap))
            .child(
                single_line(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(g.line_height))
                        .debug_selector(|| part_name(&id, "value"))
                        .text_color(colour(shown_ink)),
                )
                .child(shown),
            );
        if let Some(mark) = s.invalid_mark {
            field = field.child(
                field_glyph(
                    odk_theme::component::FIELD_INVALID_MARK.into(),
                    g.icon_size,
                    colour(mark),
                )
                .debug_selector(|| part_name(&id, "invalid-mark")),
            );
        }
        field = field.child(
            field_glyph(INDICATOR.into(), g.icon_size, colour(style.indicator))
                .debug_selector(|| part_name(&id, "indicator")),
        );
        field = field_box(field, &s, None);

        if usable {
            let (click_state, key_state) = (state.clone(), state.clone());
            let (key_labels, key_enabled) = (labels, enabled);
            let can_open = !key_enabled.is_empty();
            field = field
                .track_focus(&focus)
                .cursor(CursorStyle::PointingHand)
                // The pointer toggles the list on press, like a native select. A press outside
                // the open list has closed it already, so toggle from how it was drawn, not from
                // its state now. A frame can be drawn between press and release, which is why
                // this is not the click.
                .on_mouse_down(MouseButton::Left, {
                    let state = click_state.clone();
                    move |_, _, cx| toggle(&state, open, can_open, selected, cx)
                })
                // Enter and Space arrive as keyboard clicks, with focus on the box, so the list
                // is closed.
                .on_click(move |event: &ClickEvent, _, cx| {
                    if matches!(event, ClickEvent::Keyboard(_)) {
                        toggle(&click_state, open, can_open, selected, cx);
                    }
                })
                .on_key_down(move |event: &KeyDownEvent, _, cx| {
                    let key = event.keystroke.key.as_str();
                    let target = if key == "down" || key == "up" {
                        Some(selected)
                    } else {
                        search(&key_state, &key_labels, &key_enabled, event, selected, cx).map(Some)
                    };
                    if let Some(entry) = target
                        && can_open
                    {
                        key_state.update(cx, |s, cx| {
                            s.open = true;
                            s.focus = entry;
                            cx.notify();
                        });
                        cx.stop_propagation();
                    }
                });
        }

        div()
            .relative()
            .w_full()
            .min_w_0()
            .child(field)
            .child(bounds_probe(px(0.0), move |b, _| trigger.set(b)))
            .children(overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(l: &[&str]) -> Vec<SharedString> {
        l.iter()
            .map(|s| SharedString::from(s.to_string()))
            .collect()
    }

    #[test]
    fn a_letter_goes_to_the_next_option_that_starts_with_it() {
        let l = labels(&["Apple", "Banana", "Apricot", "Avocado"]);
        let all = [0, 1, 2, 3];
        assert_eq!(find(&l, &all, None, "a"), Some(0));
        assert_eq!(find(&l, &all, Some(0), "a"), Some(2));
        assert_eq!(find(&l, &all, Some(3), "a"), Some(0), "wraps");
        assert_eq!(find(&l, &all, Some(0), "A"), Some(2), "ignores case");
        assert_eq!(find(&l, &all, Some(1), "b"), Some(1), "the only match");
        assert_eq!(find(&l, &all, Some(0), "z"), None);
    }

    #[test]
    fn repeating_a_letter_steps_through_its_options() {
        let l = labels(&["Apple", "Banana", "Apricot", "Avocado"]);
        let all = [0, 1, 2, 3];
        assert_eq!(find(&l, &all, Some(0), "aa"), Some(2));
        assert_eq!(find(&l, &all, Some(2), "aaa"), Some(3));
    }

    #[test]
    fn more_letters_narrow_the_search_from_where_it_is() {
        let l = labels(&["Apple", "Banana", "Apricot", "Avocado"]);
        let all = [0, 1, 2, 3];
        // From Apple, "ap" stays on Apple; "apr" moves on.
        assert_eq!(find(&l, &all, Some(0), "ap"), Some(0));
        assert_eq!(find(&l, &all, Some(0), "apr"), Some(2));
        assert_eq!(find(&l, &all, Some(2), "av"), Some(3));
    }

    #[test]
    fn disabled_options_are_not_found() {
        let l = labels(&["Apple", "Apricot", "Banana"]);
        let enabled = [0, 2];
        assert_eq!(find(&l, &enabled, Some(0), "a"), Some(0));
        assert_eq!(find(&l, &enabled, None, "ap"), Some(0));
        assert_eq!(find(&l, &[], None, "a"), None);
    }

    #[test]
    fn non_latin_labels_match_too() {
        let l = labels(&["日本", "日の丸", "Éire"]);
        let all = [0, 1, 2];
        assert_eq!(find(&l, &all, Some(0), "日"), Some(1));
        assert_eq!(find(&l, &all, None, "é"), Some(2));
    }

    #[test]
    fn a_pause_starts_a_new_search() {
        let mut typed = Typed::default();
        let start = Instant::now();
        assert_eq!(typed.push("a", start), "a");
        assert_eq!(typed.push("b", start + TYPEAHEAD_RESET), "ab");
        // Each key counts from the one before it, not from the first.
        let later = start + TYPEAHEAD_RESET + TYPEAHEAD_RESET;
        assert_eq!(typed.push("c", later), "abc");
        assert_eq!(
            typed.push("d", later + TYPEAHEAD_RESET + Duration::from_millis(1)),
            "d"
        );
    }

    #[test]
    fn reports_the_box_and_its_options() {
        let s = Select::new("s", "Country")
            .item("dk", "Denmark")
            .item("se", "Sweden")
            .disable("se")
            .default_value("dk");
        let a = s.accessible();
        assert_eq!((a.role, a.name.as_str()), (Role::Combobox, "Country"));
        assert_eq!(a.value.as_deref(), Some("Denmark"));
        let items = s.item_accessibles(Some("dk"));
        assert_eq!(items[0].role, Role::Listbox);
        assert_eq!(
            (items[1].role, items[1].selected),
            (Role::Option, Some(true))
        );
        assert!(items[2].disabled);
        assert_eq!(items[2].selected, Some(false));
        assert_eq!(
            Select::new("t", "Nothing").accessible().value,
            None,
            "no choice, no value"
        );
        assert!(Select::new("u", "Off").disabled(true).accessible().disabled);
    }
}
