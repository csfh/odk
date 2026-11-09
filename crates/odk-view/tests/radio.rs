//! `RadioGroup` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, RadioGroup, frame, headless};
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

#[derive(Default)]
struct App {
    /// What the controlled group was last told.
    size: Option<String>,
    /// Every choice either group reported, in order.
    heard: Vec<String>,
}

type Rt = Runtime<Hosted<App>>;

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut size = RadioGroup::new("size", "Size")
        .item("s", "Small")
        .item("m", "Medium")
        .item("l", "Large")
        .on_change(|app: &mut App, value| {
            app.size = Some(value.to_string());
            app.heard.push(format!("size:{value}"));
        });
    if let Some(chosen) = &app.size {
        size = size.value(chosen.clone());
    }
    frame(
        hosted,
        cx,
        col()
            .w(500.0)
            .gap(12.0)
            .child(el().key("before").focusable(true).size(20.0, 20.0))
            .child(
                RadioGroup::new("theme", "Theme")
                    .item("light", "Light")
                    .item("dark", "Dark")
                    .item("auto", "Automatic")
                    .item("contrast", "High contrast")
                    .disable("auto")
                    .default_value("dark")
                    .on_change(|app: &mut App, value| app.heard.push(format!("theme:{value}")))
                    .show(hosted),
            )
            .child(size.show(hosted))
            .child(
                RadioGroup::<App>::new("align", "Alignment")
                    .item("left", "Left")
                    .item("right", "Right")
                    .horizontal(true)
                    .default_value("left")
                    .show(hosted),
            )
            .child(
                RadioGroup::<App>::new("bad", "Bad")
                    .item("a", "A")
                    .item("b", "B")
                    .invalid(true)
                    .show(hosted),
            )
            .child(
                RadioGroup::<App>::new("locked", "Locked")
                    .item("a", "A")
                    .item("b", "B")
                    .disabled(true)
                    .invalid(true)
                    .show(hosted),
            )
            .child(el().key("after").focusable(true).size(20.0, 20.0)),
        [],
    )
}

fn open() -> Rt {
    headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        page,
        (600.0, 800.0),
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

fn states(runtime: &Rt, role: &str, name: &str) -> Vec<String> {
    nodes(runtime)
        .iter()
        .find(|n| n["role"] == role && n["name"] == name)
        .unwrap_or_else(|| panic!("no {role} named {name}"))["states"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn has(states: &[String], state: &str) -> bool {
    states.iter().any(|s| s == state)
}

#[test]
fn a_group_is_a_radiogroup_of_radios_and_the_chosen_one_is_checked() {
    let runtime = open();
    let all = nodes(&runtime);
    for name in ["Theme", "Size", "Alignment", "Bad", "Locked"] {
        assert!(
            all.iter()
                .any(|n| n["role"] == "radio-group" && n["name"] == name),
            "no radio group named {name}: {all:#?}"
        );
    }
    assert!(
        has(&states(&runtime, "radio-button", "Dark"), "checked"),
        "the default is chosen"
    );
    assert!(!has(&states(&runtime, "radio-button", "Light"), "checked"));
    assert!(has(
        &states(&runtime, "radio-button", "Automatic"),
        "disabled"
    ));
    let bad = states(&runtime, "radio-group", "Bad");
    assert!(has(&bad, "invalid"), "{bad:?}");
}

#[test]
fn a_group_has_one_tab_stop_the_chosen_option_or_the_first_enabled_one() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..8 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    // Theme's stop is the default, Dark; Size has none chosen, so its first; Alignment's is
    // Left; Bad has none chosen, so its first; the disabled group has none.
    assert_eq!(
        &visited[..7],
        [
            "before",
            "theme-dark",
            "size-s",
            "align-left",
            "bad-a",
            "after",
            "before"
        ],
        "{visited:?}"
    );
}

#[test]
fn arrows_move_and_choose_wrapping_and_skipping_the_disabled_and_home_and_end_go_to_the_ends() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("theme-dark"));
    let mut path = Vec::new();
    for key in [
        Key::Down,
        Key::Down,
        Key::Up,
        Key::Home,
        Key::End,
        Key::Right,
        Key::Left,
    ] {
        press(&mut runtime, key);
        path.push(focused(&runtime).unwrap());
    }
    // Dark -> (Automatic is disabled) Contrast -> wraps to Light -> back to Contrast -> Light
    // -> Contrast -> Light -> Contrast.
    assert_eq!(
        path,
        [
            "theme-contrast",
            "theme-light",
            "theme-contrast",
            "theme-light",
            "theme-contrast",
            "theme-light",
            "theme-contrast"
        ]
    );
    // Each move chose the option it reached, and told the application once.
    assert_eq!(
        runtime.state.app.heard.len(),
        7,
        "{:?}",
        runtime.state.app.heard
    );
    assert!(
        runtime
            .state
            .app
            .heard
            .iter()
            .all(|h| h.starts_with("theme:"))
    );
    assert!(has(
        &states(&runtime, "radio-button", "High contrast"),
        "checked"
    ));
    assert!(!has(&states(&runtime, "radio-button", "Dark"), "checked"));
}

