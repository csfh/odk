//! A command palette (ADR 0038, `docs/spec/odk-text-input.md`, "Command"): a modal panel with
//! an `Input` over a list that narrows as the user types.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, ElementId, Entity, InteractiveElement, IntoElement, KeyDownEvent, KeyUpEvent,
    MouseButton, ParentElement, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;

use crate::focus::{child_id, focus_handle, move_focus, part_name};
use crate::input::Input;
use crate::interaction::{Pointer, pointer_states};
use crate::layout::{h_flex, inside_window, single_line, v_flex};
use crate::menu_list::hint_text;
use crate::overlay::{Action, Dismiss, Transition, layer_over, modal};
use crate::theme::colour;
use crate::{StyledExt, components};
use odk_theme::foundation::MotionDuration;

/// The widest a palette is, in logical pixels. ODK's own (ADR 0035).
const COMMAND_WIDTH: f32 = 560.0;

/// How far down the window the panel's top is, as a fraction of its height. ODK's own: the
/// panel hangs from there, so that the field stays where it is as the list narrows.
const COMMAND_TOP: f32 = 0.18;

/// The tallest its list is, unless the window leaves less, in logical pixels. ODK's own.
const COMMAND_LIST_HEIGHT: f32 = 320.0;

/// One thing a [`Command`] can do.
#[derive(Clone)]
pub struct CommandItem {
    label: SharedString,
    hint: Option<SharedString>,
    keywords: SharedString,
    disabled: bool,
    on_select: Option<Action>,
}

impl CommandItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            keywords: SharedString::default(),
            disabled: false,
            on_select: None,
        }
    }

    /// A key chord, such as `Ctrl+C`, shown at the end of the item as muted text. It only labels the
    /// item; the application binds the key.
    pub fn hint(mut self, chord: impl Into<SharedString>) -> Self {
        self.hint = Some(chord.into());
        self
    }

    /// More text that finds the item, not shown.
    pub fn keywords(mut self, keywords: impl Into<SharedString>) -> Self {
        self.keywords = keywords.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called after the palette has been asked to close, when the item is chosen.
    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

/// Whether `query` finds an item: every word of it, lowercased, is somewhere in the label or
/// the keywords, lowercased. An empty query finds everything.
fn finds(query: &str, label: &str, keywords: &str) -> bool {
    let haystack = format!("{label} {keywords}").to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

/// What a palette lists for a query: the items that match, those of them that can be chosen,
/// and the one Enter would run, which is `highlighted` if it can be chosen and otherwise the
/// first that can.
struct Listing {
    matching: Vec<usize>,
    enabled: Vec<usize>,
    current: Option<usize>,
}

fn listing(query: &str, items: &[CommandItem], highlighted: Option<usize>) -> Listing {
    let matching: Vec<usize> = (0..items.len())
        .filter(|i| finds(query, &items[*i].label, &items[*i].keywords))
        .collect();
    let enabled: Vec<usize> = matching
        .iter()
        .copied()
        .filter(|i| !items[*i].disabled)
        .collect();
    let current = highlighted
        .filter(|h| enabled.contains(h))
        .or(enabled.first().copied());
    Listing {
        matching,
        enabled,
        current,
    }
}

/// Where the highlight goes from `current` among the `enabled` items (indices, in order) when
/// moving `forward` or back, wrapping at the ends. With no current item, the first or last.
fn step(enabled: &[usize], current: Option<usize>, forward: bool) -> Option<usize> {
    let at = current.and_then(|c| enabled.iter().position(|e| *e == c));
    let next = match (at, forward) {
        (Some(at), true) => (at + 1) % enabled.len().max(1),
        (Some(at), false) => (at + enabled.len().max(1) - 1) % enabled.len().max(1),
        (None, true) => 0,
        (None, false) => enabled.len().checked_sub(1)?,
    };
    enabled.get(next).copied()
}

/// A palette: a modal panel with a field over a list of commands, narrowed by what is typed.
///
/// Opening it puts focus in the field, with nothing typed; closing gives focus back to where it
/// was. Typing narrows the list and highlights the first command that is left; Down and Up move
/// the highlight, wrapping; Enter, when it comes up, closes the palette and runs the
/// highlighted command, and does nothing when none is left; Escape, a click on the scrim or a click outside asks to
/// close. Focus never leaves the field, so Tab stays where it is. A click on a command, or
/// hovering one, highlights it, and a click runs it. Controlled: the application owns `open`.
/// While it closes it keeps drawing the commands it last had open.
#[derive(IntoElement)]
pub struct Command {
    id: ElementId,
    name: SharedString,
    open: bool,
    placeholder: SharedString,
    empty: SharedString,
    items: Vec<CommandItem>,
    on_close: Option<Action>,
}

impl Command {
    /// `name` is what the palette is called to assistive technology.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            open: false,
            placeholder: "Search…".into(),
            empty: "No results".into(),
            items: Vec::new(),
            on_close: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Shown, muted, in the empty field.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Shown in place of the list when nothing matches.
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = text.into();
        self
    }

    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// What the list and each command report, given the one highlighted.
    pub fn item_accessibles(&self, highlighted: Option<usize>) -> Vec<Accessible> {
        let mut all = vec![Accessible::new(Role::Listbox, self.name.to_string())];
        all.extend(self.items.iter().enumerate().map(|(i, item)| {
            Accessible::new(Role::Option, item.label.to_string())
                .selected(highlighted == Some(i))
                .disabled(item.disabled)
        }));
        all
    }
}

impl Describe for Command {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Dialog, self.name.to_string())
    }
}

