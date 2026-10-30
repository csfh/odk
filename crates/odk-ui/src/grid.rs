//! A grid of equal columns, and the page column that decides how many there are
//! (`docs/spec/odk-ui-components.md`, the layout helpers).

use std::collections::HashMap;

use gpui::{
    AnyElement, AnyWindowHandle, App, Div, ElementId, Global, IntoElement, ParentElement, Styled,
    Window, WindowId, canvas, div, px,
};

use crate::layout::v_flex;
use crate::theme::foundation;

/// How many columns of the foundation's grid column width fit in `width` with the section gap
/// between them: at least one, and no more than `max_columns` or the `cells` there are, so a lone
/// cell keeps the whole width.
pub(crate) fn grid_columns(width: f32, max_columns: u16, cells: usize) -> u16 {
    let layout = &foundation().layout;
    let (column, gap) = (layout.grid_column_width, layout.section_gap);
    let most = max_columns
        .min(u16::try_from(cells).unwrap_or(u16::MAX))
        .max(1);
    // n columns take n widths and n - 1 gaps, so n fit while width + gap >= n * (column + gap).
    let fit = ((width + gap) / (column + gap)).floor();
    if fit >= f32::from(most) {
        most
    } else if fit >= 1.0 {
        fit as u16
    } else {
        1
    }
}

/// Its children in `columns` equal columns, row after row, with the section gap between them
/// both ways, as Tailwind's `grid grid-cols-2 gap-6` lays out a shadcn/ui page. A child added
/// with `wide` takes a row of its own across every column. A row is as tall as its tallest cell.
/// A page asks `content_column_for_grid` how many columns its width holds.
pub struct Grid {
    columns: u16,
    children: Vec<AnyElement>,
}

impl Grid {
    pub fn new(columns: u16) -> Self {
        Self {
            columns: columns.max(1),
            children: Vec::new(),
        }
    }

    /// A child on a row of its own, across every column: a chart or a table that needs the room.
    pub fn wide(mut self, child: impl IntoElement) -> Self {
        self.children
            .push(div().col_span_full().child(child).into_any_element());
        self
    }
}

/// What `child` adds: a cell in the next column.
impl ParentElement for Grid {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl IntoElement for Grid {
    type Element = Div;

    fn into_element(self) -> Div {
        div()
            .grid()
            .grid_cols(self.columns)
            .w_full()
            .gap(px(foundation().layout.section_gap))
            .children(self.children)
    }
}

/// A page's column of content that holds a `Grid`, which `content_column_for_grid` makes. It is
/// `content_column` until two of the grid's columns fit, and then grows to the foundation's wide
/// content width. `columns` is how many columns the grid gets.
pub struct ContentColumn {
    key: (WindowId, ElementId),
    window: AnyWindowHandle,
    max_columns: u16,
    cells: usize,
    columns: u16,
    children: Vec<AnyElement>,
}

/// A page's column for a grid of `cells` cells (its children that are not `wide`) in up to
/// `max_columns` columns, as wide as its parent up to the foundation's content width, centred.
/// Once its parent has room for two of the grid's columns it grows to the wide content width,
/// so a page of groups sits two side by side on a wide window and one under another on a narrow
/// one, and a page with one group stays at the content width. Give the grid its `columns`.
///
/// The column measures the room its parent gives it, and the count follows one frame behind:
/// the window is asked to draw again only when the count changes (ADR 0058). The width is kept
/// under `id`, which is unique in the window, for as long as the application runs, so a page
/// that keeps the id as it changes what it shows, or that is shown again after another, is laid
/// out at its count at once.
pub fn content_column_for_grid(
    id: impl Into<ElementId>,
    max_columns: u16,
    cells: usize,
    window: &mut Window,
    cx: &mut App,
) -> ContentColumn {
    let handle = window.window_handle();
    let key = (handle.window_id(), id.into());
    let room = Measured::of(&key, cx).unwrap_or(0.0) - 2.0 * foundation().layout.panel_padding;
    ContentColumn {
        columns: grid_columns(room, max_columns, cells),
        key,
        window: handle,
        max_columns,
        cells,
        children: Vec::new(),
    }
}

/// The width each page column measured last, by window and id. It outlives the column, which is
/// not drawn while its page is not shown.
#[derive(Default)]
struct Measured(HashMap<(WindowId, ElementId), f32>);

impl Global for Measured {}

impl Measured {
    fn of(key: &(WindowId, ElementId), cx: &App) -> Option<f32> {
        cx.try_global::<Self>().and_then(|m| m.0.get(key)).copied()
    }
}

impl ContentColumn {
    /// How many columns the grid in it gets.
    pub fn columns(&self) -> u16 {
        self.columns
    }
}

impl ParentElement for ContentColumn {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl IntoElement for ContentColumn {
    type Element = Div;

    fn into_element(self) -> Div {
        let layout = &foundation().layout;
        let (pad, wide) = (layout.panel_padding, layout.wide_content_width);
        let cap = if self.columns > 1 {
            wide
        } else {
            layout.content_width
        };
        let (key, handle, max_columns, cells, used) = (
            self.key,
            self.window,
            self.max_columns,
            self.cells,
            self.columns,
        );
        // The probe fills the box that is not yet narrowed to the content width, so it measures
        // the room the parent gives, whatever the count it decides.
        let probe = canvas(
            move |bounds, _, cx| {
                let measured = f32::from(bounds.size.width);
                if Measured::of(&key, cx) != Some(measured) {
                    cx.default_global::<Measured>().0.insert(key, measured);
                }
                if grid_columns(measured - 2.0 * pad, max_columns, cells) != used {
                    // Asked while the window draws, a refresh is dropped; after the frame it is
                    // drawn.
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, _| window.refresh());
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0));
        v_flex()
            .w_full()
            .max_w(px(wide))
            .mx_auto()
            .child(probe)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(cap))
                    .mx_auto()
                    .p(px(pad))
                    .gap(px(layout.section_gap))
                    .children(self.children),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::grid_columns;

    // 520 px columns and a 24 px gap, so two need 1064 px and three 1608.
    #[test]
    fn as_many_columns_fit_as_the_width_holds_up_to_the_most_and_the_cells() {
        crate::theme::install_test_theme();
        assert_eq!(grid_columns(1063.0, 2, 2), 1, "two need 1064");
        assert_eq!(grid_columns(1064.0, 2, 2), 2);
        assert_eq!(grid_columns(1607.0, 3, 3), 2);
        assert_eq!(grid_columns(1608.0, 3, 3), 3);
        assert_eq!(grid_columns(5000.0, 2, 9), 2, "never more than the most");
        assert_eq!(grid_columns(5000.0, 4, 1), 1, "a lone cell keeps the width");
        assert_eq!(grid_columns(5000.0, 4, 0), 1, "never fewer than one");
        assert_eq!(grid_columns(100.0, 2, 2), 1, "nor when one does not fit");
        assert_eq!(grid_columns(-40.0, 2, 2), 1, "nor before it is measured");
        assert_eq!(grid_columns(5000.0, 0, 3), 1);
    }
}
