//! A table of the application's rows on the Rune host: sortable headers, one selected row, only
//! the rows in view built (ADR 0070, `docs/spec/odk-ui-components.md`).

use std::borrow::Cow;
use std::cmp::Ordering;
use std::rc::Rc;

use crate::button::{Button, Press};
use crate::describe::apply;
use crate::host::Hosted;
use crate::icon::{Glyph, Icon};
use crate::ink::ink;
use crate::menu::{Menu, at_point};
use crate::skeleton::Skeleton;
use crate::store::Activator;
use crate::style::{self, Paint, focus_ring, inset_focus_ring, text_style};
use crate::text::{Flow, words};
use crate::toggle::{Toggle, pressable};
use odk_a11y::{Accessible, Describe, Role};
use odk_theme::component::{ButtonVariant, States};
use odk_theme::foundation::Density;
use rune::{El, GrowthSpec, Key, KeyEvent, PointerPress, ScrollSpec, Tx, col, el, row, text};

/// How many skeleton rows an empty table shows while it is loading. ODK's own.
const LOADING_ROWS: usize = 3;

/// How wide a character of the table's text is, as a share of its size, for a column given in
/// characters.
const CHAR_WIDTH_EM: f32 = 0.6;

/// How many rows a page is before the viewport has been measured.
const FIRST_PAGE: usize = 10;

