//! `Checkbox` and `Switch` on the Rune host. Nothing here opens a window.

use std::sync::{Arc, OnceLock};

use odk_theme::ThemeSnapshot;
use odk_theme::component::{ComponentTokens, States};
use odk_theme::foundation::{FoundationConfig, FoundationTokens};
use odk_view::{Checkbox, Face, Hosted, Switch, frame, headless};
use rune::{Cx, El, Key, KeyEvent, Runtime, col};

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

/// The component layer as the host builds it, so a test can say what a token says.
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
    agree: bool,
    mixed: bool,
    wifi: bool,
    /// Every state a controlled toggle asked for, in order.
    asked: Vec<bool>,
    /// The same for the toggles that keep their own state.
    own: Vec<bool>,
}

type Rt = Runtime<Hosted<App>>;

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    frame(
        hosted,
        cx,
        col()
            .w(400.0)
            .gap(8.0)
            .child(
                Checkbox::new("agree", "I agree")
                    .checked(app.agree)
                    .on_change(|app: &mut App, on| {
                        app.agree = on;
                        app.asked.push(on);
                    })
                    .show(hosted),
            )
            .child(
                Checkbox::new("mixed", "Some of them")
                    .checked(app.mixed)
                    .indeterminate(!app.mixed)
                    .on_change(|app: &mut App, on| {
                        app.mixed = on;
                        app.asked.push(on);
                    })
                    .show(hosted),
            )
            .child(
                Checkbox::new("locked", "Locked")
                    .disabled(true)
                    .invalid(true)
                    .on_change(|app: &mut App, on| app.asked.push(on))
                    .show(hosted),
            )
            .child(
                Switch::new("wifi", "Wi-Fi")
                    .checked(app.wifi)
                    .on_change(|app: &mut App, on| {
                        app.wifi = on;
                        app.asked.push(on);
                    })
                    .show(hosted),
            )
            .child(
                // These two keep their own state.
                Checkbox::new("own", "Keeps its own state")
                    .default_checked(true)
                    .on_change(|app: &mut App, on| app.own.push(on))
                    .show(hosted),
            )
            .child(
                Switch::new("own-switch", "Keeps its own state")
                    .on_change(|app: &mut App, on| app.own.push(on))
                    .show(hosted),
            )
            .child(
                col().w(160.0).child(
                    Checkbox::new(
                        "long",
                        "A label far longer than the room the row gives it, in 日本語",
                    )
                    .show(hosted),
                ),
            ),
        [],
    )
}

fn open() -> Rt {
    open_with(ThemeSnapshot::emergency())
}

fn open_with(theme: ThemeSnapshot) -> Rt {
    headless(
        Hosted::new(App::default(), theme, face()),
        page,
        (500.0, 700.0),
    )
}

fn settle(runtime: &mut Rt) {
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
}

