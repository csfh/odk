//! What a check box, a switch and a radio share: their on/off state, controlled or not, and
//! the row that puts the control beside its label, so a click on the label toggles the control
//! and gives it focus.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use odk_theme::Color;
use odk_theme::foundation::TypeRole;

use crate::Icon;
use crate::focus::{child_id, move_focus, part_name};
use crate::icon;
use crate::layout::StyledExt;
use crate::stack::Row;
use crate::theme::colour;

pub(crate) type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>;
pub(crate) type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// The state of an on/off control: whether it is on now, and what changes it to `next`. The
/// uncontrolled state follows a change, then the application hears of it.
pub(crate) fn toggle_state(
    id: &ElementId,
    controlled: Option<bool>,
    default: bool,
    next: impl Fn(bool) -> bool,
    on_change: Option<ChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) -> (bool, Action) {
    let state = window.use_keyed_state(child_id(id, "checked"), cx, |_, _| default);
    let on = controlled.unwrap_or(*state.read(cx));
    let next = next(on);
    let is_controlled = controlled.is_some();
    let change: Action = Rc::new(move |window, cx| {
        if !is_controlled {
            state.update(cx, |v, cx| {
                *v = next;
                cx.notify();
            });
        }
        if let Some(f) = &on_change {
            f(next, window, cx);
        }
    });
    (on, change)
}

/// How a control's label is drawn, from the control's component style.
pub(crate) struct Label {
    pub colour: Color,
    pub role: TypeRole,
    /// Between the control and its label.
    pub gap: f32,
    /// The row takes the width its parent gives it, as a radio group's options do, instead of
    /// its content's. A row that sizes to its content inside a parent that does too is measured
    /// at its label's unwrapped width in GPUI 0.2.2, so a wrapping label would overlap what
    /// follows (`docs/checks.md`).
    pub stretch: bool,
    /// The colour of a wrong choice's mark (`!`) after the label, so it never shows by colour
    /// alone.
    pub mark: Option<Color>,
}

/// The control beside its label. A click on the label runs `change` and gives the control
/// focus; the control handles its own clicks and stops them reaching the row. The row is as
/// wide as its content and never wider than its column, so a long label wraps, with the
/// control centred on its first line.
pub(crate) fn labelled(
    id: &ElementId,
    label: SharedString,
    control: AnyElement,
    look: Label,
    usable: bool,
    focus: FocusHandle,
    change: Action,
) -> AnyElement {
    let f = crate::foundation();
    let line = f.typography.style(look.role).line_height;
    // The row is the pointer target. Pad it when the text line is shorter than the 24 px
    // minimum, so the control stays centred on the first line and the target still meets it.
    let pad = ((f.geometry.target_min - line) / 2.0).max(0.0);
    let text = div()
        .debug_selector(|| part_name(id, "label"))
        .flex_1()
        .min_w_0()
        .text_color(colour(look.colour))
        .child(label);
    let mut row = Row::new()
        .id(child_id(id, "row"))
        .debug_selector(|| part_name(id, "row"))
        .items_start()
        // A floor on the height only. A minimum width would replace the row's content
        // width, and a row beside another control would then shrink to 24 px and wrap.
        .min_h(px(f.geometry.target_min))
        .pt(px(pad))
        .pb(px(pad))
        .gap(px(look.gap))
        .text_role(look.role)
        .child(
            div()
                .h(px(line))
                .flex()
                .flex_shrink_0()
                .items_center()
                .child(control),
        )
        .child(text);
    if let Some(ink) = look.mark {
        row = row.child(
            div()
                .debug_selector(|| part_name(id, "invalid-mark"))
                .flex()
                .flex_shrink_0()
                .items_center()
                .h(px(line))
                .child(icon::still(
                    Icon::Warning,
                    f.typography.style(look.role).size,
                    ink,
                )),
        );
    }
    if usable {
        row = row.cursor_pointer().on_click(move |_, window, cx| {
            change(window, cx);
            move_focus(&focus, window);
        });
    }
    if look.stretch {
        return row.into_any_element();
    }
    row.style().align_self = Some(gpui::AlignSelf::Start);
    row.max_w_full().into_any_element()
}
