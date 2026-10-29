//! A table of an application's rows: a header of column names that sorts, and rows a control's
//! height drawn only while they are in view (ADR 0070).

use std::cmp::Ordering;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Pixels, RenderOnce, ScrollStrategy, SharedString,
    StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, canvas, div, px,
    uniform_list,
};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::States;
use odk_theme::foundation::Density;

use crate::Icon;
use crate::decoration::{self, ClipFit};
use crate::focus::{Axis, child_id, focus_handle, move_focus, part_name, roving_among};
use crate::icon;
use crate::interaction::{Pointer, RingTarget, pointer_states};
use crate::layout::{StyledExt, bounds_probe, h_flex, text_width, truncate_end, v_flex};
use crate::menu::Menu;
use crate::theme::colour;
use crate::{Button, ButtonVariant, ContextMenu, Skeleton, components};

/// How many skeleton rows an empty table shows while it is loading. ODK's own.
const LOADING_ROWS: usize = 3;

/// What a column sorts its rows by: text, compared without regard to case, or a number.
#[derive(Debug, Clone, PartialEq)]
pub enum SortKey {
    Text(SharedString),
    Number(f64),
}

impl SortKey {
    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.total_cmp(b),
            (Self::Text(a), Self::Text(b)) => a
                .to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| a.cmp(b)),
            // Numbers before text, so a column that mixes them still has one order.
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,
        }
    }
}

