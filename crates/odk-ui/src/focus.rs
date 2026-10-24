//! Keyboard focus, shared by every focusable component (`docs/spec/odk-ui-components.md`,
//! "Focus and keyboard"). Tab and Shift+Tab move between tab stops; [`crate::run`] binds and
//! handles them. Composite controls such as tabs use a roving tab stop:
//! one item is in the Tab order and the arrow keys move between items.

use gpui::{App, ElementId, FocusHandle, SharedString, Window, actions};

actions!(odk, [FocusNext, FocusPrevious]);

/// The id of a part of a component, derived from the component's own id.
pub(crate) fn child_id(parent: &ElementId, part: impl Into<SharedString>) -> ElementId {
    ElementId::NamedChild(Box::new(parent.clone()), part.into())
}

/// The name tests use to find a part of a component (GPUI's `debug_selector`, which does
/// nothing in release builds): `"<id>/<part>"` for a component with a name for its id.
pub(crate) fn part_name(id: &ElementId, part: &str) -> String {
    format!("{}/{part}", id_path(id))
}

/// An id as a path: a part's id inside another component's reads `"file/button"`.
pub(crate) fn id_path(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => name.to_string(),
        ElementId::NamedChild(parent, name) => format!("{}/{name}", id_path(parent)),
        ElementId::Integer(i) => i.to_string(),
        ElementId::NamedInteger(name, i) => format!("{name}-{i}"),
        other => format!("{other:?}"),
    }
}

/// A focus handle that lives as long as the element with this id keeps being rendered.
/// `tab_stop` puts it in the Tab order. GPUI takes the Tab order from the handle itself, not
/// from the element's `tab_index` and `tab_stop`, once the element tracks a handle of its own.
pub(crate) fn focus_handle(
    id: ElementId,
    tab_stop: bool,
    window: &mut Window,
    cx: &mut App,
) -> FocusHandle {
    #[cfg(debug_assertions)]
    let named = id.clone();
    window
        .use_keyed_state(id, cx, move |_, cx| {
            let handle = cx.focus_handle();
            #[cfg(debug_assertions)]
            NAMES.with(|n| {
                n.borrow_mut()
                    .push((handle.downgrade(), id_path(&named).into()))
            });
            handle
        })
        .read(cx)
        .clone()
        .tab_stop(tab_stop)
}

#[cfg(debug_assertions)]
thread_local! {
    /// The id each focus handle was made for, so that [`focused_component`] can say which
    /// control has focus. Written once when a control's handle is made, and pruned when one is
    /// looked up and its control has gone.
    static NAMES: std::cell::RefCell<Vec<(gpui::WeakFocusHandle, SharedString)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The id of the control that has keyboard focus in `window`, as its part's path: the id the
/// application gave the component, then the part, as in `"save/focus"`. For diagnostics and
/// event logs (the gallery's Interaction page). `None` when nothing has focus, when what has it
/// is not an ODK control, and always in a release build, which records no names so that making
/// a control costs nothing for them.
pub fn focused_component(window: &Window, cx: &App) -> Option<SharedString> {
    #[cfg(debug_assertions)]
    {
        let focused = window.focused(cx)?;
        NAMES.with(|n| {
            let mut names = n.borrow_mut();
            names.retain(|(handle, _)| handle.upgrade().is_some());
            names
                .iter()
                .find(|(handle, _)| handle.upgrade().as_ref() == Some(&focused))
                .map(|(_, name)| name.clone())
        })
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (window, cx);
        None
    }
}

/// Gives `handle` focus and redraws, so the focus ring moves with it; GPUI does not redraw
/// on a focus change by itself.
pub(crate) fn move_focus(handle: &FocusHandle, window: &mut Window) {
    handle.focus(window);
    window.refresh();
}

/// Which way a row or column of items runs, for the arrow keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    Horizontal,
    Vertical,
}

/// Where a roving tab stop goes when `key` is pressed on item `current` of `len`: the
/// neighbour along `axis`, wrapping at the ends, or the first or last for Home and End.
pub(crate) fn roving_target(key: &str, current: usize, len: usize, axis: Axis) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let (back, forward) = match axis {
        Axis::Horizontal => ("left", "right"),
        Axis::Vertical => ("up", "down"),
    };
    match key {
        k if k == forward => Some((current + 1) % len),
        k if k == back => Some((current + len - 1) % len),
        "home" => Some(0),
        "end" => Some(len - 1),
        _ => None,
    }
}

/// Like [`roving_target`], over only the items in `enabled` (item indices, in order):
/// where focus goes from item `current`, skipping disabled items.
pub(crate) fn roving_among(
    key: &str,
    current: usize,
    enabled: &[usize],
    axis: Axis,
) -> Option<usize> {
    let at = enabled.iter().position(|e| *e == current)?;
    roving_target(key, at, enabled.len(), axis).map(|next| enabled[next])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_wrap_and_home_end_jump() {
        let h = Axis::Horizontal;
        assert_eq!(roving_target("right", 0, 3, h), Some(1));
        assert_eq!(roving_target("right", 2, 3, h), Some(0));
        assert_eq!(roving_target("left", 0, 3, h), Some(2));
        assert_eq!(roving_target("home", 2, 3, h), Some(0));
        assert_eq!(roving_target("end", 0, 3, h), Some(2));
        assert_eq!(roving_target("down", 0, 3, h), None);
        assert_eq!(roving_target("right", 0, 0, h), None);
        assert_eq!(roving_target("down", 0, 3, Axis::Vertical), Some(1));
    }

    #[test]
    fn disabled_items_are_skipped() {
        let h = Axis::Horizontal;
        // Items 0, 2 and 3 are enabled; 1 is disabled.
        let enabled = [0, 2, 3];
        assert_eq!(roving_among("right", 0, &enabled, h), Some(2));
        assert_eq!(roving_among("left", 2, &enabled, h), Some(0));
        assert_eq!(roving_among("right", 3, &enabled, h), Some(0));
        assert_eq!(roving_among("end", 0, &enabled, h), Some(3));
        assert_eq!(
            roving_among("right", 1, &enabled, h),
            None,
            "a disabled item has no focus"
        );
    }
}
