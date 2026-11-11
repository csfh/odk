//! `InputGroup` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_theme::component::ButtonVariant;
use odk_view::{Button, Face, Hosted, Icon, Input, InputGroup, frame, headless};
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
    acted: u32,
    masked: bool,
    long: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let prefix = if hosted.app.long {
        "https://a-very-long-prefix-that-cannot-fit.example.org/"
    } else {
        "https://"
    };
    let group = InputGroup::<App, _, _>::new(
        "g",
        "Address",
        |_: &mut App, _: String| {},
        |_: &mut App| {},
    )
    .prefix(prefix)
    .suffix(".com")
    .masked(hosted.app.masked)
    .action(
        Button::icon("act", Icon::Search, "Look up", |a: &mut App| a.acted += 1)
            .variant(ButtonVariant::Ghost),
    )
    .show(hosted);
    let plain = Input::new(
        "plain",
        "Plain",
        |_: &mut App, _: String| {},
        |_: &mut App| {},
    )
    .show(hosted);
    let body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(group)
        .child(plain)
        .child(Button::new("elsewhere", "Elsewhere", |_: &mut App| {}).show(hosted));
    frame(hosted, cx, body, Vec::new())
}

fn start(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (420.0, 280.0),
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

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

#[test]
fn it_is_as_high_as_a_plain_field_and_an_action_does_not_change_that() {
    let runtime = start(App::default());
    assert_eq!(rect(&runtime, "g-box")[3], rect(&runtime, "plain-box")[3]);
}

#[test]
fn the_addons_are_decoration_a_screen_reader_does_not_read() {
    let runtime = start(App::default());
    let tree = runtime.accessibility_json().to_string();
    assert!(!tree.contains("https://"), "{tree}");
    assert!(!tree.contains(".com"), "{tree}");
    assert!(tree.contains("Address"), "the field is named");
}

#[test]
fn a_press_on_an_addon_focuses_the_field() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "elsewhere");
    assert_ne!(focused(&runtime).as_deref(), Some("g"));
    tap(&mut runtime, "g-prefix");
    assert_eq!(focused(&runtime).as_deref(), Some("g"));
    tap(&mut runtime, "elsewhere");
    tap(&mut runtime, "g-suffix");
    assert_eq!(focused(&runtime).as_deref(), Some("g"));
}

#[test]
fn an_action_runs_on_a_press_and_is_reached_by_tab_and_run_by_enter() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "act");
    assert_eq!(runtime.state.app.acted, 1);
    // A pointer press on a tab stop takes focus, as Rune does it; the keyboard path is the one
    // that matters: Tab from the field reaches the action and Enter runs it.
    tap(&mut runtime, "g");
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("act"));
    press(&mut runtime, Key::Enter);
    assert_eq!(runtime.state.app.acted, 2);
}

#[test]
fn a_long_addon_takes_no_more_than_a_quarter_of_the_box() {
    let runtime = start(App {
        long: true,
        ..App::default()
    });
    let prefix = rect(&runtime, "g-prefix");
    assert!(prefix[2] <= 240.0 * 0.25 + 12.0, "{prefix:?}");
}

#[test]
fn a_masked_group_writes_no_primary_selection() {
    let mut runtime = start(App {
        masked: true,
        ..App::default()
    });
    tap(&mut runtime, "g");
    for c in "secret".chars() {
        press(&mut runtime, Key::Text(c.to_string()));
    }
    press(&mut runtime, Key::Command('a'));
    assert_eq!(runtime.state.ui.primary(), "");

    let mut runtime = start(App::default());
    tap(&mut runtime, "g");
    for c in "open".chars() {
        press(&mut runtime, Key::Text(c.to_string()));
    }
    press(&mut runtime, Key::Command('a'));
    assert_eq!(runtime.state.ui.primary(), "open");
}
