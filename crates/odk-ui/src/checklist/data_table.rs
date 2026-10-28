use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, MouseButton, ParentElement, SharedString, Styled, TestAppContext,
    VisualTestContext, px,
};
use odk_a11y::{Describe, Role};

use super::{Count, FIXTURES, open, press, publish_fixture, redraw, reduce_motion};
use crate::drawn;
use crate::overlay::probe;
use crate::{Button, Column, DataTable, MenuItem, SortDirection, v_flex};

/// A process: its id, name and CPU use.
#[derive(Clone)]
struct Proc(u32, &'static str, f64);

type Rows = Rc<RefCell<Vec<Proc>>>;
type Seen = Rc<RefCell<Vec<String>>>;

fn procs() -> Vec<Proc> {
    vec![
        Proc(1, "systemd", 0.1),
        Proc(42, "hyprland", 12.5),
        Proc(7, "waybar", 1.5),
        Proc(300, "firefox", 30.0),
        Proc(12, "atmosd", 0.4),
    ]
}

struct Table {
    rows: Rows,
    selected: Seen,
    activated: Seen,
    sorted: Seen,
    pressed: Seen,
    after: Count,
}

impl Table {
    fn new(rows: Vec<Proc>) -> Self {
        Self {
            rows: Rc::new(RefCell::new(rows)),
            selected: Seen::default(),
            activated: Seen::default(),
            sorted: Seen::default(),
            pressed: Seen::default(),
            after: Count::default(),
        }
    }

    /// The table in a box 200 px high, and a button after it.
    fn build(&self, loading: bool) -> impl Fn() -> gpui::AnyElement + 'static {
        let (rows, selected, activated) = (
            self.rows.clone(),
            self.selected.clone(),
            self.activated.clone(),
        );
        let (sorted, pressed, after) = (
            self.sorted.clone(),
            self.pressed.clone(),
            self.after.clone(),
        );
        move || {
            let (selected, activated, sorted) =
                (selected.clone(), activated.clone(), sorted.clone());
            let (pressed, after) = (pressed.clone(), after.clone());
            v_flex()
                .size_full()
                .child(
                    v_flex().h(px(200.0)).w_full().child(
                        DataTable::new("t", "Processes", rows.borrow().clone())
                            .row_key(|p: &Proc| p.0.to_string().into())
                            .column(
                                Column::new("name", "Name", |p: &Proc| p.1.into())
                                    .sort_by(|p: &Proc| p.1),
                            )
                            .column(
                                Column::new("cpu", "CPU", |p: &Proc| format!("{}%", p.2).into())
                                    .chars(6)
                                    .align_end()
                                    .sort_by(|p: &Proc| p.2),
                            )
                            .column(Column::action(
                                "end",
                                "",
                                |p: &Proc| (p.0 != 1).then(|| "End".into()),
                                move |p: &Proc, _, _| pressed.borrow_mut().push(p.1.to_string()),
                            ))
                            .loading(loading)
                            .empty("No process matches")
                            .on_select(move |k, _, _| selected.borrow_mut().push(k.to_string()))
                            .on_activate(move |k, _, _| activated.borrow_mut().push(k.to_string()))
                            .on_sort(move |key, d, _, _| {
                                sorted.borrow_mut().push(format!("{key} {d:?}"))
                            })
                            .row_menu(|p: &Proc, menu| {
                                menu.item(MenuItem::new(format!("End {}", p.1)))
                            }),
                    ),
                )
                .child(
                    Button::new("after")
                        .label("After")
                        .on_click(move |_, _, _| after.bump()),
                )
                .into_any_element()
        }
    }
}

/// The keys of the rows in the order drawn, and the selected key.
fn drawn_rows() -> (Vec<SharedString>, Option<SharedString>) {
    drawn::last("t")
}

fn order() -> Vec<String> {
    drawn_rows().0.iter().map(|k| k.to_string()).collect()
}

fn selected() -> Option<String> {
    drawn_rows().1.map(|k| k.to_string())
}

fn click(cx: &mut VisualTestContext, part: &'static str, button: MouseButton) {
    let at = cx
        .debug_bounds(part)
        .unwrap_or_else(|| panic!("{part}"))
        .center();
    cx.simulate_mouse_down(at, button, Modifiers::none());
    cx.simulate_mouse_up(at, button, Modifiers::none());
}

