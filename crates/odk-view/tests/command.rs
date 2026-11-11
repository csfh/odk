//! `Command` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, Command, CommandItem, Face, Hosted, frame, headless};
use rune::{Cx, El, Key, KeyEvent, Runtime, col};

type Rt = Runtime<Hosted<App>>;

const WINDOW: (f32, f32) = (640.0, 500.0);

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
    open: bool,
    many: bool,
    chosen: Vec<String>,
    behind: u32,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let item = |label: &'static str| {
        CommandItem::<App>::new(label).on_select(move |a: &mut App| a.chosen.push(label.into()))
    };
    let mut palette = Command::<App>::new("pal", "Command palette")
        .open(hosted.app.open)
        .on_close(|a: &mut App| a.open = false)
        .returns_focus_to("open")
        .item(item("Save file").hint("Ctrl+S").keywords("write disk"))
        .item(item("Open file").hint("Ctrl+O"))
        .item(item("Locked command").disabled(true))
        .item(item("Delete file"));
    if hosted.app.many {
        for n in 0..30 {
            let label: &'static str = Box::leak(format!("Extra {n:02}").into_boxed_str());
            palette = palette.item(item(label));
        }
    }
    palette.show(hosted, &mut layers);
    let body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(Button::new("open", "Open", |a: &mut App| a.open = true).show(hosted))
        .child(Button::new("behind", "Behind", |a: &mut App| a.behind += 1).show(hosted));
    frame(hosted, cx, body, layers)
}

fn start(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        WINDOW,
    );
    settle(&mut runtime);
    runtime
}

fn opened() -> Rt {
    let mut runtime = start(App::default());
    runtime.state.app.open = true;
    runtime.invalidate();
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..24 {
        runtime.frame(1.0 / 60.0);
    }
}

fn has(runtime: &Rt, key: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(key) && !e.exiting)
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

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key.clone()));
    runtime.key_up(KeyEvent::from(key));
    settle(runtime);
}

fn type_text(runtime: &mut Rt, text: &str) {
    press(runtime, Key::Text(text.into()));
}

