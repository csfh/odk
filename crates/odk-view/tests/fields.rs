//! `SearchInput` and `PasswordInput` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Dialog, Face, Hosted, PasswordInput, SearchInput, frame, headless};
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
    changes: Vec<String>,
    submits: u32,
    dialog: bool,
    seeded: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let mut search = SearchInput::new(
        "q",
        "Search files",
        |a: &mut App, text: String| a.changes.push(text),
        |a: &mut App| a.submits += 1,
    )
    .shortcut("Ctrl+K");
    if hosted.app.seeded {
        search = search.default_value("seed");
    }
    let search = search.show(hosted);
    let password = PasswordInput::new(
        "pw",
        "Password",
        |a: &mut App, text: String| a.changes.push(format!("pw:{text}")),
        |_: &mut App| {},
    )
    .show(hosted);
    if hosted.app.dialog {
        Dialog::<App>::new("dlg", "Find")
            .open(true)
            .on_close(|a: &mut App| a.dialog = false)
            .content(
                SearchInput::new(
                    "dq",
                    "Search in dialog",
                    |a: &mut App, text: String| a.changes.push(text),
                    |_: &mut App| {},
                )
                .show(hosted),
            )
            .show(hosted, &mut layers);
    }
    let body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(search)
        .child(password);
    frame(hosted, cx, body, layers)
}

fn start(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (520.0, 360.0),
    );
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..12 {
        runtime.frame(1.0 / 60.0);
    }
}

fn has(runtime: &Rt, key: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
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

fn node(runtime: &Rt, key: &str) -> serde_json::Value {
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
    walk(&runtime.accessibility_json(), key).unwrap_or_else(|| panic!("no node {key}"))
}

#[test]
fn the_clear_button_shows_with_text_clears_it_and_leaves_focus_in_the_field() {
    let mut runtime = start(App::default());
    assert!(!has(&runtime, "q-clear"), "nothing to clear yet");
    tap(&mut runtime, "q");
    type_text(&mut runtime, "ab");
    assert!(has(&runtime, "q-clear"));
    tap(&mut runtime, "q-clear");
    assert_eq!(
        runtime.state.app.changes.last().map(String::as_str),
        Some("")
    );
    assert!(!has(&runtime, "q-clear"), "empty again");
    assert_eq!(focused(&runtime).as_deref(), Some("q"), "focus stays");
}

#[test]
fn the_clear_button_is_not_a_tab_stop() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "q");
    type_text(&mut runtime, "a");
    press(&mut runtime, Key::Tab);
    assert_ne!(focused(&runtime).as_deref(), Some("q-clear"));
}

#[test]
fn escape_clears_a_field_with_text_and_an_empty_one_leaves_it_to_the_dialog() {
    let mut runtime = start(App {
        dialog: true,
        ..App::default()
    });
    assert!(runtime.state.app.dialog);
    tap(&mut runtime, "dq");
    type_text(&mut runtime, "ab");
    press(&mut runtime, Key::Escape);
    assert!(runtime.state.app.dialog, "the first Escape only cleared");
    assert_eq!(
        runtime.state.app.changes.last().map(String::as_str),
        Some("")
    );
    press(&mut runtime, Key::Escape);
    assert!(!runtime.state.app.dialog, "the second one closed it");
}

#[test]
fn enter_submits_a_search_and_a_default_value_is_there_without_a_change() {
    let mut runtime = start(App {
        seeded: true,
        ..App::default()
    });
    assert!(
        runtime.state.app.changes.is_empty(),
        "a seed is not an edit"
    );
    assert!(has(&runtime, "q-clear"), "the seed counts as text");
    tap(&mut runtime, "q");
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.submits, 1);
}

#[test]
fn a_password_is_masked_reports_no_value_and_writes_no_primary_selection() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "pw");
    type_text(&mut runtime, "hunter2");
    assert!(
        runtime
            .state
            .app
            .changes
            .last()
            .is_some_and(|c| c == "pw:hunter2")
    );
    press(&mut runtime, Key::Command('a'));
    assert_eq!(
        runtime.state.ui.primary(),
        "",
        "a masked selection is not offered to a middle click"
    );
    let field = node(&runtime, "pw");
    let shown = field["value"].as_str().unwrap_or_default();
    assert!(!shown.contains("hunter2"), "no value: {field}");
}

#[test]
fn a_search_selection_does_write_the_primary_selection() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "q");
    type_text(&mut runtime, "abc");
    press(&mut runtime, Key::Command('a'));
    assert_eq!(runtime.state.ui.primary(), "abc");
}

#[test]
fn the_show_button_is_a_tab_stop_that_names_what_it_does_and_keeps_the_text() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "pw");
    type_text(&mut runtime, "ab");
    let button = node(&runtime, "pw-reveal");
    assert_eq!(button["name"], "Show password", "{button}");
    let before = node(&runtime, "pw")["selection"].clone();
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("pw-reveal"));
    press(&mut runtime, Key::Enter);
    assert_eq!(node(&runtime, "pw-reveal")["name"], "Hide password");
    // Rune still withholds a masked field's value from the tree while it is revealed; the caret
    // is where it was.
    let field = node(&runtime, "pw");
    assert_eq!(field["selection"], before, "caret kept: {field}");
    press(&mut runtime, Key::Enter);
    assert_eq!(node(&runtime, "pw-reveal")["name"], "Show password");
}
