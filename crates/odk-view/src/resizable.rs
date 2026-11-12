//! A group of panels in a row or a column whose sizes the user changes by dragging the handle
//! between two of them, or with the keys on a focused handle (ADR 0069), on the Rune host.
//!
//! The sizes live in the store, as fractions of the group. Rune reports where the group was
//! laid out, so a drag's travel is a share of the room the panels have.

use std::rc::Rc;

use crate::describe::apply;
use crate::host::Hosted;
use crate::ink::ink;
use crate::style::{self, focus_ring};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use rune::{Cursor, Drag, El, Key, KeyEvent, Phase, Tx, col, el, row};

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
    content: Vec<El>,
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

    pub fn child(mut self, element: El) -> Self {
        self.content.push(element);
        self
    }
}

impl Default for ResizablePanel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

type OnLayout<T> = Option<Rc<dyn Fn(&mut T, &[f32])>>;

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
pub struct Resizable<T> {
    id: String,
    name: String,
    axis: Axis,
    panels: Vec<ResizablePanel>,
    disabled: bool,
    on_layout: OnLayout<T>,
}

impl<T: 'static> Resizable<T> {
    /// Panels side by side, named `name` (such as "Services and detail") for assistive
    /// technology.
    pub fn horizontal(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, Axis::Horizontal)
    }

    /// Panels stacked one above another.
    pub fn vertical(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, Axis::Vertical)
    }

    fn new(id: impl Into<String>, name: impl Into<String>, axis: Axis) -> Self {
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
    pub fn on_layout(mut self, f: impl Fn(&mut T, &[f32]) + 'static) -> Self {
        self.on_layout = Some(Rc::new(f));
        self
    }

    fn limits(&self) -> Vec<Limits> {
        self.panels.iter().map(|p| p.limits).collect()
    }

    /// Each handle, as a focusable separator whose value is the size of the panel before it, at
    /// the sizes the group starts at.
    pub fn item_accessibles(&self) -> Vec<Accessible> {
        handle_accessibles(&self.name, &initial(&self.limits()), self.disabled)
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

impl<T> Describe for Resizable<T> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Group, self.name.clone()).disabled(self.disabled)
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

impl<T: 'static> Resizable<T> {
    pub fn show(self, hosted: &Hosted<T>) -> El {
        let paint = style::paint(hosted);
        let limits: Rc<Vec<Limits>> = Rc::new(self.limits());
        let first = initial(&limits);
        let (sizes, hovered, dragging, length) = match hosted.ui.resize(&self.id) {
            Some(slot) if slot.sizes.len() == limits.len() => (
                slot.sizes.clone(),
                slot.hovered,
                slot.start.as_ref().map(|(handle, _)| *handle),
                slot.length,
            ),
            // A group given other panels starts again from their sizes.
            Some(slot) => (first.clone(), None, None, slot.length),
            None => (first.clone(), None, None, 0.0),
        };
        let axis = self.axis;
        let horizontal = axis == Axis::Horizontal;
        let count = self.panels.len();
        let thickness = paint.tokens.resize_handle(States::NONE).thickness;
        let rooms = thickness * count.saturating_sub(1) as f32;
        let cursor = if horizontal {
            Cursor::ResizeColumn
        } else {
            Cursor::ResizeRow
        };
        let enabled = !self.disabled;
        let accessibles = handle_accessibles(&self.name, &sizes, self.disabled);

        let id = self.id.clone();
        let measure_id = self.id.clone();
        let mut group = if horizontal { row() } else { col() }
            .key(self.id.clone())
            .w_full()
            .h_full()
            .min_w(0.0)
            .min_h(0.0)
            // A drag follows the pointer exactly, and a key lands the step at once.
            .instant_changes()
            .on_layout(move |hosted: &mut Hosted<T>, rect: [f32; 4]| {
                let along = if horizontal { rect[2] } else { rect[3] };
                if (hosted.ui.resize(&measure_id).map_or(0.0, |s| s.length) - along).abs() > 0.5 {
                    hosted.ui.resize_mut(&measure_id).length = along;
                }
            });
        apply_group(&mut group, &self.name, self.disabled);

        let mut handles = Vec::new();
        let mut before = 0.0;
        for (i, panel) in self.panels.into_iter().enumerate() {
            let size = sizes.get(i).copied().unwrap_or(0.0);
            let mut cell = col()
                .key(format!("{id}-panel-{i}"))
                .grow(size)
                .min_w(0.0)
                .min_h(0.0);
            // A zero size to grow from, so the panels share the group by their fractions and not
            // by what their content asks for.
            cell = if horizontal {
                cell.w(0.0).h_full()
            } else {
                cell.h(0.0).w_full()
            };
            for child in panel.content {
                cell = cell.child(child);
            }
            group = group.child(cell);
            before += size;
            if i + 1 < count {
                // The room the handle takes; its line and target are drawn over the panels.
                let room = el();
                group = group.child(if horizontal {
                    room.w(thickness).h_full()
                } else {
                    room.h(thickness).w_full()
                });
                handles.push((i, before));
            }
        }

        let avail = (length - rooms).max(0.0);
        for (i, fraction) in handles {
            let states = States::NONE
                .hovered(hovered == Some(i))
                .pressed(dragging == Some(i))
                .disabled(!enabled);
            let s = paint.tokens.resize_handle(states);
            // Where the handle's room starts along the group: the panels before it, and the
            // rooms of the handles before it.
            let start = fraction * avail + i as f32 * thickness;
            let at = start + (thickness - s.target) / 2.0;
            let line = el().fill(ink(s.line));
            let line = if horizontal {
                line.w(s.drawn).h_full()
            } else {
                line.h(s.drawn).w_full()
            };
            let mut target = apply(
                el().key(format!("{id}-handle-{i}")),
                accessibles
                    .get(i)
                    .unwrap_or(&Accessible::new(Role::Separator, "")),
            )
            .center()
            .child(line);
            target = if horizontal {
                target.w(s.target).pin(Some(at), Some(0.0), None, Some(0.0))
            } else {
                target.h(s.target).pin(Some(0.0), Some(at), Some(0.0), None)
            };
            if enabled {
                let (hover_id, drag_id, key_id) = (id.clone(), id.clone(), id.clone());
                let (drag_limits, key_limits) = (limits.clone(), limits.clone());
                let (drag_report, key_report) = (self.on_layout.clone(), self.on_layout.clone());
                target = focus_ring(target, &s.focus)
                    .focusable(true)
                    .cursor(cursor)
                    .on_hover_change(move |hosted: &mut Hosted<T>, on: bool, _: &mut Tx| {
                        let slot = hosted.ui.resize_mut(&hover_id);
                        if on {
                            slot.hovered = Some(i);
                        } else if slot.hovered == Some(i) {
                            slot.hovered = None;
                        }
                    })
                    .on_drag(move |hosted: &mut Hosted<T>, drag: &mut Drag, _: &mut Tx| {
                        let slot = hosted.ui.resize_mut(&drag_id);
                        // The group has not been changed yet: it is at its initial sizes.
                        if slot.sizes.len() != drag_limits.len() {
                            slot.sizes = initial(&drag_limits);
                        }
                        match drag.phase {
                            Phase::Down => slot.start = Some((i, slot.sizes.clone())),
                            Phase::Move => {
                                let Some((handle, start)) = slot.start.clone() else {
                                    return;
                                };
                                let room = slot.length - rooms;
                                if room <= 0.0 || drag.cancelled() {
                                    return;
                                }
                                let travel = if horizontal { drag.dx } else { drag.dy };
                                let next = moved(&start, &drag_limits, handle, travel / room);
                                if next != slot.sizes {
                                    slot.sizes = next;
                                }
                            }
                            Phase::Up => {
                                let Some((_, start)) = slot.start.take() else {
                                    return;
                                };
                                if drag.cancelled() {
                                    // Escape puts the sizes back.
                                    slot.sizes = start;
                                    return;
                                }
                                let sizes = slot.sizes.clone();
                                if let Some(report) = &drag_report {
                                    report(&mut hosted.app, &sizes);
                                }
                            }
                        }
                    })
                    .on_key(move |hosted: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                        if ev.mods.cmd
                            || ev.mods.ctrl
                            || ev.mods.alt
                            || ev.mods.logo
                            || ev.mods.shift
                        {
                            return false;
                        }
                        let (back, forward) = if horizontal {
                            (Key::Left, Key::Right)
                        } else {
                            (Key::Up, Key::Down)
                        };
                        let sizes = match hosted.ui.resize(&key_id) {
                            Some(slot) if slot.sizes.len() == key_limits.len() => {
                                slot.sizes.clone()
                            }
                            _ => initial(&key_limits),
                        };
                        let next = if ev.key == back {
                            moved(&sizes, &key_limits, i, -KEY_STEP)
                        } else if ev.key == forward {
                            moved(&sizes, &key_limits, i, KEY_STEP)
                        } else if ev.key == Key::Home {
                            to(&sizes, i, bounds(&sizes, &key_limits, i).0)
                        } else if ev.key == Key::End {
                            to(&sizes, i, bounds(&sizes, &key_limits, i).1)
                        } else {
                            return false;
                        };
                        if next != sizes {
                            hosted.ui.resize_mut(&key_id).sizes = next.clone();
                            if let Some(report) = &key_report {
                                report(&mut hosted.app, &next);
                            }
                        }
                        true
                    });
            }
            group = group.child(target);
        }
        group
    }
}

