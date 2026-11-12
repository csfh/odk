//! `Toast` and `Toaster` on the Rune host. Nothing here opens a window.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::Duration;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, Face, Hosted, Toast, ToastId, Toaster, Tone, frame, headless};
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
    toaster: Toaster<App>,
    acted: u32,
    behind: u32,
    builds: Cell<usize>,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    hosted.app.builds.set(hosted.app.builds.get() + 1);
    let mut layers = Vec::new();
    let body = col()
        .key("page")
        .padding(16.0)
        .child(Button::new("behind", "Behind", |app: &mut App| app.behind += 1).show(hosted));
    hosted.app.toaster.show(hosted, &mut layers);
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

fn settle(runtime: &mut Rt) {
    for _ in 0..6 {
        runtime.frame(1.0 / 60.0);
    }
}

/// Raises a toast the way an application would, and lets the window draw it.
fn raise(runtime: &mut Rt, toast: Toast<App>) {
    runtime.state.app.toaster.raise(toast);
    runtime.invalidate();
    settle(runtime);
}

/// Lets `seconds` of the host's clock pass.
fn pass(runtime: &mut Rt, seconds: f32) {
    let steps = (seconds / 0.1).round() as usize;
    for _ in 0..steps {
        runtime.frame(0.1);
    }
    settle(runtime);
}

fn shown(runtime: &Rt) -> usize {
    runtime
        .elements()
        .iter()
        .filter(|e| {
            !e.exiting
                && e.key
                    .as_deref()
                    .is_some_and(|k| k.starts_with("toast-") && k.matches('-').count() == 1)
        })
        .count()
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

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn a_toast_stacks_in_the_bottom_right_corner_and_never_takes_focus() {
    let mut runtime = open();
    assert_eq!(shown(&runtime), 0);
    raise(
        &mut runtime,
        Toast::new("Saved").description("Your changes are safe."),
    );
    assert_eq!(shown(&runtime), 1);
    let stack = rect(&runtime, "odk-toasts");
    assert!(
        stack[0] + stack[2] < 480.0 && stack[1] + stack[3] < 360.0,
        "{stack:?}"
    );
    assert!(
        stack[0] + stack[2] > 480.0 - 40.0 && stack[1] + stack[3] > 360.0 - 40.0,
        "in the corner: {stack:?}"
    );
    assert_eq!(
        focused(&runtime),
        None,
        "it never takes focus when it appears"
    );
}

#[test]
fn a_toast_goes_after_the_foundations_time_and_one_with_an_action_after_twice_that() {
    let mut runtime = open();
    raise(&mut runtime, Toast::new("Plain"));
    pass(&mut runtime, 4.0);
    assert_eq!(shown(&runtime), 1, "still there before its time");
    pass(&mut runtime, 2.0);
    assert_eq!(shown(&runtime), 0, "gone after it");

    raise(&mut runtime, Toast::new("Undoable").action("Undo", |_| {}));
    pass(&mut runtime, 6.0);
    assert_eq!(shown(&runtime), 1, "an action doubles it");
    pass(&mut runtime, 5.0);
    assert_eq!(shown(&runtime), 0);
}

#[test]
fn a_persistent_toast_stays_and_a_timeout_is_as_long_as_it_says() {
    let mut runtime = open();
    raise(&mut runtime, Toast::new("Stays").persistent());
    pass(&mut runtime, 60.0);
    assert_eq!(shown(&runtime), 1, "persistent");
    runtime.state.app.toaster.dismiss(ToastId::named("nothing"));
    let id = runtime.state.app.toaster.len();
    assert_eq!(id, 1);

    let mut runtime = open();
    raise(&mut runtime, Toast::new("Brief").timeout(secs(2)));
    pass(&mut runtime, 1.0);
    assert_eq!(shown(&runtime), 1);
    pass(&mut runtime, 1.5);
    assert_eq!(shown(&runtime), 0);
}

#[test]
fn at_most_three_show_and_the_rest_wait_their_turn_and_start_their_clock_when_they_do() {
    let mut runtime = open();
    // All five at once, so the clock starts for the three that show at the same moment.
    for i in 0..5 {
        runtime
            .state
            .app
            .toaster
            .raise(Toast::new(format!("Toast {i}")).timeout(secs(2)));
    }
    runtime.invalidate();
    settle(&mut runtime);
    assert_eq!(shown(&runtime), 3);
    assert_eq!(runtime.state.app.toaster.len(), 5);
    pass(&mut runtime, 2.5);
    assert_eq!(
        shown(&runtime),
        2,
        "the two that waited have shown, with their own two seconds"
    );
    assert_eq!(runtime.state.app.toaster.len(), 2);
    pass(&mut runtime, 1.0);
    assert_eq!(
        shown(&runtime),
        2,
        "their clocks started when they came on screen"
    );
    pass(&mut runtime, 1.5);
    assert_eq!(shown(&runtime), 0);
}

#[test]
fn a_toast_raised_again_under_one_id_updates_in_place_and_starts_its_clock_again() {
    let mut runtime = open();
    let id = ToastId::named("upload");
    raise(
        &mut runtime,
        Toast::new("Uploading").with_id(id).timeout(secs(3)),
    );
    pass(&mut runtime, 2.0);
    raise(
        &mut runtime,
        Toast::new("Uploaded").with_id(id).timeout(secs(3)),
    );
    assert_eq!(shown(&runtime), 1, "one toast, not two");
    assert_eq!(runtime.state.app.toaster.len(), 1);
    pass(&mut runtime, 2.0);
    assert_eq!(shown(&runtime), 1, "its clock began again");
    pass(&mut runtime, 1.5);
    assert_eq!(shown(&runtime), 0);
}

#[test]
fn the_clocks_stop_while_the_pointer_is_over_the_stack() {
    let mut runtime = open();
    raise(&mut runtime, Toast::new("Held").timeout(secs(2)));
    let r = rect(&runtime, "toast-0");
    runtime.pointer_move(r[0] + 6.0, r[1] + 6.0);
    pass(&mut runtime, 10.0);
    assert_eq!(shown(&runtime), 1, "paused under the pointer");
    runtime.pointer_move(4.0, 4.0);
    pass(&mut runtime, 3.0);
    assert_eq!(shown(&runtime), 0, "running again once it leaves");
}

#[test]
fn the_close_button_dismisses_and_the_action_runs_and_dismisses() {
    let mut runtime = open();
    raise(&mut runtime, Toast::new("Closable").persistent());
    let (x, y) = runtime.find("toast-0-close").expect("the close button");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(shown(&runtime), 0);

    raise(
        &mut runtime,
        Toast::new("Undoable")
            .persistent()
            .action("Undo", |app: &mut App| app.acted += 1),
    );
    let (x, y) = runtime.find("toast-0-action").expect("the action");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.acted, 1);
    assert_eq!(shown(&runtime), 0, "the action dismisses it");
}