fn tab(runtime: &mut Rt) {
    runtime.key(KeyEvent::from(Key::Tab));
    settle(runtime);
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

fn tap_at(runtime: &mut Rt, x: f32, y: f32) {
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

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

#[test]
fn each_one_is_a_checkbox_or_a_switch_with_its_state_in_the_tree() {
    let mut runtime = open();
    let all = nodes(&runtime);
    let find = |role: &str, name: &str| {
        all.iter()
            .find(|n| n["role"] == role && n["name"] == name)
            .unwrap_or_else(|| panic!("no {role} named {name}: {all:#?}"))
            .clone()
    };
    let states = |node: serde_json::Value| -> Vec<String> {
        node["states"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    };
    assert!(states(find("check-box", "I agree")).contains(&"unchecked".to_string()));
    assert!(states(find("check-box", "Some of them")).contains(&"mixed".to_string()));
    let locked = states(find("check-box", "Locked"));
    assert!(locked.contains(&"disabled".to_string()) && locked.contains(&"invalid".to_string()));
    find("switch", "Wi-Fi");
    // After a press, the application's new state is what the tree says.
    let (x, y) = runtime.find("wifi").unwrap();
    tap_at(&mut runtime, x, y);
    assert!(runtime.state.app.wifi);
    let all = nodes(&runtime);
    let wifi = all
        .iter()
        .find(|n| n["role"] == "switch" && n["name"] == "Wi-Fi")
        .expect("the switch");
    assert!(
        !wifi["states"]
            .as_array()
            .is_some_and(|s| s.iter().any(|v| v == "unchecked")),
        "{wifi:#?}"
    );
}

#[test]
fn a_press_on_the_box_or_on_the_label_toggles_once_with_the_state_it_would_make() {
    let mut runtime = open();
    let row = rect(&runtime, "agree");
    // The box is at the row's start and the label after it.
    tap_at(&mut runtime, row[0] + 4.0, row[1] + row[3] / 2.0);
    assert_eq!(runtime.state.app.asked, [true]);
    let row = rect(&runtime, "agree");
    tap_at(&mut runtime, row[0] + row[2] - 4.0, row[1] + row[3] / 2.0);
    assert_eq!(
        runtime.state.app.asked,
        [true, false],
        "the label toggles it too"
    );
    // A mixed box becomes checked.
    let (x, y) = runtime.find("mixed").unwrap();
    tap_at(&mut runtime, x, y);
    assert_eq!(runtime.state.app.asked.last(), Some(&true));
}

#[test]
fn tab_reaches_a_row_and_space_toggles_on_its_release_and_enter_does_not() {
    let mut runtime = open();
    tab(&mut runtime);
    assert_eq!(focused(&runtime).as_deref(), Some("agree"));
    runtime.key(KeyEvent::from(Key::Enter));
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert!(runtime.state.app.asked.is_empty(), "Enter does not toggle");
    let space = || KeyEvent::from(Key::Text(" ".into()));
    runtime.key(space());
    runtime.repeat_key(space());
    runtime.key(space());
    settle(&mut runtime);
    assert!(
        runtime.state.app.asked.is_empty(),
        "nothing while it is held"
    );
    runtime.key_up(space());
    settle(&mut runtime);
    assert_eq!(runtime.state.app.asked, [true], "once, on the release");
}

#[test]
fn a_disabled_toggle_leaves_the_tab_order_and_does_nothing() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..6 {
        tab(&mut runtime);
        visited.extend(focused(&runtime));
    }
    assert!(!visited.contains(&"locked".to_string()), "{visited:?}");
    assert!(visited.contains(&"wifi".to_string()), "{visited:?}");
    let (x, y) = runtime.find("locked").unwrap();
    tap_at(&mut runtime, x, y);
    assert!(runtime.state.app.asked.is_empty());
    let all = nodes(&runtime);
    assert!(
        all.iter().any(|n| n["name"] == "Locked"
            && n["states"]
                .as_array()
                .is_some_and(|s| s.iter().any(|v| v == "disabled"))),
        "{all:#?}"
    );
}

#[test]
fn a_row_is_at_least_the_minimum_target_and_a_long_label_wraps_beside_the_box() {
    let runtime = open();
    let target = tokens().foundation().geometry.target_min;
    assert!(rect(&runtime, "agree")[3] >= target);
    // A short label stays on one line, beside its box: a row sized to its content is as wide as
    // its label, not as narrow as the label's narrowest word.
    for key in ["agree", "wifi", "mixed"] {
        let row = rect(&runtime, key);
        assert!(row[3] < 2.0 * target, "{key} is {} points high", row[3]);
        assert!(row[2] > 60.0, "{key} is {} points wide", row[2]);
        assert!(
            row[2] < 200.0,
            "{key} is as wide as its content, not stretched across its column: {} points",
            row[2]
        );
    }
    assert!(rect(&runtime, "wifi")[3] >= target);
    let long = rect(&runtime, "long");
    assert!(long[2] <= 160.0, "the row overflows its column: {long:?}");
    assert!(long[3] > target, "the label wrapped: {long:?}");
}

#[test]
fn the_box_is_a_square_of_its_token_and_the_thumb_slides_between_its_two_places() {
    let t = tokens();
    let density = t.density(None);
    let boxed = t.checkbox(density, States::NONE).geometry;
    let switch = t.switch(density, States::NONE).geometry;
    let mut runtime = open();
    let all = runtime.elements();
    assert!(
        all.iter()
            .any(|e| e.rect[2] == boxed.size && e.rect[3] == boxed.size),
        "a box of {} points",
        boxed.size
    );
    let track = rect(&runtime, "wifi");
    let thumb_x = |runtime: &Rt| {
        runtime
            .elements()
            .into_iter()
            .find(|e| {
                e.rect[2] == switch.thumb && e.rect[3] == switch.thumb && e.rect[1] > track[1] - 1.0
            })
            .expect("the thumb")
            .rect[0]
    };
    let off = thumb_x(&runtime);
    let (x, y) = runtime.find("wifi").unwrap();
    tap_at(&mut runtime, x, y);
    let on = thumb_x(&runtime);
    assert_eq!(
        on - off,
        switch.travel,
        "the thumb travels {} points",
        switch.travel
    );
    tap_at(&mut runtime, x, y);
    assert_eq!(thumb_x(&runtime), off, "and back");
}

fn states_of(runtime: &Rt, role: &str, name: &str) -> Vec<String> {
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

#[test]
fn a_toggle_keeps_its_own_state_from_its_default_and_tells_the_application() {
    let mut runtime = open();
    let own = |runtime: &Rt, role: &str| states_of(runtime, role, "Keeps its own state");
    assert!(
        own(&runtime, "check-box").contains(&"checked".to_string()),
        "starts checked"
    );
    assert!(
        own(&runtime, "switch").contains(&"unchecked".to_string()),
        "starts off"
    );
    let (x, y) = runtime.find("own").unwrap();
    tap_at(&mut runtime, x, y);
    assert!(
        own(&runtime, "check-box").contains(&"unchecked".to_string()),
        "a press unchecks it"
    );
    tap_at(&mut runtime, x, y);
    assert!(own(&runtime, "check-box").contains(&"checked".to_string()));
    let (x, y) = runtime.find("own-switch").unwrap();
    tap_at(&mut runtime, x, y);
    assert!(own(&runtime, "switch").contains(&"checked".to_string()));
    assert_eq!(
        runtime.state.app.own,
        [false, true, true],
        "each press is heard once"
    );
    assert!(
        runtime.state.app.asked.is_empty(),
        "the controlled ones were not asked"
    );
}

#[test]
fn enter_toggles_a_switch_on_its_release_and_a_checkbox_not_at_all() {
    let mut runtime = open();
    for _ in 0..8 {
        tab(&mut runtime);
        if focused(&runtime).as_deref() == Some("wifi") {
            break;
        }
    }
    assert_eq!(focused(&runtime).as_deref(), Some("wifi"));
    runtime.key(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert!(runtime.state.app.asked.is_empty(), "not on the press");
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime);
    assert_eq!(
        runtime.state.app.asked,
        [true],
        "a switch toggles on Enter's release"
    );
}

/// The thumb's offset from where it rests when off, after `frames` frames.
fn thumb_travelled(runtime: &Rt, track: [f32; 4], thumb: f32, inset: f32) -> f32 {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.rect[2] == thumb && e.rect[3] == thumb && e.rect[1] > track[1] - 1.0)
        .expect("the thumb")
        .rect[0]
        - (track[0] + inset)
}

#[test]
fn the_thumb_lands_at_once_when_the_user_asks_for_reduced_motion() {
    let switch = tokens()
        .switch(tokens().density(None), States::NONE)
        .geometry;
    for (reduced, name) in [(true, "reduced"), (false, "normal")] {
        let mut theme = ThemeSnapshot::emergency();
        theme.reduced_motion = reduced;
        let mut runtime = open_with(theme);
        let track = rect(&runtime, "wifi");
        let (x, y) = runtime.find("wifi").unwrap();
        runtime.pointer_down(x, y);
        runtime.pointer_up(x, y);
        runtime.frame(1.0 / 60.0);
        let travelled = thumb_travelled(&runtime, track, switch.thumb, switch.inset);
        if reduced {
            assert_eq!(
                travelled, switch.travel,
                "{name}: at its place in one frame"
            );
        } else {
            assert!(
                travelled < switch.travel,
                "{name}: the spring has not finished after one frame ({travelled} of {})",
                switch.travel
            );
        }
    }
}