/// What a column sorts its rows by: text, compared without regard to case, or a number.
#[derive(Debug, Clone, PartialEq)]
pub enum SortKey {
    Text(String),
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

macro_rules! number_keys {
    ($($t:ty),*) => {$(
        impl From<$t> for SortKey {
            fn from(n: $t) -> Self {
                Self::Number(n as f64)
            }
        }
    )*};
}
number_keys!(f64, f32, i32, i64, u32, u64, usize);

impl From<&str> for SortKey {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<String> for SortKey {
    fn from(s: String) -> Self {
        Self::Text(s)
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

    fn word(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

#[derive(Clone, Copy)]
enum Width {
    Grow(f32),
    Chars(u16),
}

type Text<R> = Rc<dyn Fn(&R) -> String>;
type CellOf<T, R> = Rc<dyn Fn(&Hosted<T>, &R) -> El>;
type Label<R> = Rc<dyn Fn(&R) -> Option<String>>;
type OnKey<T> = Rc<dyn Fn(&mut T, &str)>;
type SortBy<R> = Rc<dyn Fn(&R) -> SortKey>;
type OnSort<T> = Rc<dyn Fn(&mut T, &str, SortDirection)>;
type RowMenu<T, R> = Rc<dyn Fn(&R, Menu<T>) -> Menu<T>>;
type Rows<T, R> = Rc<dyn for<'a> Fn(&'a T) -> &'a [R]>;
type KeyOf<R> = Rc<dyn Fn(&R) -> String>;
type Sorted = Option<(String, SortDirection)>;

enum Content<T, R> {
    Text(Text<R>),
    Element(CellOf<T, R>),
    Action { label: Label<R>, on_press: OnKey<T> },
}

/// One column of a [`DataTable`]: its key, its name, how wide it is and what its cells show.
pub struct Column<T, R> {
    key: String,
    header: String,
    width: Width,
    align_end: bool,
    content: Content<T, R>,
    sort: Option<SortBy<R>>,
}

impl<T: 'static, R: 'static> Column<T, R> {
    /// A column named `header`, known as `key` (what `on_sort` and `sort` name), whose cells show
    /// `text`. It shares what the other columns leave equally.
    pub fn new(
        key: impl Into<String>,
        header: impl Into<String>,
        text: impl Fn(&R) -> String + 'static,
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

    /// A column whose cells are elements one row high, such as a badge. `cell` gets the host to
    /// draw with.
    pub fn cell(
        key: impl Into<String>,
        header: impl Into<String>,
        cell: impl Fn(&Hosted<T>, &R) -> El + 'static,
    ) -> Self {
        Self {
            content: Content::Element(Rc::new(cell)),
            ..Self::new(key, header, |_| String::new())
        }
    }

    /// A column of one button per row, labelled by `label` (none for a row without it), that takes
    /// the pointer only: put the same action in the row's menu, which the keyboard reaches. The
    /// press hears the row's key.
    pub fn action(
        key: impl Into<String>,
        header: impl Into<String>,
        label: impl Fn(&R) -> Option<String> + 'static,
        on_press: impl Fn(&mut T, &str) + 'static,
    ) -> Self {
        Self {
            content: Content::Action {
                label: Rc::new(label),
                on_press: Rc::new(on_press),
            },
            ..Self::new(key, header, |_| String::new())
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

    fn text(&self, row: &R) -> String {
        match &self.content {
            Content::Text(text) => text(row),
            Content::Action { label, .. } => label(row).unwrap_or_default(),
            Content::Element(_) => String::new(),
        }
    }
}

/// A table of the application's rows. The header names the columns and stays in place; only the
/// rows in view are built, so a table of thousands of rows costs what the visible ones do
/// (ADR 0070). The application's state holds the rows, which the table reads through `rows`, and
/// a key that names each across updates.
///
/// One row is selected, by its key, and stays selected when the rows are sorted or replaced. A
/// press selects a row; `on_activate` hears Enter, and a press on the selected row when
/// `activate_on_click` is set. `row_menu` gives each row a context menu, opened by a right click
/// or by the Menu key or Shift+F10 on the selected row. With no rows the table shows `empty`, or
/// skeleton rows while `loading`.
///
/// Keyboard: the sortable headers are one tab stop, where Left and Right move between them and
/// Enter or Space sorts by one, again to reverse it. The rows are the next tab stop: Up and Down
/// move the selection, Home and End to the first and last row, Page Up and Page Down by a page,
/// Enter activates the selected row.
pub struct DataTable<T, R> {
    id: String,
    name: String,
    rows: Rows<T, R>,
    row_key: KeyOf<R>,
    columns: Vec<Column<T, R>>,
    sort: Sorted,
    default_sort: Sorted,
    on_sort: Option<OnSort<T>>,
    selected: Option<Option<String>>,
    on_select: Option<OnKey<T>>,
    on_activate: Option<OnKey<T>>,
    activate_on_click: bool,
    row_menu: Option<RowMenu<T, R>>,
    empty: String,
    loading: bool,
    density: Density,
    height: Option<f32>,
}

impl<T: 'static, R: 'static> DataTable<T, R> {
    /// A table named `name` for assistive technology. `rows` reads the application's rows from
    /// its state; `row_key` names each across updates.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        rows: impl for<'a> Fn(&'a T) -> &'a [R] + 'static,
        row_key: impl Fn(&R) -> String + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            rows: Rc::new(rows),
            row_key: Rc::new(row_key),
            columns: Vec::new(),
            sort: None,
            default_sort: None,
            on_sort: None,
            selected: None,
            on_select: None,
            on_activate: None,
            activate_on_click: false,
            row_menu: None,
            empty: "No rows".into(),
            loading: false,
            density: Density::Default,
            height: None,
        }
    }

    pub fn column(mut self, column: Column<T, R>) -> Self {
        self.columns.push(column);
        self
    }

    /// The sort the table starts with, until a header is chosen.
    pub fn default_sort(mut self, key: impl Into<String>, direction: SortDirection) -> Self {
        self.default_sort = Some((key.into(), direction));
        self
    }

    /// The sort, owned by the application, which then hears `on_sort` and gives it back.
    pub fn sort(mut self, key: impl Into<String>, direction: SortDirection) -> Self {
        self.sort = Some((key.into(), direction));
        self
    }

    pub fn on_sort(mut self, f: impl Fn(&mut T, &str, SortDirection) + 'static) -> Self {
        self.on_sort = Some(Rc::new(f));
        self
    }

    /// The selected row's key, owned by the application, which then hears `on_select`.
    pub fn selected(mut self, key: Option<String>) -> Self {
        self.selected = Some(key);
        self
    }

    pub fn on_select(mut self, f: impl Fn(&mut T, &str) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    pub fn on_activate(mut self, f: impl Fn(&mut T, &str) + 'static) -> Self {
        self.on_activate = Some(Rc::new(f));
        self
    }

    pub fn activate_on_click(mut self, on: bool) -> Self {
        self.activate_on_click = on;
        self
    }

    /// Gives a row a menu: `build` adds its items to the empty `Menu` it is handed.
    pub fn row_menu(mut self, build: impl Fn(&R, Menu<T>) -> Menu<T> + 'static) -> Self {
        self.row_menu = Some(Rc::new(build));
        self
    }

    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.empty = empty.into();
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// The table's height in points at the base text size; without it the table takes what its
    /// column gives it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

impl<T, R> Describe for DataTable<T, R> {
    fn accessible(&self) -> Accessible {
        Accessible::new(Role::Table, self.name.clone())
    }
}

/// What the table's closures share.
struct Spec<T, R> {
    id: String,
    name: String,
    rows: Rows<T, R>,
    row_key: KeyOf<R>,
    columns: Vec<Column<T, R>>,
    on_sort: Option<OnSort<T>>,
    controlled_selection: Option<Option<String>>,
    on_select: Option<OnKey<T>>,
    on_activate: Option<OnKey<T>>,
    activate_on_click: bool,
    has_menu: bool,
    density: Density,
}

impl<T: 'static, R: 'static> Spec<T, R> {
    fn context_id(&self) -> String {
        format!("{}-ctx", self.id)
    }

    fn body_id(&self) -> String {
        format!("{}-body", self.id)
    }

    /// The selected row's key now.
    fn selection(&self, ui: &crate::store::Store) -> Option<String> {
        match &self.controlled_selection {
            Some(key) => key.clone(),
            None => ui.table(&self.id).and_then(|s| s.selected.clone()),
        }
    }

    fn sort_of(&self, ui: &crate::store::Store, controlled: &Sorted, default: &Sorted) -> Sorted {
        controlled
            .clone()
            .or_else(|| ui.table(&self.id).and_then(|s| s.sort.clone()))
            .or_else(|| default.clone())
    }

    /// The rows' indices in the order the sort gives, which is the rows' own without one.
    fn order(&self, rows: &[R], sort: &Sorted) -> Vec<usize> {
        let mut order: Vec<usize> = (0..rows.len()).collect();
        let Some((key, direction)) = sort else {
            return order;
        };
        let Some(column) = self.columns.iter().find(|c| &c.key == key) else {
            return order;
        };
        let keys: Vec<SortKey> = rows
            .iter()
            .map(|row| match (&column.sort, &column.content) {
                (Some(by), _) => by(row),
                (None, _) => SortKey::Text(column.text(row)),
            })
            .collect();
        order.sort_by(|a, b| {
            let ordering = keys[*a].compare(&keys[*b]);
            match direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            }
        });
        order
    }

    /// A column's wrapper at its width.
    fn sized(&self, element: El, column: &Column<T, R>, paint: &Paint) -> El {
        match column.width {
            Width::Grow(weight) => element.w(0.0).grow(weight).shrink().min_w(0.0),
            Width::Chars(chars) => {
                let size = paint
                    .role(paint.tokens.table(self.density).geometry.type_role)
                    .size;
                element.w(chars as f32 * size * CHAR_WIDTH_EM).shrink()
            }
        }
    }

    fn header_el(&self, paint: &Paint, sort: &Sorted) -> El {
        let s = paint.tokens.table(self.density);
        let mut header = row()
            .key(format!("{}-header", self.id))
            .w_full()
            .h(s.geometry.height)
            .px(s.geometry.padding_x)
            .gap(s.geometry.gap)
            .center_y()
            // The sortable headers are one tab stop; Left and Right move between them.
            .focus_group();
        for column in &self.columns {
            let sortable = column.sort.is_some();
            let h = paint.tokens.table_header(sortable, States::NONE);
            let colour = h.colours.default.foreground;
            let current = sort.as_ref().filter(|(key, _)| *key == column.key);
            let mut accessible = Accessible::new(Role::ColumnHeader, column.header.clone());
            if let Some((_, direction)) = current {
                accessible = accessible.value(direction.word());
            }
            let id = format!("{}-h-{}", self.id, column.key);
            let mut cell = apply(row().key(id.clone()), &accessible)
                .center_y()
                .gap(s.geometry.gap);
            if column.align_end {
                cell = cell.justify_end();
            }
            cell = cell.child(
                words(
                    paint,
                    column.header.clone(),
                    s.header_role,
                    colour,
                    Flow::End,
                )
                .shrink(),
            );
            if let Some((_, direction)) = current {
                cell = cell
                    .child(Glyph::from(direction.mark()).slot(s.geometry.icon_size, ink(colour)));
            }
            let cell = self.sized(cell, column, paint);
            header = header.child(if sortable {
                let toggle: Toggle<T> = {
                    let (table, key) = (self.id.clone(), column.key.clone());
                    let on_sort = self.on_sort.clone();
                    Rc::new(move |hosted: &mut Hosted<T>| {
                        let was = hosted.ui.table(&table).and_then(|s| s.sort.clone());
                        let direction = match was {
                            Some((k, d)) if k == key => d.flipped(),
                            _ => SortDirection::Ascending,
                        };
                        hosted.ui.table_mut(&table).sort = Some((key.clone(), direction));
                        if let Some(on_sort) = &on_sort {
                            on_sort(&mut hosted.app, &key, direction);
                        }
                    })
                };
                inset_focus_ring(pressable(cell, &id, true, toggle), &s.focus).focusable(true)
            } else {
                cell
            });
        }
        col()
            .w_full()
            .child(header)
            .child(el().w_full().h(s.rule_width).fill(ink(s.rule)))
    }
}

/// One row, built when it comes into the window Rune keeps mounted.
fn row_el<T: 'static, R: 'static>(
    spec: &Spec<T, R>,
    hosted: &Hosted<T>,
    row_data: &R,
    selected: bool,
) -> El {
    let paint = style::paint(hosted.theme());
    let s = paint.tokens.table(spec.density);
    let key = (spec.row_key)(row_data);
    let states = States::NONE.selected(selected);
    let r = paint.tokens.table_row(states);
    let summary = spec
        .columns
        .iter()
        .map(|c| c.text(row_data))
        .find(|t| !t.is_empty())
        .unwrap_or_else(|| key.clone());
    let accessible = Accessible::new(Role::Row, summary).selected(selected);
    let mut line = apply(row().key(format!("{}-row-{key}", spec.id)), &accessible)
        .w_full()
        .h(s.geometry.height)
        .px(s.geometry.padding_x)
        .gap(s.geometry.gap)
        .center_y()
        .fill(ink(r.colours.default.background))
        .hover(ink(r.colours.hover.background))
        .press(ink(r.colours.pressed.background));
    for column in &spec.columns {
        let text_of = column.text(row_data);
        let cell_key = format!("{}-cell-{key}-{}", spec.id, column.key);
        let mut cell = apply(
            row().key(cell_key.clone()),
            &Accessible::new(Role::Cell, text_of.clone()),
        )
        .center_y();
        if column.align_end {
            cell = cell.justify_end();
        }
        cell = match &column.content {
            Content::Text(_) => cell.child(
                words(&paint, text_of, s.geometry.type_role, s.text, Flow::End)
                    .shrink()
                    .a11y_hidden(),
            ),
            Content::Element(make) => cell.child(make(hosted, row_data)),
            Content::Action { label, on_press } => match label(row_data) {
                Some(label) => {
                    let (on_press, key) = (on_press.clone(), key.clone());
                    let act: Press<T> =
                        Rc::new(move |h: &mut Hosted<T>| on_press(&mut h.app, &key));
                    cell.child(
                        Button::new(format!("{cell_key}-button"), label, ())
                            .variant(ButtonVariant::Ghost)
                            .tab_stop(false)
                            .show_hosted(hosted, act),
                    )
                }
                None => cell,
            },
        };
        line = line.child(spec.sized(cell, column, &paint));
    }
    // The handlers find their row by its key.
    let id = spec.id.clone();
    let press_key = key.clone();
    let spec_ptr = SpecHandles::of(spec);
    let menu_key = key.clone();
    let menu_handles = spec_ptr.clone();
    line = line.on_press(move |h: &mut Hosted<T>| {
        let was_selected = spec_ptr.selection(&h.ui).as_deref() == Some(press_key.as_str());
        spec_ptr.select(h, &press_key);
        if was_selected && spec_ptr.activate_on_click {
            spec_ptr.activate(h, &press_key);
        }
    });
    if spec.has_menu {
        line = line.on_secondary_press(move |h: &mut Hosted<T>, at: PointerPress, _: &mut Tx| {
            menu_handles.select(h, &menu_key);
            h.ui.table_mut(&id).menu_row = Some(menu_key.clone());
            h.ui.set_menu_at(&menu_handles.context_id, (at.x, at.y));
            h.ui.set_open(&menu_handles.context_id, true);
        });
    }
    line
}

/// The parts of the table a row's handlers need, cloned into each.
struct SpecHandles<T> {
    id: String,
    context_id: String,
    controlled_selection: Option<Option<String>>,
    on_select: Option<OnKey<T>>,
    on_activate: Option<OnKey<T>>,
    activate_on_click: bool,
    has_menu: bool,
}

impl<T> Clone for SpecHandles<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            context_id: self.context_id.clone(),
            controlled_selection: self.controlled_selection.clone(),
            on_select: self.on_select.clone(),
            on_activate: self.on_activate.clone(),
            activate_on_click: self.activate_on_click,
            has_menu: self.has_menu,
        }
    }
}

impl<T: 'static> SpecHandles<T> {
    fn of<R: 'static>(spec: &Spec<T, R>) -> Self {
        Self {
            id: spec.id.clone(),
            context_id: spec.context_id(),
            controlled_selection: spec.controlled_selection.clone(),
            on_select: spec.on_select.clone(),
            on_activate: spec.on_activate.clone(),
            activate_on_click: spec.activate_on_click,
            has_menu: spec.has_menu,
        }
    }

