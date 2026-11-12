//! `Stack`, `Row` and `Grid`, the layout groups (ADR 0088) on the Rune host: spacing comes from
//! the foundation's steps at the user's text size, never from a number an application writes.
//!
//! Rune's `El` is already a builder, so a group is an `El` this hands out, not a type of its
//! own. [`Layout`] reads the theme once; its methods are the only way a component or an
//! application makes a group, and a [`Space`] is the only gap it takes.
//!
//! Not carried over from `odk-ui`: a page column is not centred when its parent is wider (Rune
//! has no auto margin), recorded in `docs/checks.md`. A grid with a `wide` cell is rows of equal
//! columns, since Rune places a cell only in the first row of one grid (`El::grid_cell`).

use odk_theme::component::SidebarStyle;
use odk_theme::foundation::{LayoutTokens, SpacingTokens};
use rune::{Cx, El, col, el, row};

use crate::host::Hosted;
use crate::ink::ink;
use crate::style;

/// `element` as wide as its content and no wider than its column, where a column holds it: a
/// badge or a status in a stack. Rune's `max_w` takes points, not a share of the parent, so the
/// element sits in a row, which stretches across a column and lets a `shrink` element narrow
/// along it (`odk-ui`'s `max_w_full`).
pub(crate) fn inline(element: El) -> El {
    row().shrink().child(element)
}

/// A step of the foundation's spacing scale: 4, 8, 12, 16 and 24 points at the base text size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

/// The foundation's spacing and layout at the window's text size.
#[derive(Debug, Clone)]
pub struct Layout {
    spacing: SpacingTokens,
    layout: LayoutTokens,
    sidebar: SidebarStyle,
}

/// A cell of a grid: in the next column, or `wide`, on a row of its own across every column, such
/// as a chart or a table that needs the room.
pub struct Cell {
    element: El,
    wide: bool,
}

impl Cell {
    pub fn new(element: El) -> Self {
        Self {
            element,
            wide: false,
        }
    }

    pub fn wide(element: El) -> Self {
        Self {
            element,
            wide: true,
        }
    }
}

impl From<El> for Cell {
    fn from(element: El) -> Self {
        Self::new(element)
    }
}

/// A page's column that holds a grid, with the count of columns the grid gets: see
/// [`Layout::content_column`].
pub struct ContentColumn {
    columns: u16,
    outer: El,
    inner: El,
}

impl ContentColumn {
    /// How many columns the grid in this column gets.
    pub fn columns(&self) -> u16 {
        self.columns
    }

    /// A child of the column.
    pub fn child(mut self, child: El) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = El>) -> Self {
        self.inner = self.inner.children(children);
        self
    }

    pub fn into_el(self) -> El {
        self.outer.child(self.inner)
    }
}

impl Layout {
    /// The window's spacing. This is a read of the theme, so it is made once per view.
    pub fn of<T>(hosted: &Hosted<T>) -> Self {
        let paint = style::paint(hosted);
        let foundation = paint.tokens.foundation();
        Self {
            spacing: foundation.spacing.clone(),
            layout: foundation.layout.clone(),
            sidebar: paint.tokens.sidebar(),
        }
    }

    /// `space` in points.
    pub fn space(&self, space: Space) -> f32 {
        match space {
            Space::Xs => self.spacing.s1,
            Space::Sm => self.spacing.s2,
            Space::Md => self.spacing.s3,
            Space::Lg => self.spacing.s4,
            Space::Xl => self.spacing.s6,
        }
    }

    /// Elements one under the next, as wide as the widest, `gap` apart.
    pub fn stack(&self, gap: Space) -> El {
        col().gap(self.space(gap))
    }

    /// Elements side by side and centred across, `gap` apart.
    pub fn row(&self, gap: Space) -> El {
        row().center_y().gap(self.space(gap))
    }

    /// Elements side by side, as tall as the tallest, `gap` apart: the frame of a sidebar and its
    /// content, whose two parts fill the height. A plain [`Self::row`] centres across instead.
    pub fn row_stretch(&self, gap: Space) -> El {
        row().items_stretch().gap(self.space(gap))
    }

    /// The frame of an application with a sidebar: the sidebar at the foundation's width on the
    /// recessed surface with its content inset by the sidebar padding, a line, and the content
    /// taking the rest. It fills its parent, so put it where the page goes.
    pub fn sidebar(&self, sidebar: El, content: El) -> El {
        let s = &self.sidebar;
        row()
            .key("odk-sidebar-frame")
            .w_full()
            .h_full()
            .items_stretch()
            .child(
                col()
                    .key("odk-sidebar")
                    .w(s.width)
                    .padding(s.padding)
                    .fill(ink(s.surface))
                    .child(sidebar),
            )
            .child(el().w(s.rule_width).fill(ink(s.rule)))
            .child(col().grow(1.0).min_w(0.0).child(content))
    }

    /// A page's column: as wide as its parent up to the foundation's content width, with the
    /// panel padding and the gap between sections.
    pub fn page(&self) -> El {
        col()
            .w_full()
            .max_w(self.layout.content_width)
            .padding(self.layout.panel_padding)
            .gap(self.layout.section_gap)
    }

