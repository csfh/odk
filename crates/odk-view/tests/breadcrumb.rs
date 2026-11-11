//! `Breadcrumb` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Breadcrumb, Face, Hosted, frame, headless};
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
    went: Vec<&'static str>,
    max: Option<usize>,
    last_is_item: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let crumbs = Breadcrumb::<App>::new("path")
        .item("Home", |a: &mut App| a.went.push("Home"))
        .item("Projects", |a: &mut App| a.went.push("Projects"))
        .item("Odk", |a: &mut App| a.went.push("Odk"))
        .item("Crates", |a: &mut App| a.went.push("Crates"));
    let crumbs = if hosted.app.last_is_item {
        crumbs.item("Readme", |a: &mut App| a.went.push("Readme"))
    } else {
        crumbs.current("Readme")
    };
    let crumbs = match hosted.app.max {
        Some(max) => crumbs.max_items(max),
        None => crumbs,
    };
    let body = col()
        .key("page")
        .padding(16.0)
        .child(crumbs.show(hosted, &mut layers));
    frame(hosted, cx, body, layers)
}

fn open(max: Option<usize>) -> Rt {
    open_with(App {
        max,
        ..App::default()
    })
}

fn open_with(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (640.0, 240.0),
    );
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

fn click(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime.find(key).unwrap_or_else(|| panic!("no {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
    into.push(node.clone());
    for child in node["children"].as_array().into_iter().flatten() {
        walk(child, into);
    }
}

fn nodes(runtime: &Rt) -> Vec<serde_json::Value> {
    let mut all = Vec::new();
    walk(&runtime.accessibility_json(), &mut all);
    all
}

#[test]
fn a_press_on_a_link_goes_there_and_the_current_page_is_not_a_link() {
    let mut runtime = open(None);
    click(&mut runtime, "path-item-1");
    assert_eq!(runtime.state.app.went, ["Projects"]);
    click(&mut runtime, "path-item-4");
    assert_eq!(runtime.state.app.went, ["Projects"], "the current page");
}

#[test]
fn the_last_item_is_the_current_page_even_when_given_a_handler() {
    let mut runtime = open_with(App {
        last_is_item: true,
        ..App::default()
    });
    click(&mut runtime, "path-item-4");
    assert!(runtime.state.app.went.is_empty(), "not a link");
    let all = nodes(&runtime);
    let last = all
        .iter()
        .find(|n| n["key"] == "path-item-4")
        .expect("the last item");
    assert_ne!(last["role"], "link", "{last}");
}

#[test]
fn tab_reaches_every_link_in_turn_and_enter_follows_one_but_never_the_current_page() {
    let mut runtime = open(None);
    let mut order = Vec::new();
    for _ in 0..4 {
        press(&mut runtime, Key::Tab);
        order.push(focused(&runtime));
    }
    let linked: Vec<_> = order
        .iter()
        .flatten()
        .filter(|k| k.starts_with("path-item-"))
        .collect();
    assert_eq!(
        linked,
        ["path-item-0", "path-item-1", "path-item-2", "path-item-3"],
        "{order:?}"
    );
    // The fourth stop is the last link; Enter follows it.
    runtime.state.app.went.clear();
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.went, ["Crates"]);
}

#[test]
fn it_is_a_navigation_of_links_ending_in_the_current_page() {
    let runtime = open(None);
    let all = nodes(&runtime);
    let nav = all
        .iter()
        .find(|n| n["key"] == "path")
        .expect("the breadcrumb");
    assert_eq!(nav["role"], "navigation", "{nav}");
    assert_eq!(nav["name"], "Breadcrumb");
    let current = all
        .iter()
        .find(|n| n["key"] == "path-item-4")
        .expect("the current page");
    assert_ne!(current["role"], "link", "{current}");
}

#[test]
fn a_long_path_folds_its_middle_into_a_menu_of_the_hidden_items() {
    let mut runtime = open(Some(3));
    assert!(has(&runtime, "path-item-0"));
    assert!(!has(&runtime, "path-item-1"), "folded");
    assert!(!has(&runtime, "path-item-2"), "folded");
    assert!(has(&runtime, "path-item-3"));
    assert!(has(&runtime, "path-item-4"));
    let fold = nodes(&runtime)
        .into_iter()
        .find(|n| n["key"] == "path-more")
        .expect("the fold");
    assert_eq!(fold["role"], "button", "{fold}");
    assert_eq!(fold["name"], "Show 2 hidden items");

    click(&mut runtime, "path-more");
    assert!(has(&runtime, "path-more-menu"), "opened by a press");
    assert_eq!(focused(&runtime).as_deref(), Some("path-more-item-0"));
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.went, ["Odk"], "the second hidden item");
    assert!(!has(&runtime, "path-more-menu"), "a choice closes it");
}

#[test]
fn the_fold_is_a_tab_stop_that_opens_on_enter_or_down_and_escape_gives_focus_back() {
    let mut runtime = open(Some(3));
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("path-more"));
    press(&mut runtime, Key::Down);
    assert!(has(&runtime, "path-more-menu"), "Down");
    press(&mut runtime, Key::Escape);
    assert!(!has(&runtime, "path-more-menu"));
    assert_eq!(focused(&runtime).as_deref(), Some("path-more"));
    press(&mut runtime, Key::Enter);
    assert!(
        has(&runtime, "path-more-menu"),
        "Enter: {:?}",
        focused(&runtime)
    );
}

#[test]
fn a_path_that_fits_has_no_fold() {
    let runtime = open(Some(5));
    assert!(!has(&runtime, "path-more"));
    assert!(has(&runtime, "path-item-1"));
}