impl From<f64> for SortKey {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<f32> for SortKey {
    fn from(n: f32) -> Self {
        Self::Number(n.into())
    }
}

impl From<u64> for SortKey {
    fn from(n: u64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<i64> for SortKey {
    fn from(n: i64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<u32> for SortKey {
    fn from(n: u32) -> Self {
        Self::Number(n.into())
    }
}

impl From<usize> for SortKey {
    fn from(n: usize) -> Self {
        Self::Number(n as f64)
    }
}

impl From<SharedString> for SortKey {
    fn from(s: SharedString) -> Self {
        Self::Text(s)
    }
}

impl From<String> for SortKey {
    fn from(s: String) -> Self {
        Self::Text(s.into())
    }
}

impl From<&str> for SortKey {
    fn from(s: &str) -> Self {
        Self::Text(SharedString::from(s.to_owned()))
    }
}

/// Which way a column sorts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn flipped(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    /// The icon beside a sorted column's name, so the order never rests on colour (ADR 0083).
    fn mark(self) -> Icon {
        match self {
            Self::Ascending => Icon::ArrowUp,
            Self::Descending => Icon::ArrowDown,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// How wide a column is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Width {
    /// A share of what the fixed columns leave, by weight.
    Grow(f32),
    /// As wide as this many characters of the table's text, which grows with the text size.
    Chars(u16),
}

type Text<R> = Rc<dyn Fn(&R) -> SharedString>;
type Cell<R> = Rc<dyn Fn(&R, &mut Window, &mut App) -> AnyElement>;
type Press<R> = Rc<dyn Fn(&R, &mut Window, &mut App)>;
type Label<R> = Rc<dyn Fn(&R) -> Option<SharedString>>;
type SortBy<R> = Rc<dyn Fn(&R) -> SortKey>;
type KeyOf<R> = Rc<dyn Fn(&R) -> SharedString>;
type Depth<R> = Rc<dyn Fn(&R) -> usize>;
/// What a table does when a header is chosen, by the column's place.
type ChooseSort = Rc<dyn Fn(usize, &mut Window, &mut App)>;
/// What a table does when a row is selected, by its key.
type Select = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

enum Content<R> {
    Text(Text<R>),
    Element(Cell<R>),
    Action { label: Label<R>, on_press: Press<R> },
}

/// One column of a [`DataTable`]: its key, its name, how wide it is and what its cells show.
pub struct Column<R> {
    key: SharedString,
    header: SharedString,
    width: Width,
    align_end: bool,
    content: Content<R>,
    sort: Option<SortBy<R>>,
}

impl<R: 'static> Column<R> {
    /// A column named `header` (shown above it), known as `key` (what `on_sort` and `sort`
    /// name), whose cells show `text`. It shares what the other columns leave equally.
    pub fn new(
        key: impl Into<SharedString>,
        header: impl Into<SharedString>,
        text: impl Fn(&R) -> SharedString + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            header: header.into(),
            width: Width::Grow(1.0),
            align_end: false,
            content: Content::Text(Rc::new(text)),
            sort: None,
        }
    }

    /// A column whose cells are elements one row high, such as a badge or a status indicator.
    pub fn cell(
        key: impl Into<SharedString>,
        header: impl Into<SharedString>,
        cell: impl Fn(&R, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            content: Content::Element(Rc::new(cell)),
            ..Self::new(key, header, |_| SharedString::default())
        }
    }

    /// A column of one button per row, labelled by `label` (none for a row without it), that
    /// takes the pointer only: put the same action in the row's menu, which the keyboard
    /// reaches (ADR 0070).
    pub fn action(
        key: impl Into<SharedString>,
        header: impl Into<SharedString>,
        label: impl Fn(&R) -> Option<SharedString> + 'static,
        on_press: impl Fn(&R, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            content: Content::Action {
                label: Rc::new(label),
                on_press: Rc::new(on_press),
            },
            ..Self::new(key, header, |_| SharedString::default())
        }
    }

    /// A share of what the fixed columns leave, by `weight` (1 by default).
    pub fn grow(mut self, weight: f32) -> Self {
        self.width = Width::Grow(if weight.is_finite() {
            weight.max(0.0)
        } else {
            1.0
        });
        self
    }

    /// As wide as `chars` characters of the table's text, such as 7 for a process id.
    pub fn chars(mut self, chars: u16) -> Self {
        self.width = Width::Chars(chars);
        self
    }

    /// Its text at the end of its cells, for numbers.
    pub fn align_end(mut self) -> Self {
        self.align_end = true;
        self
    }

    /// The column sorts its rows by `key` when its header is chosen.
    pub fn sort_by<K: Into<SortKey>>(mut self, key: impl Fn(&R) -> K + 'static) -> Self {
        self.sort = Some(Rc::new(move |row| key(row).into()));
        self
    }

    /// The cell's text, for sorting by default, `Describe` and the tooltip.
    fn text(&self, row: &R) -> SharedString {
        match &self.content {
            Content::Text(text) => text(row),
            Content::Action { label, .. } => label(row).unwrap_or_default(),
            Content::Element(_) => SharedString::default(),
        }
    }
}

type OnSort = Rc<dyn Fn(&str, SortDirection, &mut Window, &mut App)>;
type OnRow = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type RowMenu<R> = Rc<dyn Fn(&R, Menu) -> Menu>;

/// A table of the application's rows. The header names the columns and stays in place; only the
/// rows in view are drawn, each a control's height at the table's density, so a table of
/// thousands of rows costs what the visible ones do (ADR 0070). The application gives every
/// row, already filtered, and a key that names each across updates.
///
/// One row is selected, by its key, and stays selected when the rows are sorted or replaced.
/// A press selects a row; `on_activate` hears Enter, and a press on the selected row when
/// `activate_on_click` is set. `row_menu` gives each row a context menu. With no rows the table
/// shows `empty`, or skeleton rows while `loading`. A cell's text that does not fit ends in `…`
/// and shows the whole of it in a tooltip on hover.
///
/// Keyboard: the sortable headers are one tab stop, where Left and Right move between them and
/// Enter or Space sorts by one, again to reverse it. The rows are the next tab stop: Up and Down
/// move the selection, Home and End to the first and last row, Page Up and Page Down by a page,
/// Enter activates the selected row, and the Menu key or Shift+F10 opens its menu. A button in a
/// cell takes the pointer only; its action is in the row's menu.
///
/// ```ignore
/// DataTable::new("processes", "Processes", rows)
///     .row_key(|p: &Process| p.pid.to_string().into())
///     .column(Column::new("name", "Name", |p: &Process| p.name.clone()).grow(2.0).sort_by(|p| p.name.clone()))
///     .column(Column::new("cpu", "CPU", |p: &Process| format!("{:.1}%", p.cpu).into()).chars(7).align_end().sort_by(|p| p.cpu))
///     .default_sort("cpu", SortDirection::Descending)
///     .row_menu(|p, menu| menu.item(MenuItem::new("End").danger()))
/// ```
#[derive(IntoElement)]
pub struct DataTable<R: 'static> {
    id: ElementId,
    name: SharedString,
    rows: Rc<[R]>,
    columns: Vec<Column<R>>,
    row_key: Option<KeyOf<R>>,
    indent: Option<Depth<R>>,
    sort: Option<(SharedString, SortDirection)>,
    default_sort: Option<(SharedString, SortDirection)>,
    on_sort: Option<OnSort>,
    selected: Option<Option<SharedString>>,
    on_select: Option<OnRow>,
    on_activate: Option<OnRow>,
    activate_on_click: bool,
    row_menu: Option<RowMenu<R>>,
    empty: SharedString,
    loading: bool,
    density: Option<Density>,
}

impl<R: 'static> DataTable<R> {
    /// A table named `name` (such as "Processes") for assistive technology, of `rows`.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        rows: impl Into<Rc<[R]>>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            rows: rows.into(),
            columns: Vec::new(),
            row_key: None,
            indent: None,
            sort: None,
            default_sort: None,
            on_sort: None,
            selected: None,
            on_select: None,
            on_activate: None,
            activate_on_click: false,
            row_menu: None,
            empty: "Nothing to show".into(),
            loading: false,
            density: None,
        }
    }

