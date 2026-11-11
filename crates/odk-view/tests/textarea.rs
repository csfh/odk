//! `Textarea` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, Face, Hosted, Input, Textarea, frame, headless};
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
    text: String,
    seeded: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut area = Textarea::new("t", "Notes", |a: &mut App, text: String| a.text = text)
        .rows(2)
        .max_rows(5)
        .width(160.0);
    if hosted.app.seeded {
        area = area.default_value("a\nb");
    }
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
        .child(area.show(hosted))
        .child(plain)
        .child(Button::new("next", "Next", |_: &mut App| {}).show(hosted));
    frame(hosted, cx, body, Vec::new())
}

fn start(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (420.0, 480.0),
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

fn type_text(runtime: &mut Rt, text: &str) {
    for c in text.chars() {
        press(runtime, Key::Text(c.to_string()));
    }
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

fn height(runtime: &Rt, key: &str) -> f32 {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect[3]
}

#[test]
fn it_starts_as_many_rows_high_as_asked() {
    let runtime = start(App::default());
    let (area, plain) = (height(&runtime, "t-box"), height(&runtime, "plain-box"));
    assert!(
        area > plain * 1.5 && area < plain * 2.6,
        "{area} against {plain}"
    );
}

#[test]
fn enter_and_shift_enter_end_a_line_and_do_not_leave_the_field() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "t");
    type_text(&mut runtime, "a");
    press(&mut runtime, Key::Enter);
    type_text(&mut runtime, "b");
    let mut shifted = KeyEvent::from(Key::Enter);
    shifted.mods.shift = true;
    runtime.key(shifted.clone());
    runtime.key_up(shifted);
    settle(&mut runtime);
    type_text(&mut runtime, "c");
    assert_eq!(runtime.state.app.text, "a\nb\nc");
    assert_eq!(focused(&runtime).as_deref(), Some("t"));
}

#[test]
fn tab_leaves_the_field_and_inserts_nothing() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "t");
    type_text(&mut runtime, "a");
    press(&mut runtime, Key::Tab);
    assert_ne!(focused(&runtime).as_deref(), Some("t"));
    assert_eq!(runtime.state.app.text, "a");
}

#[test]
fn the_box_grows_with_its_lines_to_max_rows_and_no_further() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "t");
    let mut lines = 1;
    let mut at = |runtime: &mut Rt, upto: usize| {
        while lines < upto {
            type_text(runtime, "x");
            press(runtime, Key::Enter);
            lines += 1;
        }
        // Rune springs a box to its new height; let it arrive.
        for _ in 0..90 {
            runtime.frame(1.0 / 60.0);
        }
        height(runtime, "t-box")
    };
    let two = at(&mut runtime, 2);
    let four = at(&mut runtime, 4);
    let five = at(&mut runtime, 5);
    let eight = at(&mut runtime, 8);
    let twelve = at(&mut runtime, 12);
    assert!(four > two, "{four} grew from {two}");
    assert!(five > four, "up to five rows");
    assert_eq!(twelve, eight, "and no further");
    assert!(
        twelve < two * 3.0,
        "{twelve} is five rows at most, from {two}"
    );
}

#[test]
fn a_long_line_wraps_at_the_box_and_makes_it_taller() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "t");
    let before = height(&runtime, "t-box");
    type_text(&mut runtime, &"word ".repeat(12));
    assert!(height(&runtime, "t-box") > before);
    assert!(
        !runtime.state.app.text.contains('\n'),
        "wrapped, not broken"
    );
}

#[test]
fn a_starting_value_may_have_several_lines() {
    let mut runtime = start(App {
        seeded: true,
        ..App::default()
    });
    tap(&mut runtime, "t");
    type_text(&mut runtime, "!");
    assert!(
        runtime.state.app.text.starts_with("a\nb"),
        "{:?}",
        runtime.state.app.text
    );
}

/// The scrolled offset and the room to scroll of the box's viewport, from the tree.
fn scrolled(runtime: &Rt) -> Option<(f64, f64)> {
    fn walk(n: &serde_json::Value) -> Option<(f64, f64)> {
        if let Some(all) = n["scroll"].as_array() {
            return Some((all[1].as_f64()?, all[2].as_f64()?));
        }
        n["children"].as_array()?.iter().find_map(walk)
    }
    walk(&runtime.accessibility_json())
}

#[test]
fn past_max_rows_the_text_scrolls_inside_the_box_and_keeps_the_caret_in_view() {
    let mut runtime = start(App::default());
    tap(&mut runtime, "t");
    for c in "abcdefghij".chars() {
        type_text(&mut runtime, &c.to_string());
        press(&mut runtime, Key::Enter);
    }
    for _ in 0..120 {
        runtime.frame(1.0 / 60.0);
    }
    let (offset, room) = scrolled(&runtime).expect("a viewport");
    assert!(room > 0.0, "there is more text than the box shows");
    assert!(
        offset > 0.0,
        "the caret is at the end, so the text has scrolled"
    );
}
