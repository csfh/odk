//! `Resizable` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, Resizable, ResizablePanel, Text, frame, headless};
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

#[derive(Default)]
struct App {
    vertical: bool,
    disabled: bool,
    /// What `on_layout` last heard, and how many times it spoke.
    heard: Vec<f32>,
    reports: usize,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let panels = |group: Resizable<App>| {
        group
            .panel(
                ResizablePanel::new()
                    .default_size(0.4)
                    .min_size(0.25)
                    .child(Text::new("List").show(hosted)),
            )
            .panel(
                ResizablePanel::new()
                    .min_size(0.3)
                    .child(Text::new("Detail").show(hosted)),
            )
            .disabled(hosted.app.disabled)
            .on_layout(|app: &mut App, sizes: &[f32]| {
                app.heard = sizes.to_vec();
                app.reports += 1;
            })
            .show(hosted)
    };
    let group = if hosted.app.vertical {
        panels(Resizable::vertical("split", "Services and detail"))
    } else {
        panels(Resizable::horizontal("split", "Services and detail"))
    };
    let body = col()
        .key("page")
        .padding(0.0)
        .child(col().key("box").w(500.0).h(300.0).child(group));
    frame(hosted, cx, body, [])
}

fn open(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (500.0, 300.0),
    );
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..12 {
        runtime.frame(1.0 / 60.0);
    }
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

/// The first panel's share of the two panels, along the group.
fn share(runtime: &Rt, vertical: bool) -> f32 {
    let (a, b) = (
        rect(runtime, "split-panel-0"),
        rect(runtime, "split-panel-1"),
    );
    let (a, b) = if vertical { (a[3], b[3]) } else { (a[2], b[2]) };
    a / (a + b)
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key.clone()));
    runtime.key_up(KeyEvent::from(key));
    settle(runtime);
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

#[test]
fn the_panels_take_their_fractions_and_the_handle_sits_between() {
    let runtime = open(App::default());
    let s = share(&runtime, false);
    assert!((s - 0.4).abs() < 0.01, "the list is 40%: {s}");
    let (list, detail) = (
        rect(&runtime, "split-panel-0"),
        rect(&runtime, "split-panel-1"),
    );
    let handle = rect(&runtime, "split-handle-0");
    let middle = handle[0] + handle[2] / 2.0;
    assert!(
        middle >= list[0] + list[2] - 1.0 && middle <= detail[0] + 1.0,
        "handle at {middle}, between {} and {}",
        list[0] + list[2],
        detail[0]
    );
}

#[test]
fn a_vertical_group_stacks_its_panels() {
    let runtime = open(App {
        vertical: true,
        ..App::default()
    });
    let s = share(&runtime, true);
    assert!((s - 0.4).abs() < 0.01, "the top panel is 40%: {s}");
    let (top, bottom) = (
        rect(&runtime, "split-panel-0"),
        rect(&runtime, "split-panel-1"),
    );
    assert!(bottom[1] >= top[1] + top[3], "one under the other");
}

#[test]
fn a_handle_moves_from_the_keyboard_within_the_limits_and_reports_each_move() {
    let mut runtime = open(App::default());
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("split-handle-0"));
    press(&mut runtime, Key::Right);
    assert!((share(&runtime, false) - 0.45).abs() < 0.01);
    assert_eq!(runtime.state.app.reports, 1);
    assert!((runtime.state.app.heard[0] - 0.45).abs() < 1e-4);
    press(&mut runtime, Key::Home);
    assert!(
        (share(&runtime, false) - 0.25).abs() < 0.01,
        "the smallest the list can be"
    );
    press(&mut runtime, Key::Left);
    assert_eq!(
        runtime.state.app.reports, 2,
        "already at its limit: nothing to report"
    );
    press(&mut runtime, Key::End);
    assert!(
        (share(&runtime, false) - 0.7).abs() < 0.01,
        "the detail keeps its 30%"
    );
}

#[test]
fn a_column_is_moved_with_up_and_down_and_not_left_and_right() {
    let mut runtime = open(App {
        vertical: true,
        ..App::default()
    });
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Right);
    assert_eq!(runtime.state.app.reports, 0);
    press(&mut runtime, Key::Down);
    assert!((share(&runtime, true) - 0.45).abs() < 0.01);
}

#[test]
fn a_drag_follows_the_pointer_and_reports_once() {
    let mut runtime = open(App::default());
    let (x, y) = runtime.find("split-handle-0").expect("the handle");
    runtime.pointer_down(x, y);
    runtime.pointer_move(x + 50.0, y);
    settle(&mut runtime);
    assert!(share(&runtime, false) > 0.45, "moved while held");
    assert_eq!(runtime.state.app.reports, 0, "nothing is reported mid-drag");
    runtime.pointer_move(x + 50.0, y + 40.0);
    runtime.pointer_up(x + 50.0, y + 40.0);
    settle(&mut runtime);
    let s = share(&runtime, false);
    assert!(
        (s - 0.5).abs() < 0.02,
        "50 of 500 points is a tenth of the group: {s}"
    );
    assert_eq!(runtime.state.app.reports, 1);
    assert!((runtime.state.app.heard[0] - s).abs() < 0.02);
}

#[test]
fn escape_puts_a_drag_back_and_reports_nothing() {
    let mut runtime = open(App::default());
    let (x, y) = runtime.find("split-handle-0").expect("the handle");
    runtime.pointer_down(x, y);
    runtime.pointer_move(x + 80.0, y);
    settle(&mut runtime);
    assert!(share(&runtime, false) > 0.5);
    runtime.key(KeyEvent::from(Key::Escape));
    runtime.pointer_up(x + 80.0, y);
    settle(&mut runtime);
    assert!(
        (share(&runtime, false) - 0.4).abs() < 0.01,
        "back where it began"
    );
    assert_eq!(runtime.state.app.reports, 0);
}

#[test]
fn a_disabled_group_does_not_move_and_takes_no_focus() {
    let mut runtime = open(App {
        disabled: true,
        ..App::default()
    });
    press(&mut runtime, Key::Tab);
    assert_ne!(focused(&runtime).as_deref(), Some("split-handle-0"));
    let (x, y) = runtime.find("split-handle-0").expect("the handle");
    runtime.pointer_down(x, y);
    runtime.pointer_move(x + 60.0, y);
    runtime.pointer_up(x + 60.0, y);
    settle(&mut runtime);
    assert!((share(&runtime, false) - 0.4).abs() < 0.01);
    assert_eq!(runtime.state.app.reports, 0);
    // Rune leaves an element that neither takes focus nor acts out of the tree, so a disabled
    // handle is not there for a screen reader to land on; `item_accessibles` still reports it
    // disabled to the checklist.
    assert!(
        nodes(&runtime).iter().all(|n| n["key"] != "split-handle-0"),
        "a disabled handle is out of the tree"
    );
}

#[test]
fn each_handle_is_a_separator_named_for_the_group_with_the_panels_size_as_its_value() {
    let runtime = open(App::default());
    let all = nodes(&runtime);
    let handle = all
        .iter()
        .find(|n| n["key"] == "split-handle-0")
        .expect("the handle");
    assert_eq!(handle["role"], "splitter", "{handle}");
    assert_eq!(handle["name"], "Services and detail");
    assert_eq!(handle["value"], "40%", "{handle}");
}
