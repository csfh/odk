//! `DataTable` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Column, DataTable, Face, Hosted, MenuItem, SortDirection, frame, headless};
use rune::{Cx, El, Key, KeyEvent, Runtime, col};

type Rt = Runtime<Hosted<App>>;

fn face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    *FACE.get_or_init(|| {
        let output = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", "sans"])
            .output()
            .expect("fc-match");
        assert!(output.status.success(), "fc-match sans");
        let path = String::from_utf8(output.stdout).expect("fc-match speaks utf-8");
        let bytes = std::fs::read(path.trim()).expect("read the sans font");
        Face::leak(bytes, "Inter".into())
    })
}

struct Proc {
    pid: u32,
    name: String,
    cpu: f64,
}

#[derive(Default)]
struct App {
    rows: Vec<Proc>,
    selects: Vec<String>,
    activates: Vec<String>,
    sorts: Vec<(String, SortDirection)>,
    acted: Vec<String>,
    menu_chosen: Vec<String>,
    activate_on_click: bool,
    default_sorted: bool,
    loading: bool,
}

fn people() -> Vec<Proc> {
    [
        ("Cedar", 2.5),
        ("Alder", 40.0),
        ("Birch", 10.0),
        ("Dogwood", 0.5),
    ]
    .iter()
    .enumerate()
    .map(|(i, (name, cpu))| Proc {
        pid: 100 + i as u32,
        name: (*name).into(),
        cpu: *cpu,
    })
    .collect()
}

fn many(n: u32) -> Vec<Proc> {
    (0..n)
        .map(|i| Proc {
            pid: i,
            name: format!("Process {i:04}"),
            cpu: f64::from(i % 7),
        })
        .collect()
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let mut table = DataTable::<App, Proc>::new(
        "t",
        "Processes",
        |a: &App| a.rows.as_slice(),
        |p: &Proc| p.pid.to_string(),
    )
    .column(
        Column::new("name", "Name", |p: &Proc| p.name.clone())
            .grow(2.0)
            .sort_by(|p: &Proc| p.name.clone()),
    )
    .column(
        Column::new("cpu", "CPU", |p: &Proc| format!("{:.1}%", p.cpu))
            .chars(7)
            .align_end()
            .sort_by(|p: &Proc| p.cpu),
    )
    .column(Column::new("pid", "PID", |p: &Proc| p.pid.to_string()).chars(6))
    .column(Column::action(
        "end",
        "",
        |_: &Proc| Some("End".to_string()),
        |a: &mut App, key: &str| a.acted.push(key.into()),
    ))
    .on_select(|a: &mut App, key: &str| a.selects.push(key.into()))
    .on_activate(|a: &mut App, key: &str| a.activates.push(key.into()))
    .on_sort(|a: &mut App, key: &str, d| a.sorts.push((key.into(), d)))
    .activate_on_click(hosted.app.activate_on_click)
    .loading(hosted.app.loading)
    .row_menu(|p: &Proc, menu| {
        let name = p.name.clone();
        menu.item(
            MenuItem::new("Kill").on_select(move |a: &mut App| a.menu_chosen.push(name.clone())),
        )
    })
    .height(240.0);
    if hosted.app.default_sorted {
        table = table.default_sort("cpu", SortDirection::Descending);
    }
    let body = col()
        .key("page")
        .w_full()
        .padding(8.0)
        .child(table.show(hosted, &mut layers));
    frame(hosted, cx, body, layers)
}

fn start(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (640.0, 420.0),
    );
    settle(&mut runtime);
    runtime
}

fn people_app() -> App {
    App {
        rows: people(),
        ..App::default()
    }
}

fn settle(runtime: &mut Rt) {
    for _ in 0..24 {
        runtime.frame(1.0 / 60.0);
    }
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key.clone()));
    runtime.key_up(KeyEvent::from(key));
    settle(runtime);
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime.find(key).unwrap_or_else(|| panic!("no {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

/// Tab to the rows: past the headers' stop, to the rows' own.
fn focus_rows(runtime: &mut Rt) {
    for _ in 0..3 {
        if focused(runtime).as_deref() == Some("t-body") {
            return;
        }
        press(runtime, Key::Tab);
    }
    assert_eq!(focused(runtime).as_deref(), Some("t-body"));
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

fn has(runtime: &Rt, key: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

/// The keys of the rows built, top to bottom.
fn rows_built(runtime: &Rt) -> Vec<String> {
    let mut rows: Vec<_> = runtime
        .elements()
        .into_iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("t-row-") && !k.contains("-cell-"))
        })
        .collect();
    rows.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    rows.into_iter().filter_map(|e| e.key).collect()
}

fn nodes(runtime: &Rt) -> Vec<serde_json::Value> {
    fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
        into.push(node.clone());
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, into);
        }
    }
    let mut all = Vec::new();
    walk(&runtime.accessibility_json(), &mut all);
    all
}

