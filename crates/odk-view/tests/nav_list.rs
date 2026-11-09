//! `NavList` on the Rune host. Nothing here opens a window.

use std::sync::{Arc, OnceLock};

use odk_theme::ThemeSnapshot;
use odk_theme::component::{ComponentTokens, States};
use odk_theme::foundation::{FoundationConfig, FoundationTokens};
use odk_view::{Face, Hosted, Icon, NavList, frame, headless};
use rune::{Cx, El, Key, KeyEvent, Runtime, col, el};

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

fn tokens() -> ComponentTokens {
    let theme = ThemeSnapshot::emergency();
    let foundation = FoundationTokens::scaled(&FoundationConfig::default(), theme.text);
    ComponentTokens::new(
        Arc::new(foundation),
        theme.semantic.clone(),
        theme.generation,
    )
}

#[derive(Default)]
struct App {
    /// What the controlled list was last told.
    docs: Option<String>,
    /// Every page either list reported, in order.
    heard: Vec<String>,
}

type Rt = Runtime<Hosted<App>>;

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut docs = NavList::new("docs", "Documents")
        .item("inbox", "Inbox")
        .item("sent", "Sent")
        .on_change(|app: &mut App, value| {
            app.docs = Some(value.to_string());
            app.heard.push(format!("docs:{value}"));
        });
    if let Some(page) = &app.docs {
        docs = docs.value(page.clone());
    }
    frame(
        hosted,
        cx,
        col()
            .w(300.0)
            .gap(12.0)
            .child(el().key("before").focusable(true).size(20.0, 20.0))
            .child(
                NavList::new("settings", "Settings")
                    .group("Workspace")
                    .item("general", "General")
                    .item("members", "Members")
                    .item("billing", "Billing")
                    .item(
                        "audit",
                        "An audit log whose name is far longer than the sidebar",
                    )
                    .icon("members", Icon::Team)
                    .counter("members", "12")
                    .disable("billing")
                    .group("Account")
                    .item("profile", "Profile")
                    .item("security", "Security")
                    .default_value("general")
                    .on_change(|app: &mut App, value| app.heard.push(format!("settings:{value}")))
                    .show(hosted),
            )
            .child(docs.show(hosted))
            .child(
                NavList::<App>::new("locked", "Locked")
                    .item("a", "A")
                    .item("b", "B")
                    .default_value("a")
                    .disabled(true)
                    .show(hosted),
            )
            .child(el().key("after").focusable(true).size(20.0, 20.0)),
        [],
    )
}

fn open() -> Rt {
    headless(
        Hosted::new(
            App {
                docs: Some("sent".into()),
                ..App::default()
            },
            ThemeSnapshot::emergency(),
            face(),
        ),
        page,
        (500.0, 800.0),
    )
}

fn settle(runtime: &mut Rt) {
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key));
    settle(runtime);
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
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

fn states(runtime: &Rt, name: &str) -> Vec<String> {
    nodes(runtime)
        .iter()
        .find(|n| n["name"] == name)
        .unwrap_or_else(|| panic!("no node named {name}: {:#?}", nodes(runtime)))["states"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

#[test]
fn a_navigation_of_links_names_a_counter_and_marks_the_current_page() {
    let runtime = open();
    let all = nodes(&runtime);
    assert!(
        all.iter()
            .any(|n| n["role"] == "navigation" && n["name"] == "Settings"),
        "{all:#?}"
    );
    for name in ["General", "Members (12)", "Billing", "Profile", "Security"] {
        assert!(
            all.iter().any(|n| n["role"] == "link" && n["name"] == name),
            "no link named {name}"
        );
    }
    assert!(states(&runtime, "Billing").iter().any(|s| s == "disabled"));
    // Rune's tree has no current-page state for a link, so the current row's description says so.
    let description = |name: &str| {
        all.iter()
            .find(|n| n["name"] == name)
            .and_then(|n| n["description"].as_str().map(str::to_string))
    };
    assert_eq!(description("General").as_deref(), Some("Current page"));
    assert_eq!(description("Profile"), None);
}

#[test]
fn the_list_is_one_tab_stop_the_current_page_or_the_first_enabled_row() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..5 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    // Settings: its current page, the first row; Documents: its current page, the second; the locked
    // list has none.
    assert_eq!(
        visited,
        ["before", "settings-general", "docs-sent", "after", "before"]
    );
}

#[test]
fn up_and_down_move_wrapping_skipping_the_disabled_and_never_open_a_page() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("settings-general"));
    let mut path = Vec::new();
    for key in [
        Key::Down,
        Key::Down,
        Key::Down,
        Key::Down,
        Key::Down,
        Key::Up,
        Key::Home,
        Key::End,
    ] {
        press(&mut runtime, key);
        path.push(focused(&runtime).unwrap());
    }
    // General -> Members -> (Billing is disabled) Audit -> Profile -> Security -> wraps to
    // General; Up wraps to Security; Home -> General; End -> Security.
    assert_eq!(
        path,
        [
            "settings-members",
            "settings-audit",
            "settings-profile",
            "settings-security",
            "settings-general",
            "settings-security",
            "settings-general",
            "settings-security"
        ]
    );
    assert!(runtime.state.app.heard.is_empty(), "moving opens no page");
}

