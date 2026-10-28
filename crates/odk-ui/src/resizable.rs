//! A group of panels in a row or a column whose sizes the user changes by dragging the handle
//! between two of them, or with the keys on a focused handle (ADR 0069).

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, CursorStyle, DispatchPhase, ElementId, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, canvas, div, px, relative,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;

use crate::components;
use crate::focus::{Axis, child_id, focus_handle, move_focus, part_name};
use crate::interaction::focus_ring;
use crate::theme::colour;

/// How far one press of an arrow key moves a handle: a twentieth of the group. ODK's own.
const KEY_STEP: f32 = 0.05;

/// A panel's limits, as fractions of its group.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Limits {
    default: Option<f32>,
    min: f32,
    max: f32,
}

/// One panel of a [`Resizable`]: its content, the size it starts at and the sizes it keeps
/// between, as fractions of the group.
pub struct ResizablePanel {
    limits: Limits,
    content: Vec<AnyElement>,
}

impl ResizablePanel {
    /// A panel that shares what the panels with a `default_size` leave.
    pub fn new() -> Self {
        Self {
            limits: Limits {
                default: None,
                min: 0.0,
                max: 1.0,
            },
            content: Vec::new(),
        }
    }

    /// The fraction of the group it starts at, such as `0.4`. Give it the fraction the
    /// application kept from `on_layout` to put it back.
    pub fn default_size(mut self, fraction: f32) -> Self {
        self.limits.default = Some(fraction);
        self
    }

    /// The smallest fraction of the group it can be made.
    pub fn min_size(mut self, fraction: f32) -> Self {
        self.limits.min = fraction;
        self
    }

    /// The largest fraction of the group it can be made.
    pub fn max_size(mut self, fraction: f32) -> Self {
        self.limits.max = fraction;
        self
    }
}

impl Default for ResizablePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for ResizablePanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

/// Panels side by side (`horizontal`) or stacked (`vertical`), filling their parent, with a
/// handle between each two: a line one border wide that takes the accent's colour under the
/// pointer, with a pointer target `geometry.target_min` across that takes no room.
///
/// Dragging a handle resizes the two panels beside it, the one before taking what the one after
/// gives, within their `min_size` and `max_size`; the drag lasts until the button is released,
/// wherever the pointer goes, and Escape during a drag puts the sizes back. `on_layout` hears the
/// fractions once a drag ends, which is when to keep them (ADR 0069).
///
/// Keyboard: each handle is a tab stop. The arrow keys along the group (Left and Right in a row,
/// Up and Down in a column) move it by a twentieth of the group; Home and End make the panel
/// before it as small or as large as it can be. Each key press is reported to `on_layout`.
///
/// ```ignore
/// Resizable::horizontal("services", "Services and detail")
///     .panel(ResizablePanel::new().default_size(0.4).min_size(0.25).child(list))
///     .panel(ResizablePanel::new().min_size(0.3).child(detail))
///     .on_layout(|sizes, _, cx| keep(sizes, cx))
/// ```
#[derive(IntoElement)]
pub struct Resizable {
    id: ElementId,
    name: SharedString,
    axis: Axis,
    panels: Vec<ResizablePanel>,
    disabled: bool,
    on_layout: OnLayout,
}

impl Resizable {
    /// Panels side by side, named `name` (such as "Services and detail") for assistive
    /// technology.
    pub fn horizontal(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self::new(id, name, Axis::Horizontal)
    }

    /// Panels stacked one above another.
    pub fn vertical(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self::new(id, name, Axis::Vertical)
    }

    fn new(id: impl Into<ElementId>, name: impl Into<SharedString>, axis: Axis) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            axis,
            panels: Vec::new(),
            disabled: false,
            on_layout: None,
        }
    }

    pub fn panel(mut self, panel: ResizablePanel) -> Self {
        self.panels.push(panel);
        self
    }

    /// The handles neither move nor take focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Hears every panel's fraction of the group after a drag ends and after a key moves a
    /// handle.
    pub fn on_layout(mut self, f: impl Fn(&[f32], &mut Window, &mut App) + 'static) -> Self {
        self.on_layout = Some(Rc::new(f));
        self
    }

    fn limits(&self) -> Vec<Limits> {
        self.panels.iter().map(|p| p.limits).collect()
    }

    /// Each handle, as a focusable separator whose value is the size of the panel before it,
    /// at the sizes the group starts at.
    pub fn item_accessibles(&self) -> Vec<Accessible> {
        let sizes = initial(&self.limits());
        handle_accessibles(&self.name, &sizes, self.disabled)
    }
}