#[test]
fn the_stack_is_a_tab_stop_and_escape_dismisses_the_newest_toast() {
    let mut runtime = open();
    raise(&mut runtime, Toast::new("First").persistent());
    raise(&mut runtime, Toast::new("Second").persistent());
    for _ in 0..2 {
        runtime.key(KeyEvent::from(Key::Tab));
        settle(&mut runtime);
    }
    assert_eq!(focused(&runtime).as_deref(), Some("odk-toasts"));
    runtime.key(KeyEvent::from(Key::Escape));
    settle(&mut runtime);
    assert_eq!(shown(&runtime), 1, "the newest went");
    assert_eq!(runtime.state.app.toaster.len(), 1);
}

#[test]
fn the_page_behind_stays_live_and_a_window_with_no_toast_asks_for_nothing() {
    let mut runtime = open();
    let builds = runtime.state.app.builds.get();
    for _ in 0..60 {
        runtime.frame(1.0 / 60.0);
    }
    assert_eq!(
        runtime.state.app.builds.get(),
        builds,
        "no toast, no rebuild (item 35)"
    );
    raise(&mut runtime, Toast::new("Persistent").persistent());
    let builds = runtime.state.app.builds.get();
    for _ in 0..60 {
        runtime.frame(1.0 / 60.0);
    }
    assert_eq!(
        runtime.state.app.builds.get(),
        builds,
        "a persistent toast has no clock to wake for"
    );
    let (x, y) = runtime.find("behind").expect("the page's button");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(&mut runtime);
    assert_eq!(runtime.state.app.behind, 1);
}

#[test]
fn a_toast_is_a_status_named_by_its_title_and_description() {
    let mut runtime = open();
    raise(
        &mut runtime,
        Toast::new("Saved")
            .description("Your changes are safe.")
            .tone(Tone::Success),
    );
    fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
        into.push(node.clone());
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, into);
        }
    }
    let mut nodes = Vec::new();
    walk(&runtime.accessibility_json(), &mut nodes);
    let toast = nodes
        .into_iter()
        .find(|n| n["key"] == "toast-0")
        .expect("the toast");
    assert_eq!(toast["role"], "status", "{toast}");
    assert_eq!(toast["name"], "Saved: Your changes are safe.");
}