#[test]
fn space_chooses_the_focused_option_once_and_enter_does_not() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab); // Size's stop, with nothing chosen
    assert_eq!(focused(&runtime).as_deref(), Some("size-s"));
    runtime.key(KeyEvent::from(Key::Enter));
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert!(runtime.state.app.heard.is_empty(), "Enter does not choose");
    let space = || KeyEvent::from(Key::Text(" ".into()));
    runtime.key(space());
    runtime.key_up(space());
    settle(&mut runtime);
    assert_eq!(runtime.state.app.heard, ["size:s"]);
    runtime.key(space());
    runtime.key_up(space());
    settle(&mut runtime);
    assert_eq!(
        runtime.state.app.heard,
        ["size:s"],
        "choosing the chosen one is no change"
    );
}

#[test]
fn a_press_on_a_circle_or_a_label_chooses_that_option_and_a_controlled_group_follows_the_application()
 {
    let mut runtime = open();
    for (key, side) in [("size-m", "circle"), ("size-l", "label")] {
        let row = rect(&runtime, key);
        let x = if side == "circle" {
            row[0] + 4.0
        } else {
            row[0] + row[2] - 4.0
        };
        runtime.pointer_down(x, row[1] + row[3] / 2.0);
        runtime.pointer_up(x, row[1] + row[3] / 2.0);
        settle(&mut runtime);
    }
    assert_eq!(runtime.state.app.heard, ["size:m", "size:l"]);
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("size-l"),
        "a press gives the option focus"
    );
    assert!(has(&states(&runtime, "radio-button", "Large"), "checked"));
    assert!(!has(&states(&runtime, "radio-button", "Medium"), "checked"));
}

#[test]
fn a_horizontal_group_lays_its_options_in_a_row_and_a_vertical_one_in_a_column() {
    let runtime = open();
    let (left, right) = (rect(&runtime, "align-left"), rect(&runtime, "align-right"));
    assert_eq!(left[1], right[1], "side by side");
    assert!(right[0] > left[0] + left[2] - 1.0);
    let (small, medium) = (rect(&runtime, "size-s"), rect(&runtime, "size-m"));
    assert_eq!(small[0], medium[0], "one under the next");
    assert!(medium[1] >= small[1] + small[3]);
}

#[test]
fn a_disabled_group_has_no_tab_stop_and_an_invalid_one_marks_its_first_option() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..8 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    assert!(
        !visited.iter().any(|k| k.starts_with("locked")),
        "{visited:?}"
    );
    // A wrong choice's mark sits after the first option's label, so that row is wider than the
    // second's. A disabled group draws none.
    let (a, b) = (rect(&runtime, "bad-a"), rect(&runtime, "bad-b"));
    assert!(a[2] > b[2], "{a:?} {b:?}");
    let (a, b) = (rect(&runtime, "locked-a"), rect(&runtime, "locked-b"));
    assert_eq!(a[2], b[2], "{a:?} {b:?}");
}