/// Items 8, 9, 10: the sortable headers are one tab stop and the rows the next; the arrows,
/// Home and End move the selection, Enter activates, and the next Tab leaves. A cell's button is
/// not a tab stop.
#[gpui::test]
fn the_keyboard_sorts_selects_and_activates(cx: &mut TestAppContext) {
    let t = Table::new(procs());
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    assert_eq!(order(), ["1", "42", "7", "300", "12"], "as given");
    // Tab to the headers: Enter sorts by Name, again reverses it; Right moves to CPU.
    press(cx, "tab enter");
    assert_eq!(order(), ["12", "300", "42", "1", "7"]);
    press(cx, "enter");
    assert_eq!(order(), ["7", "1", "42", "300", "12"]);
    press(cx, "right space");
    assert_eq!(order(), ["1", "12", "7", "42", "300"], "by CPU, ascending");
    assert_eq!(
        *t.sorted.borrow(),
        ["name Ascending", "name Descending", "cpu Ascending"]
    );
    // Tab to the rows: Down selects the first, End the last, Up the one before.
    press(cx, "tab down");
    assert_eq!(selected().as_deref(), Some("1"));
    press(cx, "end up");
    assert_eq!(selected().as_deref(), Some("42"));
    press(cx, "enter");
    assert_eq!(*t.activated.borrow(), ["42"]);
    press(cx, "home");
    assert_eq!(selected().as_deref(), Some("1"));
    assert_eq!(*t.selected.borrow(), ["1", "300", "42", "1"]);
    press(cx, "tab enter");
    assert_eq!(t.after.get(), 1, "the rows' buttons are not tab stops");
    assert!(t.pressed.borrow().is_empty());
}

/// ADR 0070: the selection follows its row's key when the rows are replaced in another order.
#[gpui::test]
fn the_selection_follows_its_row(cx: &mut TestAppContext) {
    let t = Table::new(procs());
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    press(cx, "tab tab down down");
    assert_eq!(selected().as_deref(), Some("42"));
    t.rows.borrow_mut().reverse();
    redraw(cx);
    assert_eq!(order(), ["12", "300", "7", "42", "1"]);
    assert_eq!(selected().as_deref(), Some("42"), "the same process");
    press(cx, "down");
    assert_eq!(selected().as_deref(), Some("1"), "the row after it now");
}

/// ADR 0070: a thousand rows draw only those in view, and a key that selects one out of view
/// scrolls it in.
#[gpui::test]
fn only_the_rows_in_view_are_drawn(cx: &mut TestAppContext) {
    let many: Vec<Proc> = (0..1000).map(|i| Proc(i, "worker", f64::from(i))).collect();
    let t = Table::new(many);
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    let range: Range<usize> = drawn::last("t/drawn-rows");
    assert_eq!(range.start, 0);
    assert!(range.len() < 20, "{range:?}");
    press(cx, "tab tab end");
    assert_eq!(selected().as_deref(), Some("999"));
    let range: Range<usize> = drawn::last("t/drawn-rows");
    assert!(range.contains(&999), "scrolled to the last row: {range:?}");
    assert!(range.len() < 20);
}

/// Items 5, 38; ADR 0070: a press on a header sorts, a press on a row selects it, a press on the
/// selected row does not activate it unless asked, and a cell's button acts on its own row.
#[gpui::test]
fn the_pointer_sorts_selects_and_presses(cx: &mut TestAppContext) {
    let t = Table::new(procs());
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    click(cx, "t/head-1", MouseButton::Left);
    assert_eq!(order(), ["1", "12", "7", "42", "300"]);
    click(cx, "t/row-2", MouseButton::Left);
    assert_eq!(selected().as_deref(), Some("7"));
    click(cx, "t/row-2", MouseButton::Left);
    assert!(t.activated.borrow().is_empty());
    click(cx, "t/row-4", MouseButton::Left);
    assert_eq!(selected().as_deref(), Some("300"));
}