fn click(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime.find(key).unwrap_or_else(|| panic!("no {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
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

/// The labels of the rows listed, top to bottom.
fn listed(runtime: &Rt) -> Vec<String> {
    let mut rows: Vec<_> = nodes(runtime)
        .into_iter()
        .filter(|n| n["role"] == "list-box-option")
        .collect();
    rows.sort_by_key(|n| {
        n["key"]
            .as_str()
            .and_then(|k| k.rsplit('-').next())
            .and_then(|i| i.parse::<usize>().ok())
            .unwrap_or(0)
    });
    rows.iter()
        .map(|n| n["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn has_state(node: &serde_json::Value, state: &str) -> bool {
    node["states"]
        .as_array()
        .is_some_and(|all| all.iter().any(|s| s == state))
}

/// The label of the row marked as the highlighted one.
fn highlighted(runtime: &Rt) -> Option<String> {
    nodes(runtime)
        .into_iter()
        .find(|n| n["role"] == "list-box-option" && has_state(n, "selected"))
        .and_then(|n| n["name"].as_str().map(str::to_string))
}

#[test]
fn it_opens_with_focus_in_an_empty_field_and_the_first_enabled_command_highlighted() {
    let runtime = opened();
    assert!(has(&runtime, "pal"));
    assert_eq!(focused(&runtime).as_deref(), Some("pal-field"));
    assert_eq!(
        listed(&runtime),
        ["Save file", "Open file", "Locked command", "Delete file"]
    );
    assert_eq!(highlighted(&runtime).as_deref(), Some("Save file"));
}

#[test]
fn the_panel_hangs_from_a_fixed_fraction_of_the_window_wherever_its_list_ends() {
    let mut runtime = opened();
    let top = rect(&runtime, "pal")[1];
    assert!(
        (top - 0.18 * WINDOW.1).abs() < 24.0,
        "panel top {top} for a {} tall window",
        WINDOW.1
    );
    type_text(&mut runtime, "zzz");
    assert!(
        (rect(&runtime, "pal")[1] - top).abs() < 1.5,
        "it does not move as the list narrows"
    );
}

#[test]
fn typing_narrows_by_every_word_in_the_label_or_keywords_and_says_when_nothing_matches() {
    let mut runtime = opened();
    type_text(&mut runtime, "fil");
    assert_eq!(
        listed(&runtime),
        ["Save file", "Open file", "Delete file"],
        "a part of a word"
    );
    type_text(&mut runtime, "e DISK");
    assert_eq!(
        listed(&runtime),
        ["Save file"],
        "two words, any case, one of them a keyword"
    );
    assert_eq!(highlighted(&runtime).as_deref(), Some("Save file"));
    type_text(&mut runtime, "x");
    assert!(listed(&runtime).is_empty());
    assert!(has(&runtime, "pal-empty"), "the empty line");
}

#[test]
fn the_arrows_move_the_highlight_past_a_disabled_command_and_wrap() {
    let mut runtime = opened();
    press(&mut runtime, Key::Down);
    assert_eq!(highlighted(&runtime).as_deref(), Some("Open file"));
    press(&mut runtime, Key::Down);
    assert_eq!(
        highlighted(&runtime).as_deref(),
        Some("Delete file"),
        "past the disabled one"
    );
    press(&mut runtime, Key::Down);
    assert_eq!(highlighted(&runtime).as_deref(), Some("Save file"), "wraps");
    press(&mut runtime, Key::Up);
    assert_eq!(highlighted(&runtime).as_deref(), Some("Delete file"));
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("pal-field"),
        "focus stays in the field"
    );
}

#[test]
fn enter_runs_the_highlighted_command_on_its_release_and_asks_to_close() {
    let mut runtime = opened();
    press(&mut runtime, Key::Down);
    runtime.key(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert!(
        runtime.state.app.chosen.is_empty(),
        "nothing on the press of Enter"
    );
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.chosen, ["Open file"]);
    assert!(!runtime.state.app.open);
    assert!(!has(&runtime, "pal"));
}

#[test]
fn enter_with_nothing_matching_does_nothing() {
    let mut runtime = opened();
    type_text(&mut runtime, "zzz");
    press(&mut runtime, Key::Enter);
    assert!(runtime.state.app.chosen.is_empty());
    assert!(runtime.state.app.open, "still open");
}

#[test]
fn a_press_runs_a_command_and_a_disabled_one_does_nothing() {
    let mut runtime = opened();
    click(&mut runtime, "pal-item-2");
    assert!(runtime.state.app.chosen.is_empty());
    assert!(runtime.state.app.open);
    click(&mut runtime, "pal-item-3");
    assert_eq!(runtime.state.app.chosen, ["Delete file"]);
    assert!(!runtime.state.app.open);
}

#[test]
fn the_pointer_over_a_command_highlights_it_and_enter_then_runs_that_one() {
    let mut runtime = opened();
    let (x, y) = runtime.find("pal-item-3").expect("the row");
    runtime.pointer_move(x, y);
    settle(&mut runtime);
    assert_eq!(highlighted(&runtime).as_deref(), Some("Delete file"));
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.chosen, ["Delete file"]);
}

#[test]
fn escape_or_a_press_on_the_scrim_closes_it_and_a_press_on_the_panel_does_not() {
    let mut runtime = opened();
    let panel = rect(&runtime, "pal");
    runtime.pointer_down(panel[0] + 4.0, panel[1] + 2.0);
    runtime.pointer_up(panel[0] + 4.0, panel[1] + 2.0);
    settle(&mut runtime);
    assert!(runtime.state.app.open, "the panel's own press");
    press(&mut runtime, Key::Escape);
    assert!(!runtime.state.app.open, "Escape");
    assert_eq!(focused(&runtime).as_deref(), Some("open"), "focus returns");

    let mut runtime = opened();
    runtime.pointer_down(8.0, 490.0);
    runtime.pointer_up(8.0, 490.0);
    settle(&mut runtime);
    assert!(!runtime.state.app.open, "the scrim");
    assert_eq!(
        runtime.state.app.behind, 0,
        "what lay behind was not pressed"
    );
}

#[test]
fn each_opening_starts_empty_with_the_first_command_highlighted() {
    let mut runtime = opened();
    type_text(&mut runtime, "del");
    press(&mut runtime, Key::Escape);
    runtime.state.app.open = true;
    runtime.invalidate();
    settle(&mut runtime);
    assert_eq!(listed(&runtime).len(), 4, "every command again");
    assert_eq!(highlighted(&runtime).as_deref(), Some("Save file"));
    // And what is typed now goes into an empty field.
    type_text(&mut runtime, "o");
    assert_eq!(listed(&runtime), ["Open file", "Locked command"]);
}

#[test]
fn the_page_behind_is_inert_and_the_tree_names_the_dialog_the_list_and_each_option() {
    let mut runtime = opened();
    let (x, y) = runtime.find("behind").expect("the button behind");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.behind, 0);

    runtime.state.app.open = true;
    runtime.invalidate();
    settle(&mut runtime);
    let all = nodes(&runtime);
    let dialog = all.iter().find(|n| n["key"] == "pal").expect("the panel");
    assert_eq!(dialog["role"], "dialog", "{dialog}");
    assert_eq!(dialog["name"], "Command palette");
    let list = all
        .iter()
        .find(|n| n["key"] == "pal-list")
        .expect("the list");
    assert_eq!(list["role"], "list-box", "{list}");
    let locked = all
        .iter()
        .find(|n| n["name"] == "Locked command")
        .expect("the disabled option");
    assert!(has_state(locked, "disabled"), "{locked}");
    let field = all
        .iter()
        .find(|n| n["key"] == "pal-field")
        .expect("the field");
    assert_eq!(field["name"], "Search…");
}

#[test]
fn a_long_list_scrolls_to_keep_the_highlighted_command_in_view() {
    let mut runtime = start(App {
        many: true,
        ..App::default()
    });
    runtime.state.app.open = true;
    runtime.invalidate();
    settle(&mut runtime);
    let list = rect(&runtime, "pal-list");
    assert!(list[3] <= 330.0, "no taller than the list's cap: {list:?}");
    for _ in 0..20 {
        press(&mut runtime, Key::Down);
    }
    let row = rect(&runtime, "pal-item-20");
    let list = rect(&runtime, "pal-list");
    assert!(
        row[1] >= list[1] - 1.0 && row[1] + row[3] <= list[1] + list[3] + 1.0,
        "row {row:?} inside list {list:?}"
    );
}
