//! `DropdownMenu` and `Menu` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, DropdownMenu, Face, Hosted, Icon, Menu, MenuItem, frame, headless};
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
    open: bool,
    /// The labels of the items that were chosen, in order.
    chosen: Vec<&'static str>,
    behind: u32,
}

fn choose(label: &'static str) -> impl Fn(&mut App) + 'static {
    move |app: &mut App| app.chosen.push(label)
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    // Entries: 0 heading, 1 New, 2 Open, 3 separator, 4 Autosave, 5 Small, 6 Large, 7 Disabled, 8 Delete.
    let menu = Menu::<App>::new()
        .heading("File")
        .item(
            MenuItem::new("New")
                .hint("Ctrl+N")
                .icon(Icon::Check)
                .on_select(choose("New")),
        )
        .item(MenuItem::new("Open").on_select(choose("Open")))
        .separator()
        .item(
            MenuItem::new("Autosave")
                .checked(true)
                .on_select(choose("Autosave")),
        )
        .item(
            MenuItem::new("Small")
                .radio(true)
                .on_select(choose("Small")),
        )
        .item(
            MenuItem::new("Large")
                .radio(false)
                .on_select(choose("Large")),
        )
        .item(
            MenuItem::new("Disabled item")
                .disabled(true)
                .on_select(choose("Disabled item")),
        )
        .item(MenuItem::new("Delete").danger().on_select(choose("Delete")));
    let trigger = DropdownMenu::<App>::new("menu", "Actions")
        .menu(menu)
        .open(hosted.app.open)
        .on_open_change(|app: &mut App, open| app.open = open)
        .show(hosted, &mut layers);
    let body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(trigger)
        .child(Button::new("behind", "Behind", |app: &mut App| app.behind += 1).show(hosted));
    frame(hosted, cx, body, layers)
}

fn open() -> Rt {
    let mut runtime = headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        page,
        (480.0, 360.0),
    );
    settle(&mut runtime);
    runtime
}

/// A window with the menu already open.
fn opened() -> Rt {
    let mut runtime = open();
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
fn a_closed_menu_draws_nothing_and_a_press_on_the_button_opens_it_below_the_button() {
    let mut runtime = open();
    assert!(!has(&runtime, "menu-menu"));
    let (x, y) = runtime.find("menu").expect("the button");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert!(runtime.state.app.open, "the button asked to open");
    let (button, panel) = (rect(&runtime, "menu"), rect(&runtime, "menu-menu"));
    assert!(
        panel[1] >= button[1] + button[3] - 0.5,
        "below the button: {button:?} {panel:?}"
    );
}

#[test]
fn down_on_the_button_opens_it() {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("menu"));
    press(&mut runtime, Key::Down);
    assert!(runtime.state.app.open);
}

#[test]
fn it_opens_with_focus_on_the_first_enabled_item_and_the_arrow_keys_skip_the_rest_and_wrap() {
    let mut runtime = opened();
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("menu-item-1"),
        "the heading is not an item: focus starts on New"
    );
    let mut path = Vec::new();
    for _ in 0..7 {
        press(&mut runtime, Key::Down);
        path.push(focused(&runtime).unwrap_or_default());
    }
    // Open, past the separator to Autosave, Small, Large, past the disabled item to Delete, and
    // round to New and Open.
    assert_eq!(
        path,
        [
            "menu-item-2",
            "menu-item-4",
            "menu-item-5",
            "menu-item-6",
            "menu-item-8",
            "menu-item-1",
            "menu-item-2",
        ],
        "headings, separators and disabled items are never reached; the end wraps"
    );
    press(&mut runtime, Key::Up);
    assert_eq!(focused(&runtime).as_deref(), Some("menu-item-1"));
    press(&mut runtime, Key::Up);
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("menu-item-8"),
        "Up from the first wraps to the last"
    );
}

#[test]
fn home_and_end_jump_to_the_first_and_the_last_enabled_item() {
    let mut runtime = opened();
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::End);
    assert_eq!(focused(&runtime).as_deref(), Some("menu-item-8"));
    press(&mut runtime, Key::Home);
    assert_eq!(focused(&runtime).as_deref(), Some("menu-item-1"));
}

