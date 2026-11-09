//! Icons and the layout groups on the Rune host. Nothing here opens a window.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_theme::semantic::Role as Status;
use odk_view::{Face, Hosted, Icon, Layout, Space, frame, headless};
use rune::{Cx, El, Runtime, el};

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

fn icons_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../odk-ui/icons/remixicon")
}

#[test]
fn the_icons_are_the_files_odk_ui_ships() {
    let mut shipped: Vec<String> = fs::read_dir(icons_dir())
        .expect("odk-ui's icons")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".svg"))
        .collect();
    shipped.sort();
    let mut ours: Vec<String> = Icon::ALL
        .iter()
        .map(|icon| icon.file().to_string())
        .collect();
    ours.sort();
    assert_eq!(ours, shipped);
    for icon in Icon::ALL {
        let text = fs::read_to_string(icons_dir().join(icon.file())).unwrap();
        assert!(text.contains("viewBox=\"0 0 24 24\""), "{icon:?}");
        assert!(text.contains("fill=\"currentColor\""), "{icon:?}");
    }
}

#[test]
fn rune_draws_every_icon() {
    for icon in Icon::ALL {
        let coverage = icon.coverage(24);
        assert_eq!(coverage.len(), 24 * 24, "{icon:?}");
        assert!(coverage.iter().any(|&c| c > 0), "{icon:?} drew nothing");
        // A filled glyph does not cover its whole square.
        assert!(coverage.contains(&0), "{icon:?} filled its box");
    }
}

#[test]
fn a_status_has_an_icon_so_it_reads_without_its_colour() {
    assert_eq!(Icon::status(Status::Info), Some(Icon::Info));
    assert_eq!(Icon::status(Status::Success), Some(Icon::Success));
    assert_eq!(Icon::status(Status::Warning), Some(Icon::Warning));
    assert_eq!(Icon::status(Status::Danger), Some(Icon::Danger));
    assert_eq!(Icon::status(Status::Accent), None);
    assert_eq!(Icon::status(Status::Neutral), None);
}

fn hosted() -> Hosted<()> {
    Hosted::new((), ThemeSnapshot::emergency(), face())
}

fn marks(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    frame(
        hosted,
        cx,
        layout
            .row(Space::Sm)
            .key("marks")
            .child(
                Icon::Check
                    .mark(16.0, rune::color::hex("#ffffff").unwrap())
                    .key("check"),
            )
            .child(
                Icon::Close
                    .mark(24.0, rune::color::hex("#ffffff").unwrap())
                    .key("close"),
            ),
        [],
    )
}

fn rect(runtime: &Runtime<Hosted<()>>, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

#[test]
fn a_mark_is_a_square_of_its_size_and_is_not_in_the_accessibility_tree() {
    let runtime = headless(hosted(), marks, (300.0, 100.0));
    let check = rect(&runtime, "check");
    let close = rect(&runtime, "close");
    assert_eq!((check[2], check[3]), (16.0, 16.0));
    assert_eq!((close[2], close[3]), (24.0, 24.0));
    let tree = runtime.accessibility_json().to_string();
    assert!(!tree.contains("\"image\""), "a mark is decoration: {tree}");
}

#[test]
fn the_spacing_steps_rise_and_a_row_and_a_stack_use_them() {
    let runtime = headless(hosted(), marks, (300.0, 100.0));
    let layout = Layout::of(&hosted());
    let steps = [Space::Xs, Space::Sm, Space::Md, Space::Lg, Space::Xl].map(|s| layout.space(s));
    assert!(steps[0] > 0.0);
    assert!(steps.windows(2).all(|pair| pair[0] < pair[1]), "{steps:?}");
    // The second mark starts a `Sm` after the first ends, and both are centred across.
    let (check, close) = (rect(&runtime, "check"), rect(&runtime, "close"));
    assert_eq!(close[0] - (check[0] + check[2]), layout.space(Space::Sm));
    assert_eq!(check[1] + check[3] / 2.0, close[1] + close[3] / 2.0);
}

fn stacked(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    frame(
        hosted,
        cx,
        layout
            .stack(Space::Lg)
            .child(el().key("a").size(40.0, 20.0))
            .child(el().key("b").size(40.0, 20.0)),
        [],
    )
}

#[test]
fn a_stack_sets_its_children_one_under_the_next() {
    let runtime = headless(hosted(), stacked, (300.0, 200.0));
    let layout = Layout::of(&hosted());
    let (a, b) = (rect(&runtime, "a"), rect(&runtime, "b"));
    assert_eq!(a[0], b[0]);
    assert_eq!(b[1] - (a[1] + a[3]), layout.space(Space::Lg));
}

fn cells(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let layout = Layout::of(hosted);
    let cell = |key: &'static str| el().key(key).h(30.0);
    frame(
        hosted,
        cx,
        layout.grid(cx, "grid", 3, [cell("one"), cell("two"), cell("three")]),
        [],
    )
}

#[test]
fn a_grid_has_as_many_columns_as_its_width_holds() {
    // The foundation's column is 520 points wide with 24 between, so two need 1064.
    let narrow = headless(hosted(), cells, (800.0, 400.0));
    let (one, two) = (rect(&narrow, "one"), rect(&narrow, "two"));
    assert_eq!(one[0], two[0], "one column: the cells stack");
    assert!(two[1] > one[1]);

    let wide = headless(hosted(), cells, (1200.0, 400.0));
    let (one, two, three) = (rect(&wide, "one"), rect(&wide, "two"), rect(&wide, "three"));
    assert_eq!(one[1], two[1], "two columns: the first row holds two");
    assert!(two[0] > one[0]);
    assert!(
        three[1] > one[1] && three[0] == one[0],
        "the third starts a row"
    );
}