    /// [`Self::page`] at the foundation's wide content width, for a page that holds a grid's
    /// columns side by side.
    pub fn wide_page(&self) -> El {
        self.page().max_w(self.layout.wide_content_width)
    }

    /// `cells` in equal columns, as many as fit `key`'s own width at the foundation's column
    /// width, and no more than `max_columns`. `key` names the grid, which this gives; the
    /// view runs again when the count changes, not on every pixel of a resize.
    pub fn grid<T: 'static>(
        &self,
        cx: &Cx<Hosted<T>>,
        key: &str,
        max_columns: u16,
        cells: impl IntoIterator<Item = El>,
    ) -> El {
        let cells: Vec<El> = cells.into_iter().collect();
        let count = cells.len();
        let (column, gap) = (self.layout.grid_column_width, self.layout.section_gap);
        let columns = cx
            .layout_with(key, move |r| {
                grid_columns(r[2], column, gap, max_columns, count)
            })
            .unwrap_or(1);
        row()
            .key(key)
            .w_full()
            .equal_columns(usize::from(columns))
            .gap(gap)
            .children(cells)
    }

    /// [`Self::grid`] for cells some of which are [`Cell::wide`]: rows of `columns` equal cells,
    /// and each wide cell on a row of its own across them. `columns` is what the page's
    /// [`ContentColumn`] gave, or any count.
    pub fn grid_of(&self, key: &str, columns: u16, cells: impl IntoIterator<Item = Cell>) -> El {
        let gap = self.layout.section_gap;
        let columns = usize::from(columns.max(1));
        let mut rows = col().key(key).w_full().gap(gap);
        let mut open: Vec<El> = Vec::new();
        let close = |rows: El, open: &mut Vec<El>| {
            if open.is_empty() {
                return rows;
            }
            rows.child(
                row()
                    .w_full()
                    .equal_columns(columns)
                    .gap(gap)
                    .children(std::mem::take(open)),
            )
        };
        for cell in cells {
            if cell.wide {
                rows = close(rows, &mut open);
                rows = rows.child(row().w_full().child(cell.element.w_full()));
            } else {
                open.push(cell.element);
                if open.len() == columns {
                    rows = close(rows, &mut open);
                }
            }
        }
        close(rows, &mut open)
    }

    /// A page's column for a grid of `cells` cells (those that are not wide) in up to
    /// `max_columns` columns: as wide as its parent up to the foundation's content width, and once
    /// its parent has room for two of the grid's columns, up to the wide content width, so a page
    /// of groups sits two side by side on a wide window and one under another on a narrow one.
    /// `key` names it, and the count follows the width Rune measured on the last layout, so the
    /// view runs again when the count changes and not on every pixel of a resize.
    pub fn content_column<T: 'static>(
        &self,
        cx: &Cx<Hosted<T>>,
        key: &str,
        max_columns: u16,
        cells: usize,
    ) -> ContentColumn {
        let (column, gap) = (self.layout.grid_column_width, self.layout.section_gap);
        let padding = self.layout.panel_padding;
        let columns = cx
            .layout_with(key, move |r| {
                grid_columns(r[2] - 2.0 * padding, column, gap, max_columns, cells)
            })
            .unwrap_or(1);
        let inner = if columns >= 2 {
            self.wide_page()
        } else {
            self.page()
        };
        // The outer element is the room the parent gives; the inner one is what the page takes of
        // it. The count is read from the outer one, so a page at its narrow width can still grow.
        ContentColumn {
            columns,
            outer: col().key(key).w_full(),
            inner,
        }
    }
}

/// How many columns of `column` points fit in `width` with `gap` between them: at least one, and
/// no more than `max_columns` or the `cells` there are, so a lone cell keeps the whole width.
pub(crate) fn grid_columns(
    width: f32,
    column: f32,
    gap: f32,
    max_columns: u16,
    cells: usize,
) -> u16 {
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

#[cfg(test)]
mod tests {
    use super::grid_columns;

    const COLUMN: f32 = 520.0;
    const GAP: f32 = 24.0;

    #[test]
    fn a_grid_has_as_many_columns_as_fit() {
        // n columns take n widths and n - 1 gaps.
        assert_eq!(grid_columns(519.0, COLUMN, GAP, 3, 9), 1);
        assert_eq!(grid_columns(520.0, COLUMN, GAP, 3, 9), 1);
        assert_eq!(grid_columns(2.0 * COLUMN + GAP - 1.0, COLUMN, GAP, 3, 9), 1);
        assert_eq!(grid_columns(2.0 * COLUMN + GAP, COLUMN, GAP, 3, 9), 2);
        assert_eq!(grid_columns(3.0 * COLUMN + 2.0 * GAP, COLUMN, GAP, 3, 9), 3);
    }

    #[test]
    fn a_grid_has_no_more_than_the_most_asked_or_the_cells_there_are() {
        assert_eq!(grid_columns(5000.0, COLUMN, GAP, 2, 9), 2);
        assert_eq!(grid_columns(5000.0, COLUMN, GAP, 9, 1), 1);
        assert_eq!(grid_columns(5000.0, COLUMN, GAP, 0, 0), 1);
    }
}