/// Items 13 and 10; ADR 0070: a right-click selects the row and opens its menu; the Menu key
/// opens the selected row's menu; Escape closes it.
#[gpui::test]
fn a_row_has_a_menu(cx: &mut TestAppContext) {
    let t = Table::new(procs());
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    reduce_motion();
    click(cx, "t/row-3", MouseButton::Right);
    redraw(cx);
    assert_eq!(selected().as_deref(), Some("300"));
    assert_eq!(probe::last("\"menu\""), Some(1.0), "the row's menu opened");
    press(cx, "escape");
    redraw(cx);
    assert_eq!(probe::last("\"menu\""), None);
    press(cx, "down menu");
    redraw(cx);
    assert_eq!(selected().as_deref(), Some("12"));
    assert_eq!(probe::last("\"menu\""), Some(1.0), "the Menu key opens it");
}

/// Item 23: an empty table says so.
#[gpui::test]
fn an_empty_table_says_so(cx: &mut TestAppContext) {
    let t = Table::new(Vec::new());
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    assert!(cx.debug_bounds("t/empty").is_some());
}

/// Item 7: an empty table that is loading shows skeleton rows instead, and reports itself busy.
#[gpui::test]
fn an_empty_table_that_is_loading_shows_skeletons(cx: &mut TestAppContext) {
    let t = Table::new(Vec::new());
    let build = t.build(true);
    let cx = open(cx, move |_, _| build());
    assert!(cx.debug_bounds("t/empty").is_none());
    let busy = DataTable::new("t", "Processes", Vec::<Proc>::new()).loading(true);
    assert!(busy.accessible().busy);
}

/// Items 3 and 39: a theme change keeps the sort and the selection.
#[gpui::test]
fn a_theme_change_keeps_the_sort_and_selection(cx: &mut TestAppContext) {
    let t = Table::new(procs());
    let build = t.build(false);
    let cx = open(cx, move |_, _| build());
    press(cx, "tab enter tab down");
    let before = (order(), selected());
    publish_fixture(FIXTURES[2].1);
    redraw(cx);
    assert_eq!((order(), selected()), before);
}

/// Items 14 to 17: the table, its headers with their sort and its rows with their cells' text.
#[test]
fn reports_the_table_its_headers_and_rows() {
    let table = DataTable::new("t", "Processes", procs())
        .row_key(|p: &Proc| p.0.to_string().into())
        .column(Column::new("name", "Name", |p: &Proc| p.1.into()).sort_by(|p: &Proc| p.1))
        .column(Column::new("cpu", "CPU", |p: &Proc| {
            format!("{}%", p.2).into()
        }))
        .default_sort("name", SortDirection::Ascending)
        .selected(Some("42".into()));
    let a = table.accessible();
    assert_eq!((a.role, a.name.as_str()), (Role::Table, "Processes"));
    let items = table.item_accessibles();
    assert_eq!(items[0].role, Role::ColumnHeader);
    assert_eq!(items[0].value.as_deref(), Some("ascending"));
    assert_eq!(items[1].value, None);
    let hypr = &items[3];
    assert_eq!(
        (hypr.role, hypr.name.as_str()),
        (Role::Row, "hyprland · 12.5%")
    );
    assert_eq!(hypr.selected, Some(true));
    assert_eq!(items.len(), 2 + 5);
}

/// Item 23: a name longer than its column ends in `…` inside its cell, and no cell leaves the
/// table, in a narrow window.
#[gpui::test]
fn worst_case_text_stays_in_its_cell(cx: &mut TestAppContext) {
    let long = "/usr/lib/a/very/long/unbroken/path/to/an/executable/that/never/wraps/日本語🔥";
    let t = Table::new(vec![Proc(1, long, 99.9), Proc(2, "x", 0.0)]);
    let build = t.build(false);
    let cx = open(cx, move |_, _| {
        v_flex().w(px(320.0)).h(px(240.0)).child(build())
    });
    let table = cx.debug_bounds("t/table").unwrap();
    for part in ["t/head-0", "t/head-1", "t/head-2", "t/row-0", "t/row-1"] {
        let b = cx.debug_bounds(part).unwrap_or_else(|| panic!("{part}"));
        assert!(
            b.right() <= table.right() + px(0.5),
            "{part} {b:?} leaves {table:?}"
        );
    }
}