fn has_state(node: &serde_json::Value, state: &str) -> bool {
    node["states"]
        .as_array()
        .is_some_and(|all| all.iter().any(|s| s == state))
}

/// The names of the rows in the tree, in the order they are drawn.
fn names(runtime: &Rt) -> Vec<String> {
    rows_built(runtime)
        .iter()
        .map(|key| {
            nodes(runtime)
                .into_iter()
                .find(|n| n["key"] == key.as_str())
                .and_then(|n| n["name"].as_str().map(str::to_string))
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn it_is_a_table_of_rows_and_cells_with_named_column_headers() {
    let runtime = start(people_app());
    let all = nodes(&runtime);
    let table = all.iter().find(|n| n["key"] == "t").expect("the table");
    assert_eq!(table["role"], "table", "{table}");
    assert_eq!(table["name"], "Processes");
    let headers: Vec<_> = all
        .iter()
        .filter(|n| n["role"] == "column-header")
        .map(|n| n["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(headers, ["Name", "CPU", "PID", ""]);
    assert_eq!(names(&runtime), ["Cedar", "Alder", "Birch", "Dogwood"]);
    let cell = all
        .iter()
        .find(|n| n["key"] == "t-cell-101-cpu")
        .expect("a cell");
    assert_eq!(cell["role"], "cell", "{cell}");
    assert_eq!(cell["name"], "40.0%");
}

#[test]
fn a_press_selects_a_row_once_and_a_press_on_the_selected_row_activates_when_asked() {
    let mut runtime = start(people_app());
    tap(&mut runtime, "t-row-101");
    assert_eq!(runtime.state.app.selects, ["101"]);
    tap(&mut runtime, "t-row-101");
    assert_eq!(
        runtime.state.app.selects,
        ["101"],
        "no change, nothing heard"
    );
    assert!(runtime.state.app.activates.is_empty());
    let row = nodes(&runtime)
        .into_iter()
        .find(|n| n["key"] == "t-row-101")
        .expect("the row");
    assert!(has_state(&row, "selected"), "{row}");

    let mut runtime = start(App {
        activate_on_click: true,
        ..people_app()
    });
    tap(&mut runtime, "t-row-102");
    assert!(
        runtime.state.app.activates.is_empty(),
        "the first press selects"
    );
    tap(&mut runtime, "t-row-102");
    assert_eq!(runtime.state.app.activates, ["102"]);
}

#[test]
fn a_header_sorts_and_chosen_again_reverses_and_a_column_that_does_not_sort_does_nothing() {
    let mut runtime = start(people_app());
    tap(&mut runtime, "t-h-name");
    assert_eq!(names(&runtime), ["Alder", "Birch", "Cedar", "Dogwood"]);
    assert_eq!(
        runtime.state.app.sorts,
        [("name".to_string(), SortDirection::Ascending)]
    );
    tap(&mut runtime, "t-h-name");
    assert_eq!(names(&runtime), ["Dogwood", "Cedar", "Birch", "Alder"]);
    let header = nodes(&runtime)
        .into_iter()
        .find(|n| n["key"] == "t-h-name")
        .expect("the header");
    assert_eq!(header["value"], "descending", "{header}");
    tap(&mut runtime, "t-h-cpu");
    assert_eq!(
        names(&runtime),
        ["Dogwood", "Cedar", "Birch", "Alder"],
        "numbers, not text"
    );
    let before = runtime.state.app.sorts.len();
    tap(&mut runtime, "t-h-pid");
    assert_eq!(runtime.state.app.sorts.len(), before, "PID does not sort");
}

#[test]
fn the_default_sort_starts_it_and_a_selected_row_stays_selected_through_a_sort() {
    let mut runtime = start(App {
        default_sorted: true,
        ..people_app()
    });
    assert_eq!(
        names(&runtime),
        ["Alder", "Birch", "Cedar", "Dogwood"],
        "by CPU, highest first"
    );
    tap(&mut runtime, "t-row-100");
    tap(&mut runtime, "t-h-name");
    tap(&mut runtime, "t-h-name");
    let selected: Vec<_> = nodes(&runtime)
        .into_iter()
        .filter(|n| n["role"] == "row" && has_state(n, "selected"))
        .map(|n| n["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(selected, ["Cedar"], "the same row, wherever it went");
}

#[test]
fn the_sortable_headers_are_one_tab_stop_and_the_rows_are_the_next() {
    let mut runtime = start(people_app());
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("t-h-name"));
    press(&mut runtime, Key::Right);
    assert_eq!(focused(&runtime).as_deref(), Some("t-h-cpu"));
    press(&mut runtime, Key::Enter);
    assert_eq!(
        runtime.state.app.sorts.last().map(|s| s.0.as_str()),
        Some("cpu")
    );
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("t-body"));
}

#[test]
fn the_arrows_home_end_and_page_keys_move_the_selection_and_enter_activates_on_release() {
    let mut runtime = start(App {
        rows: many(60),
        ..App::default()
    });
    focus_rows(&mut runtime);
    press(&mut runtime, Key::Down);
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("0")
    );
    press(&mut runtime, Key::Down);
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("1")
    );
    press(&mut runtime, Key::Up);
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("0")
    );
    press(&mut runtime, Key::End);
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("59")
    );
    press(&mut runtime, Key::Down);
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("59"),
        "no further"
    );
    press(&mut runtime, Key::Home);
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("0")
    );
    press(&mut runtime, Key::PageDown);
    let after: usize = runtime
        .state
        .app
        .selects
        .last()
        .expect("a selection")
        .parse()
        .expect("a key");
    assert!(after > 2 && after < 60, "a page down moved to {after}");

    runtime.key(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert!(runtime.state.app.activates.is_empty(), "not on the press");
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.activates.len(), 1);
}