#[test]
fn enter_or_space_opens_the_focused_row_on_its_release_once_and_the_current_page_is_no_change() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Down);
    assert_eq!(focused(&runtime).as_deref(), Some("settings-members"));
    for (n, key) in [Key::Enter, Key::Text(" ".into())].into_iter().enumerate() {
        runtime.key(KeyEvent::from(key.clone()));
        runtime.repeat_key(KeyEvent::from(key.clone()));
        runtime.key(KeyEvent::from(key.clone()));
        settle(&mut runtime);
        assert_eq!(
            runtime.state.app.heard.len(),
            usize::from(n > 0),
            "nothing while held"
        );
        runtime.key_up(KeyEvent::from(key));
        settle(&mut runtime);
        if n == 0 {
            assert_eq!(
                runtime.state.app.heard,
                ["settings:members"],
                "once, on the release"
            );
        }
    }
    assert_eq!(
        runtime.state.app.heard,
        ["settings:members"],
        "opening the current page again is no change"
    );
    assert!(states(&runtime, "Members (12)") != states(&runtime, "General"));
}

#[test]
fn a_press_opens_a_page_and_a_controlled_list_follows_the_application() {
    let mut runtime = open();
    // The application starts Documents on Sent; opening Inbox tells it, and the list follows.
    tap(&mut runtime, "docs-inbox");
    assert_eq!(runtime.state.app.docs.as_deref(), Some("inbox"));
    assert_eq!(runtime.state.app.heard, ["docs:inbox"]);
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("docs-inbox"),
        "a press gives the row focus"
    );
    tap(&mut runtime, "settings-profile");
    // Billing is disabled and does nothing.
    let before = runtime.state.app.heard.len();
    tap(&mut runtime, "settings-billing");
    assert_eq!(runtime.state.app.heard.len(), before);
    assert_eq!(
        runtime.state.app.heard.last().map(String::as_str),
        Some("settings:profile")
    );
}

#[test]
fn the_current_page_has_an_indicator_along_its_edge_and_the_others_have_none() {
    let t = tokens();
    let indicator = t.nav_item(t.density(None), States::NONE.selected(true));
    let width = indicator.indicator_width;
    let runtime = open();
    let count = |key: &str| {
        let row = rect(&runtime, key);
        runtime
            .elements()
            .into_iter()
            .filter(|e| {
                e.rect[2] == width
                    && e.rect[3] == row[3]
                    && e.rect[0] == row[0]
                    && e.rect[1] == row[1]
            })
            .count()
    };
    assert_eq!(
        count("settings-general"),
        1,
        "the current page has an indicator"
    );
    assert_eq!(count("settings-profile"), 0, "the others have none");
}

#[test]
fn a_long_label_is_cut_and_a_counter_stays_at_the_end_of_its_row() {
    let runtime = open();
    let audit = rect(&runtime, "settings-audit");
    let label = runtime
        .elements()
        .into_iter()
        .find(|e| {
            e.text
                .as_deref()
                .is_some_and(|t| t.starts_with("An audit log"))
        })
        .expect("the label")
        .rect;
    assert!(
        label[0] + label[2] <= audit[0] + audit[2],
        "the label overflows its row"
    );
    let one_line = runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("General"))
        .expect("a one-line label")
        .rect[3];
    assert_eq!(label[3], one_line, "cut to one line, not wrapped");
    let members = rect(&runtime, "settings-members");
    let counter = runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("12"))
        .expect("the counter")
        .rect;
    assert!(
        counter[0] + counter[2] <= members[0] + members[2],
        "inside the row"
    );
    assert!(
        counter[0] > members[0] + members[2] / 2.0,
        "at the end of the row: {counter:?} {members:?}"
    );
}

#[test]
fn a_disabled_list_is_not_a_tab_stop() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..6 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    assert!(
        !visited.iter().any(|k| k.starts_with("locked")),
        "{visited:?}"
    );
}

#[test]
fn left_and_right_move_too_which_the_spec_does_not_ask_for_and_open_nothing() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("settings-general"));
    press(&mut runtime, Key::Right);
    assert_eq!(focused(&runtime).as_deref(), Some("settings-members"));
    press(&mut runtime, Key::Left);
    assert_eq!(focused(&runtime).as_deref(), Some("settings-general"));
    assert!(runtime.state.app.heard.is_empty());
}
