//! Text drawn as a link in a path, on the nav link's component tokens
//! (`docs/spec/odk-component-tokens.md`): a `Breadcrumb`'s links and the `…` that opens its folded
//! middle, a `DropdownMenu`'s link trigger.

use gpui::{
    App, Div, ElementId, InteractiveElement, ParentElement, SharedString, Stateful, Styled, Window,
    div,
};
use odk_theme::component::{LinkStyle, States};

use crate::components;
use crate::focus::{child_id, focus_handle, part_name};
use crate::interaction::{RingTarget, link_states};
use crate::layout::StyledExt;

/// Text drawn as a link: secondary text, primary and underlined under the pointer, a tab stop.
/// Its focus ring is painted by the path around it, from `ring`, since a path clips its links
/// where they truncate. The caller adds the click.
pub(crate) fn link(
    id: &ElementId,
    label: SharedString,
    ring: &RingTarget,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let focus = focus_handle(child_id(id, "focus"), true, window, cx);
    let s: LinkStyle = components().nav_link(States::NONE.focused(focus.is_focused(window)));
    #[cfg(test)]
    crate::drawn::record(&part_name(id, "link"), s);
    let mut el = div()
        .id(id.clone())
        .debug_selector(|| part_name(id, "link"))
        .relative()
        .track_focus(&focus)
        .cursor_pointer()
        .text_role(s.type_role);
    // Last, so hover and press keep the type role.
    el = link_states(el, &s);
    if s.focus_visible {
        el = el.child(ring.probe());
    }
    el.child(label)
}