    fn selection(&self, ui: &crate::store::Store) -> Option<String> {
        match &self.controlled_selection {
            Some(key) => key.clone(),
            None => ui.table(&self.id).and_then(|s| s.selected.clone()),
        }
    }

    fn select(&self, hosted: &mut Hosted<T>, key: &str) {
        let was = self.selection(&hosted.ui);
        if self.controlled_selection.is_none() {
            hosted.ui.table_mut(&self.id).selected = Some(key.to_string());
        }
        if was.as_deref() != Some(key)
            && let Some(on_select) = &self.on_select
        {
            on_select(&mut hosted.app, key);
        }
    }

    fn activate(&self, hosted: &mut Hosted<T>, key: &str) {
        if let Some(on_activate) = &self.on_activate {
            on_activate(&mut hosted.app, key);
        }
    }
}

impl<T: 'static, R: 'static> DataTable<T, R> {
    /// The table, with the open row menu drawn into `layers`.
    pub fn show(self, hosted: &Hosted<T>, layers: &mut Vec<El>) -> El {
        let accessible = self.accessible();
        let paint = style::paint(hosted.theme());
        let s = paint.tokens.table(self.density);
        let DataTable {
            id,
            name,
            rows,
            row_key,
            columns,
            sort,
            default_sort,
            on_sort,
            selected,
            on_select,
            on_activate,
            activate_on_click,
            row_menu,
            empty,
            loading,
            density,
            height,
        } = self;
        let spec = Rc::new(Spec {
            id: id.clone(),
            name,
            rows,
            row_key,
            columns,
            on_sort,
            controlled_selection: selected,
            on_select,
            on_activate,
            activate_on_click,
            has_menu: row_menu.is_some(),
            density,
        });
        let current_sort = spec.sort_of(&hosted.ui, &sort, &default_sort);
        let all = (spec.rows)(&hosted.app);
        let order = Rc::new(spec.order(all, &current_sort));
        let count = order.len();
        let header = spec.header_el(&paint, &current_sort);

        // The row menu of the row it was opened on.
        if let Some(build) = &row_menu
            && hosted.ui.is_open(&spec.context_id(), false)
            && let Some(slot) = hosted.ui.table(&id)
            && let Some(key) = slot.menu_row.clone()
            && let Some(row) = all.iter().find(|r| (spec.row_key)(r) == key)
        {
            at_point(
                hosted,
                &spec.context_id(),
                &spec.name,
                &spec.body_id(),
                build(row, Menu::new()),
                layers,
            );
        }

        let body = if count == 0 {
            if loading {
                let mut skeletons = col().w_full();
                for n in 0..LOADING_ROWS {
                    skeletons = skeletons.child(
                        row()
                            .key(format!("{id}-loading-{n}"))
                            .w_full()
                            .h(s.geometry.height)
                            .px(s.geometry.padding_x)
                            .center_y()
                            .child(Skeleton::line().show(hosted)),
                    );
                }
                skeletons
            } else {
                col()
                    .key(format!("{id}-empty"))
                    .w_full()
                    .center()
                    .py(s.geometry.height)
                    .child(text(empty).font(text_style(paint.body)).color(ink(s.empty)))
            }
        } else {
            let selected_key = spec.selection(&hosted.ui);
            let key_spec = spec.clone();
            let key_order = order.clone();
            let build_spec = spec.clone();
            let build_order = order.clone();
            let build_selected = selected_key.clone();
            // Rune reveals the selected row of a virtual list, as it scrolls a palette's.
            let rows_el = col()
                .key(format!("{id}-rows"))
                .w_full()
                .growth(GrowthSpec::default())
                .reveal_selected(true)
                .virtual_rows(
                    count,
                    s.geometry.height,
                    move |state: &Hosted<T>, i: usize| -> Cow<'_, str> {
                        let rows = (key_spec.rows)(&state.app);
                        Cow::Owned(format!(
                            "{}-row-{}",
                            key_spec.id,
                            (key_spec.row_key)(&rows[key_order[i]])
                        ))
                    },
                    move |state: &Hosted<T>, i: usize| {
                        let rows = (build_spec.rows)(&state.app);
                        let row_data = &rows[build_order[i]];
                        let selected = build_selected.as_deref()
                            == Some((build_spec.row_key)(row_data).as_str());
                        row_el(&build_spec, state, row_data, selected)
                    },
                );
            let handles = SpecHandles::of(&spec);
            let (key_id, up_id) = (id.clone(), id.clone());
            let (key_order, key_rows, key_row_key) =
                (order.clone(), spec.rows.clone(), spec.row_key.clone());
            let layout_id = id.clone();
            let menu_handles = handles.clone();
            let menu_id = id.clone();
            let release_order = order.clone();
            let (release_rows, release_row_key, release_handles) =
                (spec.rows.clone(), spec.row_key.clone(), handles.clone());
            let height_of_row = s.geometry.height;
            focus_ring(
                col()
                    .key(spec.body_id())
                    .w_full()
                    .grow(1.0)
                    .min_h(0.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .focusable(true)
                    .on_layout(move |h: &mut Hosted<T>, rect: [f32; 4]| {
                        if (h.ui.table(&layout_id).map_or(0.0, |s| s.viewport) - rect[3]).abs()
                            > 0.5
                        {
                            h.ui.table_mut(&layout_id).viewport = rect[3];
                        }
                    })
                    .on_key(move |h: &mut Hosted<T>, ev: &KeyEvent, tx: &mut Tx| {
                        if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
                            return false;
                        }
                        let rows = (key_rows)(&h.app);
                        let n = key_order.len();
                        if n == 0 {
                            return false;
                        }
                        let key_at = |i: usize| (key_row_key)(&rows[key_order[i]]);
                        let current = handles
                            .selection(&h.ui)
                            .and_then(|k| (0..n).find(|i| key_at(*i) == k));
                        let page =
                            h.ui.table(&key_id)
                                .map(|s| s.viewport)
                                .filter(|v| *v > 0.0)
                                .map_or(FIRST_PAGE, |v| {
                                    ((v / height_of_row) as usize).saturating_sub(1).max(1)
                                });
                        let target = match ev.key {
                            Key::Down => Some(current.map_or(0, |c| (c + 1).min(n - 1))),
                            Key::Up => Some(current.map_or(n - 1, |c| c.saturating_sub(1))),
                            Key::Home => Some(0),
                            Key::End => Some(n - 1),
                            Key::PageDown => Some(current.map_or(0, |c| (c + page).min(n - 1))),
                            Key::PageUp => Some(current.map_or(n - 1, |c| c.saturating_sub(page))),
                            Key::Enter => {
                                // Enter acts on its release, once for each hold.
                                if !tx.key_repeat() {
                                    h.ui.button_mut(&key_id).armed = Some(Activator::Enter);
                                }
                                return true;
                            }
                            _ => None,
                        };
                        let Some(target) = target else {
                            return false;
                        };
                        let key = key_at(target);
                        handles.select(h, &key);
                        // The row may not be built yet; Rune reaches it by its key.
                        tx.scroll_into_view(format!("{key_id}-row-{key}"));
                        true
                    })
                    .on_key_up(move |h: &mut Hosted<T>, ev: &KeyEvent, _: &mut Tx| {
                        if ev.key != Key::Enter {
                            return false;
                        }
                        let slot = h.ui.button_mut(&up_id);
                        if slot.armed != Some(Activator::Enter) {
                            return false;
                        }
                        slot.armed = None;
                        let rows = (release_rows)(&h.app);
                        let selected = release_handles.selection(&h.ui);
                        let key = selected.filter(|k| {
                            (0..release_order.len())
                                .any(|i| (release_row_key)(&rows[release_order[i]]) == *k)
                        });
                        if let Some(key) = key {
                            release_handles.activate(h, &key);
                        }
                        true
                    })
                    .on_secondary_press(move |h: &mut Hosted<T>, at: PointerPress, _: &mut Tx| {
                        // The Menu key and Shift+F10 reach here for the selected row; a right
                        // click on a row has opened its menu already.
                        if !menu_handles.has_menu || h.ui.is_open(&menu_handles.context_id, false) {
                            return;
                        }
                        if let Some(key) = menu_handles.selection(&h.ui) {
                            h.ui.table_mut(&menu_id).menu_row = Some(key);
                            h.ui.set_menu_at(&menu_handles.context_id, (at.x, at.y));
                            h.ui.set_open(&menu_handles.context_id, true);
                        }
                    })
                    .child(rows_el),
                &s.focus,
            )
        };

        let mut table = apply(col().key(id), &accessible)
            .w_full()
            .child(header)
            .child(body);
        table = match height {
            Some(h) => table.h(paint.px(h)),
            None => table.grow(1.0).min_h(0.0),
        };
        table
    }
}