    pub fn column(mut self, column: Column<R>) -> Self {
        self.columns.push(column);
        self
    }

    /// What names a row across updates, such as a process id, so the selection follows it.
    /// Without one a row is named by its place, and a re-sort moves the selection.
    pub fn row_key(mut self, key: impl Fn(&R) -> SharedString + 'static) -> Self {
        self.row_key = Some(Rc::new(key));
        self
    }

    /// How deep a row is in a tree, which indents its first column.
    pub fn indent(mut self, depth: impl Fn(&R) -> usize + 'static) -> Self {
        self.indent = Some(Rc::new(depth));
        self
    }

    /// The column and direction the table starts sorted by; the table keeps the user's choice.
    pub fn default_sort(mut self, key: impl Into<SharedString>, direction: SortDirection) -> Self {
        self.default_sort = Some((key.into(), direction));
        self
    }

    /// Makes the sort controlled: the table shows `key` in `direction`, and the application
    /// follows `on_sort`.
    pub fn sort(mut self, key: impl Into<SharedString>, direction: SortDirection) -> Self {
        self.sort = Some((key.into(), direction));
        self
    }

    /// Hears the column and direction the user chose.
    pub fn on_sort(
        mut self,
        f: impl Fn(&str, SortDirection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort = Some(Rc::new(f));
        self
    }

    /// Makes the selection controlled: the row named `key` is selected, or none.
    pub fn selected(mut self, key: Option<SharedString>) -> Self {
        self.selected = Some(key);
        self
    }

    /// Hears the key of the row the user selected.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Hears the key of the row the user activated, with Enter.
    pub fn on_activate(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }

    /// A press on the row that is already selected activates it too.
    pub fn activate_on_click(mut self, on: bool) -> Self {
        self.activate_on_click = on;
        self
    }

    /// Each row's context menu, built from the row when it opens.
    pub fn row_menu(mut self, build: impl Fn(&R, Menu) -> Menu + 'static) -> Self {
        self.row_menu = Some(Rc::new(build));
        self
    }

    /// What the table says when it has no rows, such as "No process matches".
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = text.into();
        self
    }

    /// The rows are on their way: an empty table shows skeleton rows instead of `empty`.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }

    fn key_of(&self, i: usize) -> SharedString {
        key_of(&self.row_key, &self.rows, i)
    }

