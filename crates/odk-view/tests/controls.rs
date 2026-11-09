//! `Button`, `Input` and `Select` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, Face, Hosted, Input, Select, frame, headless};
use rune::{Cx, El, Key, KeyEvent, PointerButton, col};

/// Values and callbacks. An open list, a caret and a held key are not fields of this model.
#[derive(Default)]
struct App {
    presses: u32,
    enabled: bool,
    busy: bool,
    chosen: Option<usize>,
    name: String,
    other: String,
    submits: u32,
}

impl App {
    fn ready() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

fn fruits() -> Vec<String> {
    ["Apple", "Banana", "Cherry"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

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

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let body = col()
        .key("page")
        .padding(24.0)
        .gap(16.0)
        .child(
            Button::new("save", "Save", |app: &mut App| app.presses += 1)
                .disabled(!hosted.app.enabled)
                .busy(hosted.app.busy)
                .show(hosted),
        )
        .child(
            Select::new(
                "fruit",
                "Fruit",
                fruits(),
                hosted.app.chosen,
                |app: &mut App, index| app.chosen = Some(index),
            )
            .show(hosted, &mut layers),
        )
        .child(
            Input::new(
                "name",
                "Name",
                |app: &mut App, text| app.name = text,
                |app: &mut App| app.submits += 1,
            )
            .show(hosted),
        )
        .child(
            Input::new(
                "other",
                "Other",
                |app: &mut App, text| app.other = text,
                |_: &mut App| {},
            )
            .show(hosted),
        );
    frame(hosted, cx, body, layers)
}

fn open(app: App) -> rune::Runtime<Hosted<App>> {
    headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (480.0, 360.0),
    )
}

fn settle(runtime: &mut rune::Runtime<Hosted<App>>) {
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
}

fn focused(runtime: &rune::Runtime<Hosted<App>>) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|element| element.focused)
        .and_then(|element| element.key)
}

fn tap(runtime: &mut rune::Runtime<Hosted<App>>, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

fn press(runtime: &mut rune::Runtime<Hosted<App>>, key: Key) {
    runtime.key(KeyEvent::from(key));
    settle(runtime);
}

fn ancestor_keys(runtime: &rune::Runtime<Hosted<App>>, key: &str) -> Vec<String> {
    let elements = runtime.elements();
    let mut index = elements
        .iter()
        .position(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"));
    let mut keys = Vec::new();
    while let Some(parent) = elements[index].parent {
        index = parent;
        if let Some(key) = elements[index].key.clone() {
            keys.push(key);
        }
    }
    keys
}

#[test]
fn enter_and_space_activate_on_release_once_per_hold() {
    let mut runtime = open(App::ready());
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime), Some("save".into()));
    for key in [Key::Enter, Key::Text(" ".into())] {
        let before = runtime.state.app.presses;
        runtime.key(KeyEvent::from(key.clone()));
        runtime.repeat_key(KeyEvent::from(key.clone()));
        runtime.key(KeyEvent::from(key.clone()));
        settle(&mut runtime);
        assert_eq!(
            runtime.state.app.presses, before,
            "{key:?} held down has not pressed yet"
        );
        runtime.key_up(KeyEvent::from(key.clone()));
        settle(&mut runtime);
        assert_eq!(
            runtime.state.app.presses,
            before + 1,
            "{key:?}'s release presses once"
        );
    }
}

#[test]
fn a_press_released_off_the_button_does_nothing() {
    let mut runtime = open(App::ready());
    let (x, y) = runtime.find("save").expect("save");
    runtime.pointer_down(x, y);
    runtime.pointer_move(8.0, 8.0);
    runtime.pointer_up(8.0, 8.0);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.presses, 0);
    tap(&mut runtime, "save");
    assert_eq!(runtime.state.app.presses, 1);
}