fn handle_accessibles(name: &str, sizes: &[f32], disabled: bool) -> Vec<Accessible> {
    sizes
        .iter()
        .take(sizes.len().saturating_sub(1))
        .map(|size| {
            Accessible::new(Role::Separator, name)
                .value(format!("{}%", (size * 100.0).round() as u32))
                .disabled(disabled)
        })
        .collect()
}

impl Describe for Resizable {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Group, self.name.to_string()).disabled(self.disabled)
    }
}

/// The limits made sane: within 0 to 1, the largest no smaller than the smallest.
fn sane(limits: &[Limits]) -> Vec<Limits> {
    limits
        .iter()
        .map(|l| {
            let min = finite_or(l.min, 0.0).clamp(0.0, 1.0);
            let max = finite_or(l.max, 1.0).clamp(min, 1.0);
            let default = l
                .default
                .filter(|d| d.is_finite())
                .map(|d| d.clamp(0.0, 1.0));
            Limits { default, min, max }
        })
        .collect()
}

fn finite_or(v: f32, or: f32) -> f32 {
    if v.is_finite() { v } else { or }
}

/// The sizes a group starts at: each panel its default, the panels without one sharing what is
/// left, each within its limits, all together the whole group.
fn initial(limits: &[Limits]) -> Vec<f32> {
    let limits = sane(limits);
    if limits.is_empty() {
        return Vec::new();
    }
    let given: f32 = limits.iter().filter_map(|l| l.default).sum();
    let open = limits.iter().filter(|l| l.default.is_none()).count();
    let share = if open > 0 {
        (1.0 - given).max(0.0) / open as f32
    } else {
        0.0
    };
    let mut sizes: Vec<f32> = limits
        .iter()
        .map(|l| l.default.unwrap_or(share).clamp(l.min, l.max))
        .collect();
    fill(&mut sizes, &limits);
    sizes
}

/// Brings `sizes` to a sum of 1 by growing or shrinking the panels that have room, in order.
fn fill(sizes: &mut [f32], limits: &[Limits]) {
    let mut rest = 1.0 - sizes.iter().sum::<f32>();
    for (size, l) in sizes.iter_mut().zip(limits) {
        if rest.abs() < 1e-6 {
            break;
        }
        let next = (*size + rest).clamp(l.min, l.max);
        rest -= next - *size;
        *size = next;
    }
}

/// The sizes after the handle after panel `handle` moves by `delta` of the group from `start`:
/// the panel before takes what the one after gives, both within their limits.
fn moved(start: &[f32], limits: &[Limits], handle: usize, delta: f32) -> Vec<f32> {
    let (lo, hi) = bounds(start, limits, handle);
    to(start, handle, (start[handle] + delta).clamp(lo, hi))
}

/// How small and how large the panel before handle `handle` can be made, with the one after.
fn bounds(sizes: &[f32], limits: &[Limits], handle: usize) -> (f32, f32) {
    let limits = sane(limits);
    let (a, b) = (&limits[handle], &limits[handle + 1]);
    let pair = sizes[handle] + sizes[handle + 1];
    let lo = a.min.max(pair - b.max).max(0.0);
    let hi = a.max.min(pair - b.min).max(lo);
    (lo, hi)
}

/// `sizes` with the panel before handle `handle` at `size`, the one after taking the rest of
/// the pair.
fn to(sizes: &[f32], handle: usize, size: f32) -> Vec<f32> {
    let mut next = sizes.to_vec();
    let pair = sizes[handle] + sizes[handle + 1];
    next[handle] = size;
    next[handle + 1] = pair - size;
    next
}