#[test]
fn only_the_rows_in_view_are_built_and_the_selected_row_is_scrolled_into_view() {
    let mut runtime = start(App {
        rows: many(2000),
        ..App::default()
    });
    assert!(
        rows_built(&runtime).len() < 120,
        "{} rows built",
        rows_built(&runtime).len()
    );
    focus_rows(&mut runtime);
    press(&mut runtime, Key::End);
    for _ in 0..60 {
        runtime.frame(1.0 / 60.0);
    }
    assert!(has(&runtime, "t-row-1999"), "the last row is built");
    let body = runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("t-body"))
        .expect("the body")
        .rect;
    let last = runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("t-row-1999"))
        .expect("the last row")
        .rect;
    assert!(
        last[1] >= body[1] - 1.0 && last[1] + last[3] <= body[1] + body[3] + 1.0,
        "row {last:?} in body {body:?}"
    );
}

#[test]
fn a_right_click_or_the_menu_key_opens_the_menu_of_that_row_and_a_choice_runs() {
    let mut runtime = start(people_app());
    let (x, y) = runtime.find("t-row-102").expect("a row");
    runtime.secondary_down(x, y);
    runtime.secondary_up(x, y);
    settle(&mut runtime);
    assert!(has(&runtime, "t-ctx-menu"), "opened on the row");
    assert_eq!(
        runtime.state.app.selects.last().map(String::as_str),
        Some("102"),
        "it selects the row too"
    );
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.menu_chosen, ["Birch"]);
    assert!(!has(&runtime, "t-ctx-menu"));

    tap(&mut runtime, "t-row-100");
    focus_rows(&mut runtime);
    press(&mut runtime, Key::ContextMenu);
    assert!(
        has(&runtime, "t-ctx-menu"),
        "the Menu key, for the selected row"
    );
}

#[test]
fn an_action_button_takes_the_pointer_only_and_hears_its_rows_key() {
    let mut runtime = start(people_app());
    tap(&mut runtime, "t-cell-103-end-button");
    assert_eq!(runtime.state.app.acted, ["103"]);
    for _ in 0..6 {
        press(&mut runtime, Key::Tab);
        assert_ne!(
            focused(&runtime).as_deref(),
            Some("t-cell-103-end-button"),
            "not a tab stop"
        );
    }
}

#[test]
fn with_no_rows_it_says_so_and_while_loading_shows_skeleton_rows() {
    let runtime = start(App::default());
    assert!(has(&runtime, "t-empty"));
    assert!(rows_built(&runtime).is_empty());
    let runtime = start(App {
        loading: true,
        ..App::default()
    });
    assert!(has(&runtime, "t-loading-0") && has(&runtime, "t-loading-2"));
    assert!(!has(&runtime, "t-empty"));
}