#[test]
fn a_disabled_or_busy_button_does_not_activate() {
    let mut runtime = open(App::ready());
    runtime.state.app.enabled = false;
    runtime.invalidate();
    settle(&mut runtime);
    tap(&mut runtime, "save");
    press(&mut runtime, Key::Tab);
    // A disabled control leaves the Tab order: the first stop is the next control.
    assert_eq!(focused(&runtime), Some("fruit-trigger".into()));
    runtime.key(KeyEvent::from(Key::Enter));
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    runtime.key(KeyEvent::from(Key::Escape));
    runtime.key_up(KeyEvent::from(Key::Escape));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.presses, 0, "disabled");

    runtime.state.app.enabled = true;
    runtime.state.app.busy = true;
    runtime.invalidate();
    settle(&mut runtime);
    tap(&mut runtime, "save");
    runtime.key(KeyEvent::from(Key::Text(" ".into())));
    runtime.key_up(KeyEvent::from(Key::Text(" ".into())));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.presses, 0, "busy");
}

#[test]
fn the_select_opens_moves_and_chooses_without_an_open_field_on_the_model() {
    let mut runtime = open(App::ready());
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime), Some("fruit-trigger".into()));
    for key in [Key::Enter, Key::Text(" ".into()), Key::Down, Key::Up] {
        press(&mut runtime, key);
        assert!(runtime.find("fruit-list").is_some(), "the list is open");
        assert_eq!(runtime.state.app.chosen, None);
        let ancestors = ancestor_keys(&runtime, "fruit-list");
        assert!(
            ancestors.iter().any(|key| key == "odk-root"),
            "{ancestors:?}"
        );
        assert!(
            !ancestors.iter().any(|key| key == "page"),
            "the list is a layer above the page"
        );
        let (_, trigger_y) = runtime.find("fruit-trigger").unwrap();
        let (_, option_y) = runtime.find("fruit-0").unwrap();
        assert!(option_y > trigger_y, "the list opens under its trigger");
        press(&mut runtime, Key::Escape);
        assert!(
            runtime.find("fruit-list").is_none(),
            "Escape closes without choosing"
        );
        assert_eq!(runtime.state.app.chosen, None);
        assert_eq!(focused(&runtime), Some("fruit-trigger".into()));
    }

    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Up);
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::End);
    press(&mut runtime, Key::Home);
    press(&mut runtime, Key::Text("c".into()));
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.chosen, Some(2));
    assert!(runtime.find("fruit-list").is_none());
}

#[test]
fn a_click_chooses_a_select_option() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "fruit-trigger");
    assert!(runtime.find("fruit-list").is_some());
    tap(&mut runtime, "fruit-1");
    assert_eq!(runtime.state.app.chosen, Some(1));
    assert!(runtime.find("fruit-list").is_none());
}

#[test]
fn typing_edits_the_input_and_enter_submits_once() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "name");
    for character in "Ada".chars() {
        press(&mut runtime, Key::Text(character.to_string()));
    }
    assert_eq!(runtime.state.app.name, "Ada");
    runtime.key(KeyEvent::from(Key::Enter));
    runtime.repeat_key(KeyEvent::from(Key::Enter));
    runtime.key(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert_eq!(
        runtime.state.app.submits, 0,
        "Enter held down has not submitted"
    );
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.submits, 1);
}

#[test]
fn a_keyboard_selection_writes_the_primary_selection() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "name");
    for character in "Ada".chars() {
        press(&mut runtime, Key::Text(character.to_string()));
    }
    press(&mut runtime, Key::Command('a'));
    assert_eq!(runtime.state.ui.primary(), "Ada");
}

#[test]
fn a_mouse_selection_writes_the_primary_selection_and_a_middle_click_pastes_it() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "name");
    for character in "Banana".chars() {
        press(&mut runtime, Key::Text(character.to_string()));
    }
    let ([x, y, width, height], _) = runtime.presented("name").expect("name");
    let from_x = x + 4.0;
    let to_x = x + width - 4.0;
    let at_y = y + height / 2.0;
    runtime.pointer_down(from_x, at_y);
    runtime.pointer_move(to_x, at_y);
    runtime.pointer_up(to_x, at_y);
    settle(&mut runtime);
    let primary = runtime.state.ui.primary();
    assert!(!primary.is_empty(), "a drag writes the primary selection");
    assert!(
        runtime.state.app.name.contains(&primary),
        "{primary:?} is not in the field"
    );

    let (ox, oy) = runtime.find("other").expect("other");
    runtime.button_down(ox, oy, PointerButton::Middle);
    runtime.button_up(ox, oy, PointerButton::Middle);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.other, primary);
}

