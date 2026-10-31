//! `Stack` and `Row`, the two ways to group elements in a line (ADR 0088): one under the next, and
//! side by side. Each is a `Div` that was told its direction, so every method a `Div` has works on
//! it (`.child`, `.gap(Space)`, `.w(px)`, `.debug_selector`, `.on_key_down`, `.when`), and `.id()`
//! gives a `Stateful` of it that an overlay accepts. GPUI's own `Stateful` wraps an element the
//! same way: the wrapper implements the element traits by handing each to what it holds.

use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, Interactivity, IntoElement, LayoutId, ParentElement, Pixels,
    StyleRefinement, Styled, Window, div, px,
};

use crate::theme::foundation;

/// What a group is as an element: a `Div` whose direction was set when it was made. The macro
/// writes it once for both, so a group cannot be more or less than the other.
macro_rules! group {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        pub struct $name(Div);

        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                self.0.style()
            }
        }

        impl InteractiveElement for $name {
            fn interactivity(&mut self) -> &mut Interactivity {
                self.0.interactivity()
            }
        }

        impl ParentElement for $name {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.0.extend(elements)
            }
        }

        impl IntoElement for $name {
            type Element = Self;

            fn into_element(self) -> Self::Element {
                self
            }
        }

        impl Element for $name {
            type RequestLayoutState = <Div as Element>::RequestLayoutState;
            type PrepaintState = <Div as Element>::PrepaintState;

            fn id(&self) -> Option<ElementId> {
                Element::id(&self.0)
            }

            fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
                Element::source_location(&self.0)
            }

            fn request_layout(
                &mut self,
                id: Option<&GlobalElementId>,
                inspector_id: Option<&InspectorElementId>,
                window: &mut Window,
                cx: &mut App,
            ) -> (LayoutId, Self::RequestLayoutState) {
                self.0.request_layout(id, inspector_id, window, cx)
            }

            fn prepaint(
                &mut self,
                id: Option<&GlobalElementId>,
                inspector_id: Option<&InspectorElementId>,
                bounds: Bounds<Pixels>,
                request_layout: &mut Self::RequestLayoutState,
                window: &mut Window,
                cx: &mut App,
            ) -> Self::PrepaintState {
                self.0
                    .prepaint(id, inspector_id, bounds, request_layout, window, cx)
            }

            fn paint(
                &mut self,
                id: Option<&GlobalElementId>,
                inspector_id: Option<&InspectorElementId>,
                bounds: Bounds<Pixels>,
                request_layout: &mut Self::RequestLayoutState,
                prepaint: &mut Self::PrepaintState,
                window: &mut Window,
                cx: &mut App,
            ) {
                self.0.paint(
                    id,
                    inspector_id,
                    bounds,
                    request_layout,
                    prepaint,
                    window,
                    cx,
                )
            }
        }
    };
}

group! {
    /// Elements set one under the next, the only vertical group in an application or a
    /// component. Space them with `.gap(Space::..)`; `Stack::page()` is a page's column.
    Stack
}

group! {
    /// Elements set side by side and centred across, a [`Stack`]'s horizontal counterpart. A Row
    /// that must not centre its children across, as the frame of an application with a sidebar,
    /// is [`Row::stretch`].
    Row
}

impl Stack {
    /// An empty Stack: its children in a column with no gap, as wide as they are.
    pub fn new() -> Self {
        Self(div().flex().flex_col())
    }

    /// A page's column: as wide as its parent up to the foundation's content width, centred when
    /// the parent is wider, with the panel padding and the gap between sections.
    pub fn page() -> Self {
        let layout = &foundation().layout;
        Self::new()
            .w_full()
            .max_w(px(layout.content_width))
            .mx_auto()
            .p(px(layout.panel_padding))
            .gap(px(layout.section_gap))
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    /// An empty Row: its children in a line with no gap, centred across.
    pub fn new() -> Self {
        Self(div().flex().flex_row().items_center())
    }

    /// A Row whose children are as tall as it is, not centred across it.
    pub fn stretch() -> Self {
        let mut row = Self::new();
        row.style().align_items = None;
        row
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

/// A group whose direction is only known while drawing, such as a resizable group or a
/// placement that is beside its anchor or below it, or one that is a group in one case and a plain
/// block in the other, as a context menu's wrapper is when it fills its parent. It sets the
/// direction and nothing else, so the caller sets how its children sit across it. The one place a plain `Div` is given a
/// direction outside `Stack` and `Row` (checklist item 43 allows it in this file only).
pub(crate) fn flow(horizontal: bool) -> Div {
    if horizontal {
        div().flex().flex_row()
    } else {
        div().flex().flex_col()
    }
}