/// The group's own role and name, for the tree.
fn apply_group(group: &mut El, name: &str, disabled: bool) {
    let taken = std::mem::replace(group, el());
    *group = apply(
        taken,
        &Accessible::new(Role::Group, name).disabled(disabled),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(spec: &[(Option<f32>, f32, f32)]) -> Vec<Limits> {
        spec.iter()
            .map(|&(default, min, max)| Limits { default, min, max })
            .collect()
    }

    fn near(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn panels_without_a_default_share_what_the_others_leave() {
        let sizes = initial(&limits(&[
            (Some(0.4), 0.0, 1.0),
            (None, 0.0, 1.0),
            (None, 0.0, 1.0),
        ]));
        assert!(near(&sizes, &[0.4, 0.3, 0.3]), "{sizes:?}");
    }

    #[test]
    fn sizes_always_come_to_the_whole_group_within_their_limits() {
        let l = limits(&[(Some(0.9), 0.0, 0.5), (Some(0.9), 0.2, 0.5)]);
        let sizes = initial(&l);
        assert!((sizes.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{sizes:?}");
        assert!(
            sizes
                .iter()
                .zip(&l)
                .all(|(s, l)| *s >= l.min && *s <= l.max)
        );
    }

    #[test]
    fn nonsense_limits_are_made_sane() {
        let l = sane(&limits(&[
            (Some(f32::NAN), f32::NAN, 0.2),
            (Some(2.0), 0.6, 0.1),
        ]));
        assert_eq!((l[0].min, l[0].max, l[0].default), (0.0, 0.2, None));
        assert_eq!((l[1].min, l[1].max, l[1].default), (0.6, 0.6, Some(1.0)));
    }

    #[test]
    fn a_handle_moves_the_pair_and_keeps_their_sum_within_both_limits() {
        let l = limits(&[(Some(0.4), 0.25, 0.7), (None, 0.3, 1.0)]);
        let start = initial(&l);
        assert!(near(&moved(&start, &l, 0, 0.1), &[0.5, 0.5]));
        assert!(
            near(&moved(&start, &l, 0, 1.0), &[0.7, 0.3]),
            "the detail keeps its minimum"
        );
        assert!(
            near(&moved(&start, &l, 0, -1.0), &[0.25, 0.75]),
            "the list keeps its minimum"
        );
    }

    #[test]
    fn home_and_end_are_the_bounds_of_the_panel_before_the_handle() {
        let l = limits(&[(Some(0.4), 0.25, 0.7), (None, 0.3, 1.0)]);
        let start = initial(&l);
        assert_eq!(bounds(&start, &l, 0), (0.25, 0.7));
    }

    #[test]
    fn each_handle_reports_the_size_of_the_panel_before_it() {
        let a = handle_accessibles("Split", &[0.4, 0.35, 0.25], false);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].value.as_deref(), Some("40%"));
        assert_eq!(a[1].value.as_deref(), Some("35%"));
        assert!(handle_accessibles("Split", &[1.0], false).is_empty());
    }
}