/// What a palette keeps between frames.
struct CommandState {
    query: SharedString,
    /// The item (an index into the items) that Enter would run; the first enabled match when
    /// it is not one of them.
    highlighted: Option<usize>,
    /// The item Enter was pressed on, to run when the key comes up. A key that comes up after
    /// the palette has closed would land on whatever had focus before it, and a button there
    /// is pressed by Enter's release.
    armed: Option<usize>,
    scroll: ScrollHandle,
    /// What the palette was last opened with, kept until its exit has run.
    shown: Shown,
}

/// Everything a palette needs to draw its exit.
#[derive(Clone)]
struct Shown {
    placeholder: SharedString,
    empty: SharedString,
    items: Vec<CommandItem>,
}

/// Puts what the palette shows back to nothing typed.
fn reset(state: &Entity<CommandState>, cx: &mut App) {
    state.update(cx, |s, _| {
        s.query = SharedString::default();
        s.highlighted = None;
        s.armed = None;
    });
}

impl RenderOnce for Command {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let f = c.foundation().clone();
        let wanted = Shown {
            placeholder: self.placeholder,
            empty: self.empty,
            items: self.items,
        };
        let first = wanted.clone();
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| CommandState {
            query: SharedString::default(),
            highlighted: None,
            armed: None,
            scroll: ScrollHandle::new(),
            shown: first,
        });
        #[cfg(test)]
        STATES.with(|s| {
            s.borrow_mut()
                .insert(crate::focus::id_path(&self.id), state.clone());
        });
        let frame = layer_over(
            child_id(&self.id, "layer"),
            self.open,
            true,
            Transition::of(MotionDuration::Enter, MotionDuration::Exit),
            window,
            cx,
        );
        let Some(amount) = frame.amount else {
            reset(&state, cx);
            return div().into_any_element();
        };
        let amount = f.motion.easing.apply(amount);
        if frame.opened {
            reset(&state, cx);
        }
        let open = self.open;
        let (query, highlighted, scroll, shown) = state.update(cx, |s, _| {
            if open {
                s.shown = wanted;
            }
            (
                s.query.clone(),
                s.highlighted,
                s.scroll.clone(),
                s.shown.clone(),
            )
        });
        let close: Action = self.on_close.unwrap_or_else(|| Rc::new(|_, _| {}));

        // Focus goes to the field once the palette has focus itself; the field's handle is made
        // here, since a handle belongs to the element that made it.
        let field = focus_handle(child_id(&self.id, "field-focus"), true, window, cx);
        if frame.opened {
            let field = field.clone();
            window.defer(cx, move |window, _| move_focus(&field, window));
        }

        let Listing {
            matching, current, ..
        } = listing(&query, &shown.items, highlighted);

        let mut input = Input::new(child_id(&self.id, "input"), self.name.clone())
            .placeholder(shown.placeholder.clone())
            .value(query.clone())
            .on_change({
                let (state, scroll) = (state.clone(), scroll.clone());
                move |text, window, cx| {
                    state.update(cx, |s, _| {
                        s.query = text.to_owned().into();
                        s.highlighted = None;
                        s.armed = None;
                    });
                    scroll.scroll_to_item(0);
                    window.refresh();
                }
            })
            .on_submit({
                // From the state, which has heard every key; the frame may not have.
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, _| {
                        s.armed = listing(&s.query, &s.shown.items, s.highlighted).current;
                    })
                }
            });
        input.focus = Some(field.clone());

        // The rows are menu items on component tokens: the highlight is the keyboard's place,
        // apart from the pointer's hover. The panel around them is still the dialog's.
        let density = c.density(None);
        let rows: Vec<AnyElement> = matching
            .iter()
            .map(|&i| {
                let item = &shown.items[i];
                let states = States::NONE
                    .disabled(item.disabled)
                    .highlighted(current == Some(i));
                let s = c.menu_item(density, states, false);
                let g = s.geometry;
                let mut row = h_flex()
                    .group(format!("row-{i}"))
                    .id(child_id(&self.id, format!("row-{i}")))
                    .debug_selector(|| part_name(&self.id, &format!("row-{i}")))
                    .flex_shrink_0()
                    .h(px(g.height))
                    .px(px(g.padding_x))
                    .gap(px(g.gap))
                    .text_role(g.type_role)
                    .child(single_line(
                        div()
                            .flex_1()
                            .min_w_0()
                            .debug_selector(|| part_name(&self.id, &format!("label-{i}")))
                            .child(item.label.clone()),
                    ))
                    .children(item.hint.as_ref().map(|h| {
                        hint_text(
                            child_id(&self.id, format!("hint-{i}")),
                            h.clone(),
                            &s,
                            &SharedString::from(format!("row-{i}")),
                        )
                    }));
                row = pointer_states(row, &s.colours, s.responds, &Pointer::Own);
                if !item.disabled {
                    let (state, field) = (state.clone(), field.clone());
                    let (close, on_select) = (close.clone(), item.on_select.clone());
                    row = row
                        .cursor_pointer()
                        .on_mouse_move({
                            let state = state.clone();
                            move |_, window, cx| {
                                let changed = state.update(cx, |s, _| {
                                    let changed = s.highlighted != Some(i);
                                    s.highlighted = Some(i);
                                    changed
                                });
                                if changed {
                                    window.refresh();
                                }
                            }
                        })
                        .on_click(move |_, window, cx| {
                            move_focus(&field, window);
                            close(window, cx);
                            if let Some(f) = &on_select {
                                f(window, cx);
                            }
                        });
                }
                row.into_any_element()
            })
            .collect();

        let (key_state, key_scroll) = (state.clone(), scroll.clone());
        let viewport = window.viewport_size();
        let margin = px(f.spacing.s4);
        let style = c.command();
        let g = style.overlay.geometry;
        let list = div()
            .id(child_id(&self.id, "list"))
            .debug_selector(|| part_name(&self.id, "list"))
            .flex_1()
            .min_h_0()
            .max_h(px(COMMAND_LIST_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .py(px(f.spacing.s1))
            .when(rows.is_empty(), |d| {
                let empty = c.command().empty;
                d.child(
                    div()
                        .debug_selector(|| part_name(&self.id, "empty"))
                        .px(px(f.spacing.s3))
                        .py(px(f.spacing.s2))
                        .text_color(colour(empty))
                        .child(shown.empty.clone()),
                )
            })
            .children(rows);
        let width = px(COMMAND_WIDTH).min(inside_window(viewport.width, margin));
        let top = viewport.height * COMMAND_TOP;
        let panel = v_flex()
            .id(child_id(&self.id, "panel"))
            .debug_selector(|| part_name(&self.id, "panel"))
            .absolute()
            .left((viewport.width - width) / 2.0)
            .w(width)
            .max_h(viewport.height - top - margin)
            .treatment(&style.overlay.surface)
            .border(px(g.border_width))
            .rounded(px(g.radius))
            .opacity(amount)
            // A slight rise as it fades in.
            .top(top + px((1.0 - amount) * f.spacing.s2))
            .text_role(g.body)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let forward = match event.keystroke.key.as_str() {
                    "down" => true,
                    "up" => false,
                    _ => return,
                };
                // From the state, which has heard every key; the frame may not have.
                let next = key_state.update(cx, |s, _| {
                    let Listing {
                        matching,
                        enabled,
                        current,
                    } = listing(&s.query, &s.shown.items, s.highlighted);
                    let next = step(&enabled, current, forward)?;
                    s.highlighted = Some(next);
                    // The rows are the scroll container's children, in order.
                    matching.iter().position(|m| *m == next)
                });
                if let Some(position) = next {
                    key_scroll.scroll_to_item(position);
                    window.refresh();
                }
                cx.stop_propagation();
            })
            .on_key_up({
                let (state, shown, close) = (state.clone(), shown.clone(), close.clone());
                move |event: &KeyUpEvent, window, cx| {
                    if event.keystroke.key != "enter" {
                        return;
                    }
                    if let Some(item) = state.update(cx, |s, _| s.armed.take()) {
                        close(window, cx);
                        if let Some(f) = &shown.items[item].on_select {
                            f(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }
            })
            // A press on the panel, which takes focus when pressed, keeps it in the field.
            .on_mouse_down(MouseButton::Left, {
                let field = field.clone();
                move |_, window, _| {
                    move_focus(&field, window);
                    window.prevent_default();
                }
            })
            .child(div().p(px(f.spacing.s2)).child(input))
            .child(
                div()
                    .h(px(g.border_width))
                    .bg(colour(style.overlay.surface.border)),
            )
            .child(list);
        modal(
            &self.id,
            amount,
            &Dismiss::new(close),
            &frame.content,
            panel,
            window,
            cx,
        )
    }
}

#[cfg(test)]
thread_local! {
    /// The state of each palette rendered on this thread, by id, for tests to read.
    static STATES: std::cell::RefCell<std::collections::HashMap<String, Entity<CommandState>>> =
        Default::default();
}

/// What a test sees of the palette with this id: what is typed, the items listed and the one
/// that is highlighted, and how many items it is keeping to draw its exit.
#[cfg(test)]
pub(crate) fn probe(
    cx: &mut gpui::VisualTestContext,
    id: &str,
) -> (String, Vec<usize>, Option<usize>, usize) {
    let state = STATES.with(|s| s.borrow()[id].clone());
    state.read_with(cx, |s, _| {
        let listing = listing(&s.query, &s.shown.items, s.highlighted);
        (
            s.query.to_string(),
            listing.matching,
            listing.current,
            s.shown.items.len(),
        )
    })
}

/// Changes what the palette has heard, as typing or the arrows do, without drawing a frame,
/// which the test platform otherwise does after every key.
#[cfg(test)]
pub(crate) fn hear(
    cx: &mut gpui::VisualTestContext,
    id: &str,
    query: &str,
    highlighted: Option<usize>,
) {
    let state = STATES.with(|s| s.borrow()[id].clone());
    state.update(cx, |s, _| {
        s.query = query.to_owned().into();
        s.highlighted = highlighted;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_must_be_somewhere_in_the_label_or_the_keywords() {
        assert!(finds("", "Open file", ""));
        assert!(finds("open", "Open file", ""));
        assert!(finds("FILE open", "Open file", ""));
        assert!(finds("quit", "Close window", "exit quit"));
        assert!(!finds("open quit", "Open file", ""));
        assert!(finds("öff", "Öffnen", ""));
        assert!(finds("  ", "anything", ""));
    }

    #[test]
    fn the_highlight_wraps_among_the_enabled_items() {
        let enabled = [1, 3, 4];
        assert_eq!(step(&enabled, Some(1), true), Some(3));
        assert_eq!(step(&enabled, Some(4), true), Some(1));
        assert_eq!(step(&enabled, Some(1), false), Some(4));
        assert_eq!(step(&enabled, None, true), Some(1));
        assert_eq!(step(&enabled, None, false), Some(4));
        assert_eq!(
            step(&enabled, Some(2), true),
            Some(1),
            "not an enabled item"
        );
        assert_eq!(step(&[], Some(0), true), None);
        assert_eq!(step(&[], None, false), None);
    }
}