    /// Each column header, with its sort as its value, then each row, with its cells' text,
    /// in the order given.
    pub fn item_accessibles(&self) -> Vec<Accessible> {
        let sort = self.sort.clone().or_else(|| self.default_sort.clone());
        let selected = self.selected.clone().flatten();
        let headers = self.columns.iter().map(|c| {
            let a = Accessible::new(Role::ColumnHeader, c.header.to_string());
            match &sort {
                Some((key, d)) if *key == c.key => a.value(d.name()),
                _ => a,
            }
        });
        let rows = (0..self.rows.len()).map(|i| {
            let row = &self.rows[i];
            Accessible::new(Role::Row, row_text(&self.columns, row))
                .selected(selected.as_ref() == Some(&self.key_of(i)))
        });
        headers.chain(rows).collect()
    }
}

fn key_of<R>(row_key: &Option<KeyOf<R>>, rows: &[R], i: usize) -> SharedString {
    match row_key {
        Some(key) => key(&rows[i]),
        None => SharedString::from(i.to_string()),
    }
}

/// A row's cells' text, for assistive technology.
fn row_text<R: 'static>(columns: &[Column<R>], row: &R) -> String {
    columns
        .iter()
        .map(|c| c.text(row))
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

impl<R: 'static> Describe for DataTable<R> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Table, self.name.to_string()).busy(self.loading)
    }
}

/// The rows' order: as given, or stably sorted by `column` in `direction`.
fn order<R>(rows: &[R], column: Option<&Column<R>>, direction: SortDirection) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    let Some(sort) = column.and_then(|c| c.sort.as_ref()) else {
        return order;
    };
    let keys: Vec<SortKey> = rows.iter().map(|r| sort(r)).collect();
    order.sort_by(|a, b| {
        let o = keys[*a].compare(&keys[*b]);
        match direction {
            SortDirection::Ascending => o,
            SortDirection::Descending => o.reverse(),
        }
    });
    order
}

/// Where a key moves the selection from `current` (a place in the order, if any) among `len`
/// rows, `page` rows to a page.
fn next_row(key: &str, current: Option<usize>, len: usize, page: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let last = len - 1;
    let page = page.max(1);
    Some(match (key, current) {
        ("down", None) | ("home", _) | ("pagedown", None) => 0,
        ("up", None) | ("end", _) | ("pageup", None) => last,
        ("down", Some(i)) => (i + 1).min(last),
        ("up", Some(i)) => i.saturating_sub(1),
        ("pagedown", Some(i)) => (i + page).min(last),
        ("pageup", Some(i)) => i.saturating_sub(page),
        _ => return None,
    })
}

/// What a table keeps between frames.
struct TableState {
    selected: Option<SharedString>,
    sort: Option<(SharedString, SortDirection)>,
    scroll: UniformListScrollHandle,
    /// The row whose menu was asked for.
    menu_row: Option<SharedString>,
    /// Where the rows were laid out on the last frame, for a page's length.
    rows_bounds: Option<Bounds<Pixels>>,
    /// The sortable header the header's tab stop is on.
    header: usize,
}

