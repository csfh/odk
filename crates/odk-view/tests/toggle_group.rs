//! `ToggleGroup` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Glyph, Hosted, Icon, ToggleGroup, frame, headless};
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
    sizes: Option<Vec<String>>,
    /// Every change any group reported, as `group:values`.
    heard: Vec<String>,
}

type Rt = Runtime<Hosted<App>>;

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut sizes = ToggleGroup::new("sizes", "Size")
        .item("s", "Small")
        .item("m", "Medium")
        .item("l", "Large")
        .on_change(|app: &mut App, values| {
            app.sizes = Some(values.to_vec());
            app.heard.push(format!("sizes:{}", values.join(",")));
        });
    if let Some(values) = &app.sizes {
        sizes = sizes.value(values.clone());
    }
    frame(
        hosted,
        cx,
        col()
            .w(500.0)
            .gap(12.0)
            .child(el().key("before").focusable(true).size(20.0, 20.0))
            .child(
                ToggleGroup::new("view", "View")
                    .item("list", "List")
                    .item("grid", "Grid")
                    .item("columns", "Columns")
                    .item("gallery", "Gallery")
                    .disable("grid")
                    .default_value(["columns"])
                    .on_change(|app: &mut App, values| {
                        app.heard.push(format!("view:{}", values.join(",")))
                    })
                    .show(hosted),
            )
            .child(
                ToggleGroup::new("format", "Format")
                    .multiple(true)
                    .item("bold", "Bold")
                    .item("italic", "Italic")
                    .item("underline", "Underline")
                    .default_value(["bold"])
                    .on_change(|app: &mut App, values| {
                        app.heard.push(format!("format:{}", values.join(",")))
                    })
                    .show(hosted),
            )
            .child(sizes.show(hosted))
            .child(
                ToggleGroup::<App>::new("icons", "Layout")
                    .icon_item("rows", Glyph::Icon(Icon::List), "Rows")
                    .icon_item("tiles", Icon::Grid, "Tiles")
                    .item("text", "Text")
                    .show(hosted),
            )
            .child(
                col().w(160.0).child(
                    ToggleGroup::<App>::new("narrow", "Narrow")
                        .item("a", "A label far longer than the room")
                        .item("b", "Another long label for the group")
                        .show(hosted),
                ),
            )
            .child(
                ToggleGroup::<App>::new("locked", "Locked")
                    .item("a", "A")
                    .item("b", "B")
                    .disabled(true)
                    .show(hosted),
            )
            .child(ToggleGroup::<App>::new("empty", "Empty").show(hosted))
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

fn states(runtime: &Rt, name: &str) -> Vec<String> {
    nodes(runtime)
        .iter()
        .find(|n| n["name"] == name && n["role"] != "radio-group" && n["role"] != "group")
        .unwrap_or_else(|| panic!("no option named {name}: {:#?}", nodes(runtime)))["states"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn on(runtime: &Rt, name: &str) -> bool {
    states(runtime, name).iter().any(|s| s == "checked")
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
fn a_single_group_is_a_radio_group_a_multiple_one_a_group_and_the_default_is_on() {
    let runtime = open();
    let all = nodes(&runtime);
    for (role, name) in [
        ("radio-group", "View"),
        ("group", "Format"),
        ("radio-group", "Size"),
    ] {
        assert!(
            all.iter().any(|n| n["role"] == role && n["name"] == name),
            "no {role} named {name}"
        );
    }
    assert!(on(&runtime, "Columns"), "the default of View is on");
    assert!(!on(&runtime, "List"));
    assert!(on(&runtime, "Bold") && !on(&runtime, "Italic"));
    // An icon alone is named by the name it was given.
    states(&runtime, "Rows");
    states(&runtime, "Tiles");
    assert!(states(&runtime, "Grid").iter().any(|s| s == "disabled"));
}

#[test]
fn a_group_has_one_tab_stop_the_selected_option_or_the_first_enabled_one() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..9 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    // View's stop is its default, Columns; Format's is Bold; Size has none on, so its first;
    // Layout's is its first; Narrow's is its first; the locked group has none; the empty
    // group draws nothing.
    assert_eq!(
        visited,
        [
            "before",
            "view-columns",
            "format-bold",
            "sizes-s",
            "icons-rows",
            "narrow-a",
            "after",
            "before",
            "view-columns"
        ]
    );
}

#[test]
fn arrows_move_without_choosing_skipping_the_disabled_and_home_and_end_go_to_the_ends() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("view-columns"));
    let mut path = Vec::new();
    for key in [
        Key::Right,
        Key::Right,
        Key::Left,
        Key::Home,
        Key::Right,
        Key::End,
    ] {
        press(&mut runtime, key);
        path.push(focused(&runtime).unwrap());
    }
    // Columns -> Gallery -> (wraps) List -> Gallery; Home -> List; Right skips the disabled
    // Grid to Columns; End -> Gallery.
    assert_eq!(
        path,
        [
            "view-gallery",
            "view-list",
            "view-gallery",
            "view-list",
            "view-columns",
            "view-gallery"
        ]
    );
    assert!(runtime.state.app.heard.is_empty(), "moving chooses nothing");
    assert!(on(&runtime, "Columns"), "and Columns is still on");
}

#[test]
fn space_or_enter_toggles_the_focused_option_on_its_release_and_a_held_key_once() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Right);
    assert_eq!(focused(&runtime).as_deref(), Some("view-gallery"));
    for (n, key) in [Key::Enter, Key::Text(" ".into())].into_iter().enumerate() {
        runtime.key(KeyEvent::from(key.clone()));
        runtime.repeat_key(KeyEvent::from(key.clone()));
        runtime.key(KeyEvent::from(key.clone()));
        settle(&mut runtime);
        assert_eq!(runtime.state.app.heard.len(), n, "nothing while it is held");
        runtime.key_up(KeyEvent::from(key));
        settle(&mut runtime);
        assert_eq!(runtime.state.app.heard.len(), n + 1, "once, on the release");
    }
    assert_eq!(runtime.state.app.heard, ["view:gallery", "view:gallery"]);
    assert!(
        on(&runtime, "Gallery") && !on(&runtime, "Columns"),
        "single mode: one is on"
    );
}

