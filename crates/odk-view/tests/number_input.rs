//! `NumberInput` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, NumberInput, frame, headless};
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
    changes: Vec<Option<f64>>,
    commits: Vec<f64>,
    submits: Vec<Option<f64>>,
    bounded: bool,
    fine: bool,
    read_only: bool,
    seeded: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut n = NumberInput::<App>::new("n", "Quantity")
        .on_change(|a: &mut App, v| a.changes.push(v))
        .on_commit(|a: &mut App, v| a.commits.push(v))
        .on_submit(|a: &mut App, v| a.submits.push(v))
        .read_only(hosted.app.read_only);
    if hosted.app.bounded {
        n = n.min(0.0).max(10.0);
    }
    if hosted.app.fine {
        n = n.step(0.1);
    }
    if hosted.app.seeded {
        n = n.default_value(3.0);
    }
    let body = col().key("page").padding(16.0).child(n.show(hosted));
    frame(hosted, cx, body, Vec::new())
}

fn start(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (420.0, 240.0),
    );
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..12 {
        runtime.frame(1.0 / 60.0);
    }
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key.clone()));
    runtime.key_up(KeyEvent::from(key));
    settle(runtime);
}

fn type_text(runtime: &mut Rt, text: &str) {
    for c in text.chars() {
        press(runtime, Key::Text(c.to_string()));
    }
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime.find(key).unwrap_or_else(|| panic!("no {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

fn node(runtime: &Rt, key: &str) -> Option<serde_json::Value> {
    fn walk(node: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
        if node["key"] == key {
            return Some(node.clone());
        }
        node["children"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|c| walk(c, key))
    }
    walk(&runtime.accessibility_json(), key)
}

fn text(runtime: &Rt) -> String {
    node(runtime, "n").expect("the field")["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn has_state(node: &serde_json::Value, state: &str) -> bool {
    node["states"]
        .as_array()
        .is_some_and(|all| all.iter().any(|s| s == state))
}

fn focus(runtime: &mut Rt) {
    tap(runtime, "n");
}

#[test]
fn typing_is_free_the_application_hears_the_number_and_nothing_is_committed_until_enter() {
    let mut runtime = start(App::default());
    focus(&mut runtime);
    type_text(&mut runtime, "2.50");
    assert_eq!(runtime.state.app.changes.last(), Some(&Some(2.5)));
    assert!(runtime.state.app.commits.is_empty(), "not yet");
    assert_eq!(text(&runtime), "2.50", "untidy until committed");
    press(&mut runtime, Key::Enter);
    assert_eq!(text(&runtime), "2.5", "tidied");
    assert_eq!(runtime.state.app.commits, [2.5]);
    assert_eq!(runtime.state.app.submits, [Some(2.5)]);
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.commits, [2.5], "once for a change");
    assert_eq!(
        runtime.state.app.submits.len(),
        2,
        "but every Enter submits"
    );
}

#[test]
fn a_commit_clamps_to_the_bounds() {
    let mut runtime = start(App {
        bounded: true,
        ..App::default()
    });
    focus(&mut runtime);
    type_text(&mut runtime, "99");
    assert_eq!(text(&runtime), "99", "typing is not clamped");
    press(&mut runtime, Key::Enter);
    assert_eq!(text(&runtime), "10");
    assert_eq!(runtime.state.app.commits, [10.0]);
}

#[test]
fn text_that_is_no_number_is_kept_and_marked_and_commits_nothing() {
    let mut runtime = start(App::default());
    focus(&mut runtime);
    type_text(&mut runtime, "-");
    assert_eq!(runtime.state.app.changes.last(), Some(&None));
    let field = node(&runtime, "n").expect("the field");
    assert!(has_state(&field, "invalid"), "{field}");
    assert_eq!(field["description"], "Enter a number", "{field}");
    press(&mut runtime, Key::Enter);
    assert_eq!(text(&runtime), "-", "nothing typed is lost");
    assert!(runtime.state.app.commits.is_empty());
    assert_eq!(runtime.state.app.submits, [None]);
}

#[test]
fn up_and_down_step_and_stop_at_the_bounds() {
    let mut runtime = start(App {
        bounded: true,
        ..App::default()
    });
    focus(&mut runtime);
    type_text(&mut runtime, "9");
    press(&mut runtime, Key::Up);
    assert_eq!(text(&runtime), "10");
    press(&mut runtime, Key::Up);
    assert_eq!(text(&runtime), "10", "no higher than max");
    press(&mut runtime, Key::Down);
    assert_eq!(text(&runtime), "9");
    assert_eq!(runtime.state.app.commits, [10.0, 9.0], "a step commits");
}

#[test]
fn ten_steps_of_a_tenth_are_exactly_one() {
    let mut runtime = start(App {
        fine: true,
        ..App::default()
    });
    focus(&mut runtime);
    for _ in 0..10 {
        press(&mut runtime, Key::Up);
    }
    assert_eq!(text(&runtime), "1");
}

#[test]
fn the_steppers_step_leave_focus_in_the_field_and_are_not_tab_stops() {
    let mut runtime = start(App {
        bounded: true,
        ..App::default()
    });
    focus(&mut runtime);
    type_text(&mut runtime, "10");
    let up = node(&runtime, "n-increase").expect("the increase button");
    assert!(has_state(&up, "disabled"), "at max: {up}");
    tap(&mut runtime, "n-decrease");
    assert_eq!(text(&runtime), "9");
    assert_eq!(focused(&runtime).as_deref(), Some("n"), "focus stays");
    press(&mut runtime, Key::Tab);
    assert_ne!(focused(&runtime).as_deref(), Some("n-increase"));
}

#[test]
fn a_read_only_field_shows_its_number_and_neither_steps_nor_has_steppers() {
    let mut runtime = start(App {
        read_only: true,
        seeded: true,
        ..App::default()
    });
    assert!(node(&runtime, "n-increase").is_none());
    focus(&mut runtime);
    press(&mut runtime, Key::Up);
    assert_eq!(text(&runtime), "3");
    assert!(runtime.state.app.commits.is_empty());
}

#[test]
fn a_starting_number_is_shown_without_a_change() {
    let runtime = start(App {
        seeded: true,
        ..App::default()
    });
    assert_eq!(text(&runtime), "3");
    assert!(runtime.state.app.changes.is_empty());
}