impl<R: 'static> RenderOnce for DataTable<R> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let c = components();
        let density = c.density(self.density);
        let s = c.table(density);
        let g = s.geometry;
        let default_sort = self.default_sort.clone();
        let state = window.use_keyed_state(child_id(&self.id, "state"), cx, |_, _| TableState {
            selected: None,
            sort: default_sort,
            scroll: UniformListScrollHandle::new(),
            menu_row: None,
            rows_bounds: None,
            header: 0,
        });
        let (own_selected, own_sort, scroll, menu_row, rows_bounds, header_at) = {
            let st = state.read(cx);
            (
                st.selected.clone(),
                st.sort.clone(),
                st.scroll.clone(),
                st.menu_row.clone(),
                st.rows_bounds,
                st.header,
            )
        };
        let selected = self.selected.clone().unwrap_or(own_selected);
        let sort = self.sort.clone().or(own_sort);
        let controlled_sort = self.sort.is_some();
        let controlled_selection = self.selected.is_some();

        let columns: Rc<Vec<Column<R>>> = Rc::new(self.columns);
        let sorted_by = sort
            .as_ref()
            .and_then(|(key, d)| columns.iter().position(|c| c.key == *key).map(|i| (i, *d)));
        let order: Rc<Vec<usize>> = Rc::new(order(
            &self.rows,
            sorted_by.map(|(i, _)| &columns[i]),
            sorted_by.map_or(SortDirection::Ascending, |(_, d)| d),
        ));
        let rows = self.rows.clone();
        let row_key = self.row_key.clone();
        let keys: Rc<Vec<SharedString>> =
            Rc::new(order.iter().map(|i| key_of(&row_key, &rows, *i)).collect());
        let at = selected
            .as_ref()
            .and_then(|k| keys.iter().position(|key| key == k));
        #[cfg(test)]
        crate::drawn::record(
            &crate::focus::id_path(&self.id),
            (keys.as_ref().clone(), selected.clone()),
        );

        // How wide each fixed column is: its characters of the table's text.
        let text_size = px(c.foundation().typography.style(g.type_role).size);
        let char_width = text_width(window, &SharedString::from("0"), text_size);
        let widths: Rc<Vec<Width>> = Rc::new(columns.iter().map(|c| c.width).collect());
        let sized = move |el: gpui::Div, i: usize, widths: &[Width]| -> gpui::Div {
            match widths[i] {
                Width::Chars(n) => el
                    .flex_shrink_0()
                    .w(char_width * f32::from(n) + px(g.padding_x + g.padding_x)),
                Width::Grow(weight) => {
                    let mut el = el.flex_basis(px(0.0)).min_w_0();
                    el.style().flex_grow = Some(weight);
                    el
                }
            }
        };

        // The header: the column names, the sortable ones one tab stop with the arrow keys.
        let sortable: Rc<Vec<usize>> = Rc::new(
            columns
                .iter()
                .enumerate()
                .filter(|(_, c)| c.sort.is_some())
                .map(|(i, _)| i)
                .collect(),
        );
        let header_stop = header_at.min(sortable.len().saturating_sub(1));
        let header_handles: Rc<Vec<FocusHandle>> = Rc::new(
            sortable
                .iter()
                .enumerate()
                .map(|(n, _)| {
                    focus_handle(
                        child_id(&self.id, format!("header-{n}")),
                        n == header_stop,
                        window,
                        cx,
                    )
                })
                .collect(),
        );
        let choose_sort: ChooseSort = {
            let (state, columns, on_sort) = (state.clone(), columns.clone(), self.on_sort.clone());
            let current = sort.clone();
            Rc::new(move |column: usize, window: &mut Window, cx: &mut App| {
                let key = columns[column].key.clone();
                let direction = match &current {
                    Some((k, d)) if *k == key => d.flipped(),
                    _ => SortDirection::Ascending,
                };
                if !controlled_sort {
                    state.update(cx, |s, cx| {
                        s.sort = Some((key.clone(), direction));
                        cx.notify();
                    });
                }
                if let Some(f) = &on_sort {
                    f(&key, direction, window, cx);
                }
            })
        };
        let header_ring = RingTarget::default();
        let mut header = h_flex()
            .id(child_id(&self.id, "header"))
            .debug_selector(|| part_name(&self.id, "header"))
            .relative()
            .flex_shrink_0()
            .w_full()
            .h(px(g.height))
            .border_b(px(s.rule_width))
            .border_color(colour(s.rule))
            .text_role(s.header_role);
        {
            let (keys_handles, keys_state) = (header_handles.clone(), state.clone());
            header = header.on_key_down(move |e: &KeyDownEvent, window, cx| {
                let Some(current) = keys_handles.iter().position(|h| h.is_focused(window)) else {
                    return;
                };
                // Enter and Space sort through the header's click, which GPUI gives a focused
                // element on the key's release.
                let key = e.keystroke.key.as_str();
                let all: Vec<usize> = (0..keys_handles.len()).collect();
                if let Some(next) = roving_among(key, current, &all, Axis::Horizontal) {
                    keys_state.update(cx, |s, _| s.header = next);
                    move_focus(&keys_handles[next], window);
                    cx.stop_propagation();
                }
            });
        }
        let mut header_focused = false;
        for (i, column) in columns.iter().enumerate() {
            let place = sortable.iter().position(|s| *s == i);
            let focused = place.is_some_and(|n| header_handles[n].is_focused(window));
            header_focused |= focused;
            let hs = c.table_header(place.is_some(), States::NONE.focused(focused));
            let mark = sorted_by.filter(|(at, _)| *at == i).map(|(_, d)| d.mark());
            let mut label = h_flex()
                .min_w_0()
                .gap(px(g.gap))
                .child(truncate_end(div().min_w_0()).child(column.header.clone()))
                .children(mark.map(|m| {
                    let size = crate::foundation().controls.metrics().icon_size;
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .child(icon::still(m, size, hs.current.foreground))
                }));
            if column.align_end {
                label = label.justify_end();
            }
            let mut cell = sized(div(), i, &widths)
                .id(child_id(&self.id, format!("head-{i}")))
                .debug_selector(|| part_name(&self.id, &format!("head-{i}")))
                .relative()
                .flex()
                .items_center()
                .h_full()
                .px(px(g.padding_x))
                .child(label.w_full());
            if let Some(n) = place {
                let (handle, choose) = (header_handles[n].clone(), choose_sort.clone());
                let pressed_state = state.clone();
                cell = pointer_states(cell, &hs.colours, hs.responds, &Pointer::Own)
                    .track_focus(&handle)
                    .cursor_pointer()
                    .on_click(move |_: &ClickEvent, window, cx| {
                        pressed_state.update(cx, |s, _| s.header = n);
                        move_focus(&handle, window);
                        choose(i, window, cx);
                    });
                if focused {
                    cell = cell.child(header_ring.probe());
                }
            } else {
                cell = cell.treatment(&hs.current);
            }
            header = header.child(cell);
        }
        if header_focused {
            header =
                header.child(header_ring.ring(s.focus, g.radius, &child_id(&self.id, "header")));
        }

        // The rows: one tab stop, drawn only while in view.
        let body_focus = focus_handle(child_id(&self.id, "rows-focus"), true, window, cx);
        let body_focused = body_focus.is_focused(window);
        let row_ring = RingTarget::default();
        // Where the selected row is, for its menu from the keyboard.
        let anchor: Rc<std::cell::Cell<Option<Bounds<Pixels>>>> = Rc::default();
        let select: Select = {
            let (state, on_select) = (state.clone(), self.on_select.clone());
            Rc::new(
                move |key: SharedString, window: &mut Window, cx: &mut App| {
                    if !controlled_selection {
                        state.update(cx, |s, cx| {
                            if s.selected.as_ref() != Some(&key) {
                                s.selected = Some(key.clone());
                                cx.notify();
                            }
                        });
                    }
                    if let Some(f) = &on_select {
                        f(&key, window, cx);
                    }
                },
            )
        };

        let list = {
            let (rows, columns, order, keys) =
                (rows.clone(), columns.clone(), order.clone(), keys.clone());
            let (widths, sized, indent) = (widths.clone(), sized, self.indent.clone());
            let (select, on_activate) = (select.clone(), self.on_activate.clone());
            let (selected, ring, id) = (selected.clone(), row_ring.clone(), self.id.clone());
            let row_anchor = anchor.clone();
            let (focus, menu_state) = (body_focus.clone(), state.clone());
            let activate_on_click = self.activate_on_click;
            let has_menu = self.row_menu.is_some();
            uniform_list(
                child_id(&self.id, "rows"),
                order.len(),
                move |range, window, cx| {
                    let c = components();
                    let g = c.table(density).geometry;
                    let indent_step = c.foundation().spacing.s4;
                    #[cfg(test)]
                    crate::drawn::record(
                        &format!("{}/drawn-rows", crate::focus::id_path(&id)),
                        range.clone(),
                    );
                    range
                        .map(|place| {
                            let index = order[place];
                            let row = &rows[index];
                            let key = keys[place].clone();
                            let is_selected = selected.as_ref() == Some(&key);
                            let rs = c.table_row(States::NONE.selected(is_selected));
                            let mut el = h_flex()
                                .id(child_id(&id, format!("row-{key}")))
                                .debug_selector(|| part_name(&id, &format!("row-{place}")))
                                .relative()
                                .w_full()
                                .h(px(g.height));
                            el = pointer_states(el, &rs.colours, rs.responds, &Pointer::Own);
                            {
                                let (select, key, focus) =
                                    (select.clone(), key.clone(), focus.clone());
                                let on_activate = on_activate.clone();
                                el = el.on_mouse_down(MouseButton::Left, move |e, window, cx| {
                                    move_focus(&focus, window);
                                    let again =
                                        is_selected && (activate_on_click || e.click_count > 1);
                                    select(key.clone(), window, cx);
                                    if again && let Some(f) = &on_activate {
                                        f(&key, window, cx);
                                    }
                                });
                            }
                            if has_menu {
                                let (select, key, focus) =
                                    (select.clone(), key.clone(), focus.clone());
                                let menu_state = menu_state.clone();
                                el = el.on_mouse_down(MouseButton::Right, move |_, window, cx| {
                                    move_focus(&focus, window);
                                    select(key.clone(), window, cx);
                                    menu_state.update(cx, |s, _| s.menu_row = Some(key.clone()));
                                });
                            }
                            for (i, column) in columns.iter().enumerate() {
                                let mut cell =
                                    div().flex().items_center().h_full().px(px(g.padding_x));
                                if i == 0
                                    && let Some(depth) = &indent
                                {
                                    cell =
                                        cell.pl(px(g.padding_x + indent_step * depth(row) as f32));
                                }
                                if column.align_end {
                                    cell = cell.justify_end();
                                }
                                cell = sized(cell, i, &widths);
                                let cell_id = child_id(&id, format!("cell-{key}-{i}"));
                                let content: AnyElement = match &column.content {
                                    Content::Text(text) => {
                                        let text = text(row);
                                        let size =
                                            px(c.foundation().typography.style(g.type_role).size);
                                        let (over, probe) = decoration::truncation_probe(
                                            &cell_id,
                                            text.clone(),
                                            size,
                                            1,
                                            window,
                                            cx,
                                        );
                                        decoration::overflow_tooltip(
                                            &cell_id,
                                            text.clone(),
                                            over,
                                            ClipFit::Fill,
                                            false,
                                            truncate_end(div().relative().min_w_0())
                                                .child(text)
                                                .child(probe),
                                        )
                                    }
                                    // A block inside the cell's row, so the cell centres
                                    // it whatever the element says of its alignment.
                                    Content::Element(build) => div()
                                        .min_w_0()
                                        .child(build(row, window, cx))
                                        .into_any_element(),
                                    Content::Action { label, on_press } => match label(row) {
                                        Some(label) => {
                                            let on_press = on_press.clone();
                                            let rows = rows.clone();
                                            div()
                                                .child(
                                                    Button::new(cell_id.clone())
                                                        .label(label)
                                                        .variant(ButtonVariant::Ghost)
                                                        .density(Density::Compact)
                                                        .skip_tab()
                                                        .keep_focus()
                                                        .on_click(move |_, window, cx| {
                                                            on_press(&rows[index], window, cx)
                                                        }),
                                                )
                                                .into_any_element()
                                        }
                                        None => div().into_any_element(),
                                    },
                                };
                                el = el.child(cell.child(content));
                            }
                            if is_selected {
                                let row_anchor = row_anchor.clone();
                                el = el
                                    .child(ring.probe())
                                    .child(bounds_probe(px(0.0), move |b, _| {
                                        row_anchor.set(Some(b))
                                    }));
                            }
                            el.into_any_element()
                        })
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(scroll.clone())
            .size_full()
        };

        let page = rows_bounds
            .map(|b| (f32::from(b.size.height) / g.height).floor() as usize)
            .unwrap_or(1)
            .saturating_sub(1)
            .max(1);
        let probe_state = state.clone();
        let mut body = v_flex()
            .id(child_id(&self.id, "body"))
            .debug_selector(|| part_name(&self.id, "rows"))
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .track_focus(&body_focus)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        probe_state.update(cx, |s, _| s.rows_bounds = Some(bounds))
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        {
            let (keys, select, scroll) = (keys.clone(), select.clone(), scroll.clone());
            let on_activate = self.on_activate.clone();
            let selected = selected.clone();
            body = body.on_key_down(move |e: &KeyDownEvent, window, cx| {
                if e.keystroke.modifiers.modified() {
                    return;
                }
                let key = e.keystroke.key.as_str();
                if key == "enter" {
                    if let (Some(k), Some(f)) = (&selected, &on_activate) {
                        f(k, window, cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if let Some(next) = next_row(key, at, keys.len(), page) {
                    scroll.scroll_to_item(next, ScrollStrategy::Top);
                    select(keys[next].clone(), window, cx);
                    cx.stop_propagation();
                }
            });
        }
        body = if self.rows.is_empty() {
            if self.loading {
                body.p(px(g.padding_x))
                    .gap(px(g.gap))
                    .children((0..LOADING_ROWS).map(|_| Skeleton::line()))
            } else {
                body.child(
                    div()
                        .debug_selector(|| part_name(&self.id, "empty"))
                        .p(px(g.padding_x))
                        .text_color(colour(s.empty))
                        .child(self.empty.clone()),
                )
            }
        } else {
            body.child(row_ring.clip_probe()).child(list)
        };
        if body_focused && at.is_some() {
            body = body.child(row_ring.ring(s.focus, g.radius, &child_id(&self.id, "rows")));
        }

        let body: AnyElement = match self.row_menu.clone() {
            Some(build) => {
                let (rows, row_key) = (rows.clone(), row_key.clone());
                let target = menu_row.clone().or_else(|| selected.clone());
                ContextMenu::new(child_id(&self.id, "menu"), "Row actions", body)
                    .focusable(false)
                    .fill()
                    .keyboard_anchor(anchor)
                    .menu(move |menu| {
                        let found = target.as_ref().and_then(|k| {
                            (0..rows.len()).find(|i| key_of(&row_key, &rows, *i) == *k)
                        });
                        match found {
                            Some(i) => build(&rows[i], menu),
                            None => menu,
                        }
                    })
                    .into_any_element()
            }
            None => body.into_any_element(),
        };

        v_flex()
            .id(self.id.clone())
            .debug_selector(|| part_name(&self.id, "table"))
            .size_full()
            .min_h_0()
            .text_role(g.type_role)
            .text_color(colour(s.text))
            .child(header)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_sort_stably_by_text_without_case_and_by_number() {
        let rows = ["beta", "Alpha", "alpha", "gamma"];
        let column = Column::new("name", "Name", |r: &&str| SharedString::from(r.to_string()))
            .sort_by(|r: &&str| r.to_string());
        assert_eq!(
            order(&rows, Some(&column), SortDirection::Ascending),
            [1, 2, 0, 3]
        );
        assert_eq!(
            order(&rows, Some(&column), SortDirection::Descending),
            [3, 0, 2, 1]
        );
        let numbers = [3.5f64, -1.0, 10.0, f64::NAN];
        let column = Column::new("n", "N", |r: &f64| r.to_string().into()).sort_by(|r: &f64| *r);
        assert_eq!(
            order(&numbers, Some(&column), SortDirection::Ascending),
            [1, 0, 2, 3]
        );
        let unsorted = Column::new("n", "N", |r: &f64| r.to_string().into());
        assert_eq!(
            order(&numbers, Some(&unsorted), SortDirection::Descending),
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn the_keys_move_the_selection_within_the_rows() {
        assert_eq!(next_row("down", None, 10, 4), Some(0));
        assert_eq!(
            next_row("down", Some(9), 10, 4),
            Some(9),
            "stays at the end"
        );
        assert_eq!(next_row("up", Some(0), 10, 4), Some(0));
        assert_eq!(next_row("pagedown", Some(2), 10, 4), Some(6));
        assert_eq!(next_row("pagedown", Some(8), 10, 4), Some(9));
        assert_eq!(next_row("pageup", Some(2), 10, 4), Some(0));
        assert_eq!(next_row("end", Some(2), 10, 4), Some(9));
        assert_eq!(next_row("home", None, 10, 4), Some(0));
        assert_eq!(next_row("down", None, 0, 4), None);
        assert_eq!(next_row("left", Some(2), 10, 4), None);
    }
}