#[test]
fn a_letter_jumps_to_the_next_item_that_starts_with_it_and_never_to_a_disabled_one() {
    let mut runtime = opened();
    type_text(&mut runtime, "d");
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("menu-item-8"),
        "Delete, not the disabled item that also starts with d"
    );
    // A pause of a second starts afresh: without it "do" would match nothing.
    for _ in 0..90 {
        runtime.frame(1.0 / 60.0);
    }
    type_text(&mut runtime, "o");
    assert_eq!(focused(&runtime).as_deref(), Some("menu-item-2"), "Open");
}

#[test]
fn enter_or_space_or_a_press_chooses_an_item_and_closes_the_menu() {
    let mut runtime = opened();
    press(&mut runtime, Key::Enter);
    assert_eq!(
        runtime.state.app.chosen,
        ["New"],
        "Enter, on the item focus starts on"
    );
    assert!(!runtime.state.app.open, "the menu asked to close");

    let mut runtime = opened();
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Text(" ".into()));
    assert_eq!(runtime.state.app.chosen, ["Open"], "Space");

    let mut runtime = opened();
    let (x, y) = runtime.find("menu-item-8").expect("Delete");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.chosen, ["Delete"], "a press");
    assert!(!runtime.state.app.open);
}

#[test]
fn a_disabled_item_does_nothing_and_leaves_the_menu_open() {
    let mut runtime = opened();
    let (x, y) = runtime.find("menu-item-7").expect("the disabled item");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert!(runtime.state.app.chosen.is_empty());
    assert!(runtime.state.app.open);
}

#[test]
fn escape_or_a_press_outside_closes_it_and_focus_goes_back_to_the_button() {
    let mut runtime = opened();
    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Escape);
    assert!(!runtime.state.app.open, "Escape");
    assert_eq!(focused(&runtime).as_deref(), Some("menu"));

    let mut runtime = opened();
    runtime.pointer_down(470.0, 350.0);
    runtime.pointer_up(470.0, 350.0);
    settle(&mut runtime);
    assert!(!runtime.state.app.open, "a press outside");
    assert_eq!(
        runtime.state.app.behind, 0,
        "and it presses nothing beneath"
    );
}

#[test]
fn while_it_is_open_the_page_behind_takes_nothing() {
    let mut runtime = opened();
    let (x, y) = runtime.find("behind").expect("the page's button");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.behind, 0);
}

#[test]
fn items_report_their_roles_and_states_and_the_menu_is_named_for_its_button() {
    let runtime = opened();
    let all = nodes(&runtime);
    let item = |key: &str| {
        all.iter()
            .find(|n| n["key"] == key)
            .unwrap_or_else(|| panic!("no {key}"))
    };
    let states = |n: &serde_json::Value| -> Vec<String> {
        n["states"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    };
    assert_eq!(item("menu-menu")["role"], "menu");
    assert_eq!(item("menu-menu")["name"], "Actions");
    assert_eq!(item("menu-item-1")["role"], "menu-item");
    assert_eq!(item("menu-item-4")["role"], "menu-item-check-box");
    assert!(
        states(item("menu-item-4")).contains(&"checked".into()),
        "{}",
        item("menu-item-4")
    );
    assert_eq!(item("menu-item-5")["role"], "menu-item-radio");
    assert!(states(item("menu-item-5")).contains(&"checked".into()));
    assert!(!states(item("menu-item-6")).contains(&"checked".into()));
}

#[test]
fn the_menu_lists_what_each_item_reports_for_the_checklist() {
    let menu = Menu::<App>::new()
        .heading("Group")
        .item(MenuItem::new("Plain"))
        .separator()
        .item(MenuItem::new("On").checked(true))
        .item(MenuItem::new("Chosen").radio(true))
        .item(MenuItem::new("Off").disabled(true));
    let accessibles = menu.accessibles();
    assert_eq!(
        accessibles.len(),
        4,
        "a heading and a separator are not items"
    );
    assert!(accessibles[1].selected == Some(true) && accessibles[2].selected == Some(true));
    assert!(accessibles[3].disabled);
}