/// What a group keeps between frames.
struct Layout {
    sizes: Vec<f32>,
    drag: Option<Drag>,
    /// The handle under the pointer.
    hovered: Option<usize>,
    /// Where the group was laid out on the last frame.
    bounds: Option<Bounds<Pixels>>,
}

#[derive(Clone)]
struct Drag {
    handle: usize,
    /// Where the pointer pressed, along the group.
    from: f32,
    /// The sizes when the drag began, which Escape puts back.
    start: Vec<f32>,
}

impl Axis {
    fn along(self, p: Point<Pixels>) -> f32 {
        f32::from(match self {
            Axis::Horizontal => p.x,
            Axis::Vertical => p.y,
        })
    }

    fn length(self, b: Bounds<Pixels>) -> f32 {
        f32::from(match self {
            Axis::Horizontal => b.size.width,
            Axis::Vertical => b.size.height,
        })
    }
}

type OnLayout = Option<Rc<dyn Fn(&[f32], &mut Window, &mut App)>>;

fn report(on_layout: &OnLayout, sizes: &[f32], window: &mut Window, cx: &mut App) {
    if let Some(f) = on_layout {
        f(sizes, window, cx);
    }
}

impl RenderOnce for Resizable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let limits: Rc<Vec<Limits>> = Rc::new(self.limits());
        let first = initial(&limits);
        let state = window.use_keyed_state(child_id(&self.id, "layout"), cx, |_, _| Layout {
            sizes: first.clone(),
            drag: None,
            hovered: None,
            bounds: None,
        });
        // A group given other panels starts again from their sizes.
        if state.read(cx).sizes.len() != limits.len() {
            state.update(cx, |s, _| {
                s.sizes = first.clone();
                s.drag = None;
                s.hovered = None;
            });
        }
        let (sizes, drag, hovered) = {
            let s = state.read(cx);
            (s.sizes.clone(), s.drag.clone(), s.hovered)
        };
        let axis = self.axis;
        let horizontal = axis == Axis::Horizontal;
        let count = self.panels.len();
        let thickness = c.resize_handle(States::NONE).thickness;
        let cursor = if horizontal {
            CursorStyle::ResizeColumn
        } else {
            CursorStyle::ResizeRow
        };
        #[cfg(test)]
        crate::drawn::record(&crate::focus::id_path(&self.id), sizes.clone());

        let probe_state = state.clone();
        let mut group = div()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "group"))
            .relative()
            .flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                canvas(
                    move |bounds, _, cx| probe_state.update(cx, |s, _| s.bounds = Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        group = if horizontal {
            group.flex_row()
        } else {
            group.flex_col()
        };

        let mut handles = Vec::new();
        let mut before = 0.0;
        for (i, panel) in self.panels.into_iter().enumerate() {
            let size = sizes.get(i).copied().unwrap_or(0.0);
            let mut cell = div()
                .debug_selector(|| part_name(&self.id, &format!("panel-{i}")))
                .flex_basis(px(0.0))
                .flex_shrink_0()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .children(panel.content);
            // Each panel grows by its fraction into what the handles leave.
            cell.style().flex_grow = Some(size);
            cell = if horizontal {
                cell.h_full()
            } else {
                cell.w_full()
            };
            group = group.child(cell);
            before += size;
            if i + 1 < count {
                // The room the handle takes; its line and target are drawn over the panels.
                let room = div().flex_shrink_0();
                group = group.child(if horizontal {
                    room.w(px(thickness)).h_full()
                } else {
                    room.h(px(thickness)).w_full()
                });
                handles.push((i, before));
            }
        }

        let enabled = !self.disabled;
        for (i, fraction) in handles {
            let focus = focus_handle(
                child_id(&self.id, format!("handle-{i}")),
                enabled,
                window,
                cx,
            );
            let focused = enabled && focus.is_focused(window);
            let dragging = drag.as_ref().is_some_and(|d| d.handle == i);
            let states = States::NONE
                .hovered(hovered == Some(i))
                .pressed(dragging)
                .focused(focused)
                .disabled(!enabled);
            let s = c.resize_handle(states);
            #[cfg(test)]
            crate::drawn::record(&part_name(&self.id, &format!("handle-{i}")), s);
            // The handle's room starts after `i + 1` panels' share of what the rooms leave and
            // `i` rooms: `fraction` of the group, moved by the rooms' part of it.
            let shift = thickness * (i as f32 - fraction * (count as f32 - 1.0));
            let line = div().absolute().bg(colour(s.line));
            let line = if horizontal {
                line.top_0()
                    .bottom_0()
                    .left(px((s.target - s.drawn) / 2.0))
                    .w(px(s.drawn))
            } else {
                line.left_0()
                    .right_0()
                    .top(px((s.target - s.drawn) / 2.0))
                    .h(px(s.drawn))
            };
            let mut target = div()
                .id(child_id(&self.id, format!("target-{i}")))
                .debug_selector(|| part_name(&self.id, &format!("handle-{i}")))
                .absolute()
                .child(line);
            target = if horizontal {
                target
                    .top_0()
                    .bottom_0()
                    .left(px(shift + (thickness - s.target) / 2.0))
                    .w(px(s.target))
            } else {
                target
                    .left_0()
                    .right_0()
                    .top(px(shift + (thickness - s.target) / 2.0))
                    .h(px(s.target))
            };
            if s.focus_visible {
                let ring = div().absolute().child(focus_ring(&s.focus, 0.0, 0.0));
                let ring = if horizontal {
                    ring.top_0()
                        .bottom_0()
                        .left(px((s.target - thickness) / 2.0))
                        .w(px(thickness))
                } else {
                    ring.left_0()
                        .right_0()
                        .top(px((s.target - thickness) / 2.0))
                        .h(px(thickness))
                };
                target = target.child(ring);
            }
            if enabled {
                let (hover, press, keys) = (state.clone(), state.clone(), state.clone());
                let (pressed_focus, key_limits) = (focus.clone(), limits.clone());
                let on_layout = self.on_layout.clone();
                target = target
                    .track_focus(&focus)
                    .cursor(cursor)
                    .on_hover(move |on: &bool, _, cx| {
                        hover.update(cx, |s, cx| {
                            let next = if *on {
                                Some(i)
                            } else {
                                s.hovered.filter(|h| *h != i)
                            };
                            if next != s.hovered {
                                s.hovered = next;
                                cx.notify();
                            }
                        });
                    })
                    .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                        press.update(cx, |s, cx| {
                            s.drag = Some(Drag {
                                handle: i,
                                from: axis.along(e.position),
                                start: s.sizes.clone(),
                            });
                            cx.notify();
                        });
                        move_focus(&pressed_focus, window);
                        cx.stop_propagation();
                    })
                    .on_key_down(move |e: &KeyDownEvent, window, cx| {
                        if e.keystroke.modifiers.modified() {
                            return;
                        }
                        let key = e.keystroke.key.as_str();
                        let (back, forward) = if horizontal {
                            ("left", "right")
                        } else {
                            ("up", "down")
                        };
                        if key == "escape" {
                            let cancelled = keys.update(cx, |s, cx| {
                                let d = s.drag.take()?;
                                s.sizes = d.start;
                                cx.notify();
                                Some(())
                            });
                            if cancelled.is_some() {
                                cx.stop_propagation();
                            }
                            return;
                        }
                        let sizes = keys.read(cx).sizes.clone();
                        let next = match key {
                            k if k == back => moved(&sizes, &key_limits, i, -KEY_STEP),
                            k if k == forward => moved(&sizes, &key_limits, i, KEY_STEP),
                            "home" => to(&sizes, i, bounds(&sizes, &key_limits, i).0),
                            "end" => to(&sizes, i, bounds(&sizes, &key_limits, i).1),
                            _ => return,
                        };
                        cx.stop_propagation();
                        if next != sizes {
                            keys.update(cx, |s, cx| {
                                s.sizes = next.clone();
                                cx.notify();
                            });
                            report(&on_layout, &next, window, cx);
                        }
                    });
            }
            group = group.child(div().absolute().top_0().left_0().size_full().child(
                if horizontal {
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(fraction))
                        .child(target)
                } else {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(relative(fraction))
                        .child(target)
                },
            ));
        }

        // While a drag lasts, the whole window's moves and the release are the group's.
        if drag.is_some() {
            let (moves, ups) = (state.clone(), state.clone());
            let (move_limits, on_layout) = (limits.clone(), self.on_layout.clone());
            group = group.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        window.set_window_cursor_style(cursor);
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            moves.update(cx, |s, cx| {
                                let (Some(d), Some(b)) = (&s.drag, s.bounds) else {
                                    return;
                                };
                                let room = axis.length(b) - thickness * (count as f32 - 1.0);
                                if room <= 0.0 {
                                    return;
                                }
                                let delta = (axis.along(e.position) - d.from) / room;
                                let next = moved(&d.start, &move_limits, d.handle, delta);
                                if next != s.sizes {
                                    s.sizes = next;
                                    cx.notify();
                                }
                            });
                        });
                        window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                                return;
                            }
                            let ended = ups.update(cx, |s, cx| {
                                s.drag.take()?;
                                cx.notify();
                                Some(s.sizes.clone())
                            });
                            if let Some(sizes) = ended {
                                report(&on_layout, &sizes, window, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
        }
        group
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(default: Option<f32>, min: f32, max: f32) -> Limits {
        Limits { default, min, max }
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn panels_without_a_size_share_what_the_others_leave() {
        let free = panel(None, 0.0, 1.0);
        assert!(close(&initial(&[free, free]), &[0.5, 0.5]));
        let list = panel(Some(0.4), 0.0, 1.0);
        assert!(close(&initial(&[list, free]), &[0.4, 0.6]));
        assert!(close(&initial(&[list, free, free]), &[0.4, 0.3, 0.3]));
        assert!(initial(&[]).is_empty());
    }

    #[test]
    fn the_sizes_always_fill_the_group_within_their_limits() {
        let tight = panel(Some(0.9), 0.0, 0.6);
        let small = panel(None, 0.0, 0.2);
        let sizes = initial(&[tight, small, panel(None, 0.1, 1.0)]);
        assert!((sizes.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{sizes:?}");
        assert!(
            sizes[0] <= 0.6 && sizes[1] <= 0.2 && sizes[2] >= 0.1,
            "{sizes:?}"
        );
        let wild = panel(Some(f32::NAN), f32::NEG_INFINITY, f32::INFINITY);
        let sizes = initial(&[wild, panel(Some(7.0), 2.0, -1.0)]);
        assert!((sizes.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{sizes:?}");
        assert!(sizes.iter().all(|s| (0.0..=1.0).contains(s)), "{sizes:?}");
    }

    #[test]
    fn a_handle_moves_within_both_panels_limits() {
        let limits = [panel(None, 0.25, 0.7), panel(None, 0.3, 1.0)];
        let start = [0.4, 0.6];
        assert!(close(&moved(&start, &limits, 0, 0.1), &[0.5, 0.5]));
        assert!(
            close(&moved(&start, &limits, 0, -0.5), &[0.25, 0.75]),
            "own minimum"
        );
        assert!(
            close(&moved(&start, &limits, 0, 0.5), &[0.7, 0.3]),
            "both maxima"
        );
        let three = [panel(None, 0.0, 1.0); 3];
        let after = moved(&[0.2, 0.3, 0.5], &three, 1, 0.1);
        assert!(close(&after, &[0.2, 0.4, 0.4]), "only the pair moves");
        assert_eq!(bounds(&[0.4, 0.6], &limits, 0), (0.25, 0.7));
    }

    #[test]
    fn each_handle_reports_the_panel_before_it() {
        let a = handle_accessibles("Split", &[0.4, 0.35, 0.25], false);
        let values: Vec<_> = a.iter().map(|a| a.value.clone().unwrap()).collect();
        assert_eq!(values, ["40%", "35%"]);
        assert!(
            a.iter()
                .all(|a| a.role == Role::Separator && a.name == "Split")
        );
    }
}
