//! `ContextMenu` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, ContextMenu, Face, Hosted, Menu, MenuItem, Text, frame, headless};
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
    chosen: Vec<&'static str>,
    behind: u32,
    focusable: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let menu = Menu::<App>::new()
        .item(MenuItem::new("Rename").on_select(|app: &mut App| app.chosen.push("Rename")))
        .item(MenuItem::new("Copy").on_select(|app: &mut App| app.chosen.push("Copy")))
        .item(MenuItem::new("Locked").disabled(true))
        .item(
            MenuItem::new("Delete")
                .danger()
                .on_select(|app: &mut App| app.chosen.push("Delete")),
        );
    let trigger = ContextMenu::<App>::new(
        "ctx",
        "Row actions",
        col()
            .w(200.0)
            .h(80.0)
            .child(Text::new("Right-click me").show(hosted)),
    )
    .menu(menu)
    .focusable(hosted.app.focusable)
    .show(hosted, &mut layers);
    let body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(trigger)
        .child(Button::new("behind", "Behind", |app: &mut App| app.behind += 1).show(hosted));
    frame(hosted, cx, body, layers)
}

fn open(focusable: bool) -> Rt {
    let mut runtime = headless(
        Hosted::new(
            App {
                focusable,
                ..App::default()
            },
            ThemeSnapshot::emergency(),
            face(),
        ),
        page,
        (480.0, 360.0),
    );
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..24 {
        runtime.frame(1.0 / 60.0);
    }
}

fn right_click(runtime: &mut Rt, x: f32, y: f32) {
    runtime.secondary_down(x, y);
    runtime.secondary_up(x, y);
    settle(runtime);
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

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.5
}

#[test]
fn a_right_click_opens_the_menu_at_the_pointer_and_another_moves_it() {
    let mut runtime = open(true);
    assert!(!has(&runtime, "ctx-menu"));
    right_click(&mut runtime, 100.0, 40.0);
    let panel = rect(&runtime, "ctx-menu");
    assert!(
        near(panel[0], 100.0) && near(panel[1], 40.0),
        "at the pointer: {panel:?}"
    );
    press(&mut runtime, Key::Escape);
    right_click(&mut runtime, 160.0, 80.0);
    let panel = rect(&runtime, "ctx-menu");
    assert!(
        near(panel[0], 160.0) && near(panel[1], 80.0),
        "at the new point: {panel:?}"
    );
}

#[test]
fn the_menu_key_opens_it_below_the_trigger_and_shift_f10_does_too() {
    let mut runtime = open(true);
    press(&mut runtime, Key::Tab);
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("ctx"),
        "the trigger is a tab stop"
    );
    let trigger = rect(&runtime, "ctx");
    press(&mut runtime, Key::ContextMenu);
    assert!(has(&runtime, "ctx-menu"), "the Menu key");
    let panel = rect(&runtime, "ctx-menu");
    assert!(
        near(panel[0], trigger[0]) && near(panel[1], trigger[1] + trigger[3]),
        "below the trigger: {trigger:?} {panel:?}"
    );
}

#[test]
fn a_trigger_that_is_not_focusable_is_not_a_tab_stop() {
    let mut runtime = open(false);
    press(&mut runtime, Key::Tab);
    assert_ne!(focused(&runtime).as_deref(), Some("ctx"));
}

#[test]
fn it_opens_on_the_first_enabled_item_and_a_choice_runs_and_closes_it() {
    let mut runtime = open(true);
    right_click(&mut runtime, 100.0, 40.0);
    assert_eq!(focused(&runtime).as_deref(), Some("ctx-item-0"));
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Down);
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("ctx-item-3"),
        "past the disabled item"
    );
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.chosen, ["Delete"]);
    assert!(!has(&runtime, "ctx-menu"));
}

#[test]
fn escape_or_a_press_outside_closes_it_and_focus_goes_back_to_the_trigger() {
    let mut runtime = open(true);
    right_click(&mut runtime, 100.0, 40.0);
    press(&mut runtime, Key::Escape);
    assert!(!has(&runtime, "ctx-menu"));
    assert_eq!(focused(&runtime).as_deref(), Some("ctx"));

    let mut runtime = open(true);
    right_click(&mut runtime, 100.0, 40.0);
    runtime.pointer_down(460.0, 340.0);
    runtime.pointer_up(460.0, 340.0);
    settle(&mut runtime);
    assert!(!has(&runtime, "ctx-menu"), "a press outside");
    assert_eq!(
        runtime.state.app.behind, 0,
        "and it presses nothing beneath"
    );
}

#[test]
fn while_it_is_open_the_page_behind_takes_nothing_and_the_menu_is_named() {
    let mut runtime = open(true);
    right_click(&mut runtime, 100.0, 40.0);
    let (x, y) = runtime.find("behind").expect("the page's button");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.behind, 0);
    fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
        into.push(node.clone());
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, into);
        }
    }
    let mut nodes = Vec::new();
    walk(&runtime.accessibility_json(), &mut nodes);
    let menu = nodes
        .into_iter()
        .find(|n| n["key"] == "ctx-menu")
        .expect("the menu");
    assert_eq!(
        (menu["role"].as_str(), menu["name"].as_str()),
        (Some("menu"), Some("Row actions")),
        "{menu}"
    );
}
