//! The layout groups beyond a stack: a stretching row, the sidebar frame, a grid with wide cells
//! and a page column that grows for it. Nothing here opens a window.

use std::cell::Cell as Counter;
use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Cell, Face, Hosted, Layout, Space, frame, headless};
use rune::{Cx, El, Runtime, col, el};

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

#[derive(Default)]
struct App {
    which: u8,
    columns: Counter<u16>,
}

fn block(key: &str, height: f32) -> El {
    el().key(key.to_string()).h(height).w_full()
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let layout = Layout::of(hosted);
    let body = match hosted.app.which {
        0 => layout
            .row_stretch(Space::Sm)
            .key("stretch")
            // Children with no height of their own, as tall as what they hold: a fixed height is
            // not stretched.
            .child(col().key("short").w(60.0).child(el().h(20.0)))
            .child(col().key("tall").w(60.0).child(el().h(90.0))),
        1 => layout.sidebar(block("nav", 10.0), col().key("content").w_full().h_full()),
        2 => layout.grid_of(
            "grid",
            2,
            [
                Cell::new(block("a", 30.0)),
                Cell::new(block("b", 50.0)),
                Cell::wide(block("wide", 40.0)),
                Cell::new(block("c", 30.0)),
            ],
        ),
        _ => {
            let column = layout.content_column(cx, "col", 2, 2);
            hosted.app.columns.set(column.columns());
            column.child(block("child", 20.0)).into_el()
        }
    };
    frame(hosted, cx, body, Vec::new())
}

fn open(which: u8, size: (f32, f32)) -> Rt {
    let mut runtime = headless(
        Hosted::new(
            App {
                which,
                ..App::default()
            },
            ThemeSnapshot::emergency(),
            face(),
        ),
        page,
        size,
    );
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
    runtime
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.5
}

#[test]
fn a_stretching_row_makes_its_children_as_tall_as_the_tallest() {
    let runtime = open(0, (400.0, 300.0));
    assert!(
        near(rect(&runtime, "short")[3], rect(&runtime, "tall")[3]),
        "{:?}",
        rect(&runtime, "short")
    );
}

#[test]
fn the_sidebar_frame_is_the_foundations_width_with_the_content_taking_the_rest_and_both_full_height()
 {
    let runtime = open(1, (800.0, 400.0));
    let tokens = runtime.state.components().sidebar();
    let sidebar = rect(&runtime, "odk-sidebar");
    let content = rect(&runtime, "content");
    assert!(
        near(sidebar[2], tokens.width),
        "{sidebar:?} against {}",
        tokens.width
    );
    assert!(
        near(sidebar[3], 400.0) && near(content[3], 400.0),
        "full height: {sidebar:?} {content:?}"
    );
    assert!(
        near(sidebar[2] + tokens.rule_width + content[2], 800.0),
        "the content takes the rest: {sidebar:?} {content:?}"
    );
    assert!(content[0] >= sidebar[0] + sidebar[2], "after the sidebar");
}

#[test]
fn a_grid_puts_equal_cells_in_rows_and_a_wide_cell_on_a_row_of_its_own() {
    let runtime = open(2, (600.0, 400.0));
    let (a, b, wide, c) = (
        rect(&runtime, "a"),
        rect(&runtime, "b"),
        rect(&runtime, "wide"),
        rect(&runtime, "c"),
    );
    assert!(
        near(a[1], b[1]) && b[0] > a[0] + a[2] - 1.0,
        "a and b share a row: {a:?} {b:?}"
    );
    assert!(near(a[2], b[2]), "equal columns: {a:?} {b:?}");
    assert!(
        wide[1] >= a[1] + a[3].max(b[3]),
        "the wide cell is below: {wide:?}"
    );
    assert!(
        near(wide[2], b[0] + b[2] - a[0]),
        "across every column: {wide:?} {a:?} {b:?}"
    );
    assert!(
        c[1] >= wide[1] + wide[3] && near(c[0], a[0]),
        "c starts the next row: {c:?}"
    );
}

#[test]
fn the_page_column_grows_for_two_columns_on_a_wide_window_and_not_on_a_narrow_one() {
    let wide = open(3, (1600.0, 400.0));
    let tokens = wide.state.foundation().layout.clone();
    assert_eq!(wide.state.app.columns.get(), 2, "room for two");
    assert!(
        rect(&wide, "child")[2] > tokens.content_width,
        "it grew past the content width: {:?} against {}",
        rect(&wide, "child"),
        tokens.content_width
    );
    assert!(rect(&wide, "child")[2] <= tokens.wide_content_width + 1.0);

    let narrow = open(3, (500.0, 400.0));
    assert_eq!(narrow.state.app.columns.get(), 1);
    assert!(rect(&narrow, "child")[2] <= tokens.content_width + 1.0);
}