#[test]
fn a_programmatic_selection_does_not_write_the_primary_selection() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "name");
    for character in "Ada".chars() {
        press(&mut runtime, Key::Text(character.to_string()));
    }
    press(&mut runtime, Key::Command('a'));
    assert_eq!(runtime.state.ui.primary(), "Ada");
    runtime.state.ui.set_input_selection("name", 0, 1);
    runtime.invalidate();
    settle(&mut runtime);
    assert_eq!(runtime.state.ui.primary(), "Ada");
    let (x, y) = runtime.find("other").expect("other");
    runtime.button_down(x, y, PointerButton::Middle);
    runtime.button_up(x, y, PointerButton::Middle);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.other, "Ada");
}

/// The accessibility tree's node for `key`, searched depth first.
fn node(tree: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    if tree["key"] == key {
        return Some(tree.clone());
    }
    tree["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

fn has_state(node: &serde_json::Value, state: &str) -> bool {
    node["states"]
        .as_array()
        .is_some_and(|states| states.iter().any(|s| s == state))
}

#[test]
fn each_control_is_in_the_tree_with_the_role_and_name_it_describes() {
    let runtime = open(App::ready());
    let tree = runtime.accessibility_json();
    for (key, role, name) in [
        ("save", "button", "Save"),
        ("fruit-trigger", "combo-box", "Fruit"),
        ("name", "text-input", "Name"),
        ("other", "text-input", "Other"),
    ] {
        let found = node(&tree, key).unwrap_or_else(|| panic!("no node {key}"));
        assert_eq!(found["role"], role, "{key}");
        assert_eq!(found["name"], name, "{key}");
    }
}

#[test]
fn a_disabled_button_is_dimmed_in_the_tree() {
    let mut runtime = open(App::ready());
    assert!(!has_state(
        &node(&runtime.accessibility_json(), "save").unwrap(),
        "disabled"
    ));
    runtime.state.app.enabled = false;
    runtime.invalidate();
    settle(&mut runtime);
    let save = node(&runtime.accessibility_json(), "save").unwrap();
    assert!(has_state(&save, "disabled"), "{save}");
}

#[test]
fn an_open_list_describes_a_listbox_of_options() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "fruit-trigger");
    let tree = runtime.accessibility_json();
    let list = node(&tree, "fruit-list").expect("the open list");
    assert_eq!(list["role"], "list-box");
    assert_eq!(list["name"], "Fruit");
    let banana = node(&tree, "fruit-1").expect("an option");
    assert_eq!(banana["role"], "list-box-option");
    assert_eq!(banana["name"], "Banana");
}

#[test]
fn the_chosen_option_is_the_select_s_value_and_the_selected_option() {
    let mut runtime = open(App::ready());
    tap(&mut runtime, "fruit-trigger");
    tap(&mut runtime, "fruit-1");
    assert_eq!(runtime.state.app.chosen, Some(1));
    let trigger = node(&runtime.accessibility_json(), "fruit-trigger").unwrap();
    assert_eq!(trigger["value"], "Banana", "{trigger}");

    tap(&mut runtime, "fruit-trigger");
    let tree = runtime.accessibility_json();
    assert!(has_state(&node(&tree, "fruit-1").unwrap(), "selected"));
    assert!(!has_state(&node(&tree, "fruit-0").unwrap(), "selected"));
}

#[test]
fn the_controls_describe_themselves_for_the_checklist() {
    use odk_view::{Accessible, Describe, Role};
    let button = Button::new("b", "Save", |_: &mut App| {})
        .disabled(true)
        .busy(true);
    assert_eq!(
        button.accessible(),
        Accessible::new(Role::Button, "Save")
            .disabled(true)
            .busy(true)
    );
    let input = Input::new("i", "Name", |_: &mut App, _: String| {}, |_: &mut App| {});
    assert_eq!(input.accessible(), Accessible::new(Role::TextInput, "Name"));
    let select = Select::new("s", "Fruit", fruits(), Some(1), |_: &mut App, _: usize| {});
    assert_eq!(
        select.accessible(),
        Accessible::new(Role::Combobox, "Fruit").value("Banana")
    );
}