#[test]
fn single_mode_turns_one_on_and_multiple_mode_toggles_each_and_a_controlled_group_follows() {
    let mut runtime = open();
    tap(&mut runtime, "view-list");
    assert!(on(&runtime, "List") && !on(&runtime, "Columns"));
    tap(&mut runtime, "format-italic");
    tap(&mut runtime, "format-bold");
    assert!(on(&runtime, "Italic") && !on(&runtime, "Bold"));
    assert_eq!(
        &runtime.state.app.heard[1..],
        ["format:bold,italic", "format:italic"]
    );
    tap(&mut runtime, "sizes-m");
    tap(&mut runtime, "sizes-l");
    assert_eq!(runtime.state.app.sizes, Some(vec!["l".to_string()]));
    assert!(
        on(&runtime, "Large") && !on(&runtime, "Medium"),
        "the application's value"
    );
    // A disabled option does nothing.
    let before = runtime.state.app.heard.len();
    tap(&mut runtime, "view-grid");
    assert_eq!(runtime.state.app.heard.len(), before);
}

#[test]
fn an_icon_alone_is_a_square_and_a_long_label_is_cut_inside_a_narrow_column() {
    let runtime = open();
    let rows = rect(&runtime, "icons-rows");
    assert_eq!(rows[2], rows[3], "an icon alone is a square segment");
    let text = rect(&runtime, "icons-text");
    assert!(text[2] > rows[2], "a text option is wider than an icon");
    let narrow = rect(&runtime, "narrow");
    assert!(
        narrow[2] <= 160.0,
        "the group overflows its column: {narrow:?}"
    );
    // Every label stays on one line.
    let one_line = rect(&runtime, "view-list")[3];
    assert_eq!(rect(&runtime, "narrow-a")[3], one_line);
    let text_height = |starts: &str| {
        runtime
            .elements()
            .into_iter()
            .find(|e| e.text.as_deref().is_some_and(|t| t.starts_with(starts)))
            .unwrap_or_else(|| panic!("no text {starts}"))
            .rect[3]
    };
    assert_eq!(
        text_height("A label far"),
        text_height("List"),
        "a long label is cut to one line, not wrapped"
    );
    assert!(rect(&runtime, "narrow-b")[0] + rect(&runtime, "narrow-b")[2] <= narrow[0] + narrow[2]);
}

#[test]
fn a_disabled_group_is_not_a_tab_stop_and_a_group_without_options_draws_nothing() {
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
    assert!(
        runtime
            .elements()
            .iter()
            .all(|e| e.key.as_deref() != Some("empty")),
        "a group with no options has no frame"
    );
}
