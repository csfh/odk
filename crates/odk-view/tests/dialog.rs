//! `Dialog` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, Dialog, Face, Hosted, Input, Text, frame, headless};
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

struct App {
    open: bool,
    /// How many times the dialog asked to close, and how many times the page behind was pressed.
    closes: u32,
    behind: u32,
    escape: bool,
    backdrop: bool,
    button: bool,
    tall: bool,
    returns: bool,
    name: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            open: false,
            closes: 0,
            behind: 0,
            escape: true,
            backdrop: true,
            button: true,
            tall: false,
            returns: false,
            name: String::new(),
        }
    }
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut layers = Vec::new();
    let mut body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(Button::new("open", "Open", |app: &mut App| app.behind += 1).show(hosted));
    body = body.child(Button::new("other", "Other", |app: &mut App| app.behind += 1).show(hosted));
    let mut dialog = Dialog::<App>::new("dlg", "Delete the project?")
        .open(app.open)
        .description("This cannot be undone.")
        .close_on_escape(app.escape)
        .close_on_backdrop(app.backdrop)
        .close_button(app.button)
        .on_close(|app: &mut App| {
            app.closes += 1;
            app.open = false;
        })
        .content(
            Input::new(
                "name",
                "Project name",
                |app: &mut App, text| app.name = text,
                |_: &mut App| {},
            )
            .show(hosted),
        )
        .action(Button::new("cancel", "Cancel", |app: &mut App| app.open = false).show(hosted))
        .action(Button::new("confirm", "Delete", |app: &mut App| app.open = false).show(hosted));
    if app.tall {
        for i in 0..40 {
            dialog = dialog.content(
                Text::new(format!("Line {i} of a body that is far too tall")).show(hosted),
            );
        }
    }
    if app.returns {
        dialog = dialog.returns_focus_to("open");
    }
    dialog.show(hosted, &mut layers);
    frame(hosted, cx, body, layers)
}

fn open_in(app: App, size: (f32, f32)) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        size,
    );
    settle(&mut runtime);
    runtime
}

fn open_dialog(app: App) -> Rt {
    open_in(App { open: true, ..app }, (480.0, 360.0))
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

fn tap(runtime: &mut Rt, x: f32, y: f32) {
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

#[test]
fn a_closed_dialog_draws_nothing_and_an_open_one_is_centred_and_no_wider_than_its_measure() {
    let closed = open_in(App::default(), (480.0, 360.0));
    assert!(!has(&closed, "dlg"));
    let runtime = open_dialog(App::default());
    let panel = rect(&runtime, "dlg");
    assert!(panel[2] <= 480.0 + 0.5, "{panel:?}");
    let (cx, cy) = (panel[0] + panel[2] / 2.0, panel[1] + panel[3] / 2.0);
    assert!(
        (cx - 240.0).abs() < 2.0 && (cy - 180.0).abs() < 2.0,
        "centred: {panel:?}"
    );
}

#[test]
fn it_reports_a_dialog_with_its_title_and_description() {
    let runtime = open_dialog(App::default());
    let node = nodes(&runtime)
        .into_iter()
        .find(|n| n["key"] == "dlg")
        .expect("the dialog in the tree");
    assert_eq!(node["role"], "dialog", "{node}");
    assert_eq!(node["name"], "Delete the project?");
    assert_eq!(node["description"], "This cannot be undone.");
}

#[test]
fn escape_the_close_button_and_the_scrim_each_ask_to_close_once() {
    let mut runtime = open_dialog(App::default());
    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.app.closes, 1, "Escape");
    assert!(!has(&runtime, "dlg"));

    let mut runtime = open_dialog(App::default());
    let (x, y) = runtime.find("dlg-close").expect("the close button");
    tap(&mut runtime, x, y);
    assert_eq!(runtime.state.app.closes, 1, "the close button");

    let mut runtime = open_dialog(App::default());
    tap(&mut runtime, 8.0, 8.0);
    assert_eq!(runtime.state.app.closes, 1, "the scrim");
}

#[test]
fn a_press_inside_the_panel_does_not_close_it() {
    let mut runtime = open_dialog(App::default());
    let panel = rect(&runtime, "dlg");
    tap(&mut runtime, panel[0] + 6.0, panel[1] + panel[3] - 6.0);
    assert_eq!(runtime.state.app.closes, 0);
    assert!(has(&runtime, "dlg"));
}

#[test]
fn each_way_out_can_be_turned_off_and_not_all_three() {
    let mut runtime = open_dialog(App {
        backdrop: false,
        ..App::default()
    });
    tap(&mut runtime, 8.0, 8.0);
    assert_eq!(runtime.state.app.closes, 0, "the scrim is off");

    let mut runtime = open_dialog(App {
        escape: false,
        ..App::default()
    });
    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.app.closes, 0, "Escape is off");

    let runtime = open_dialog(App {
        button: false,
        ..App::default()
    });
    assert!(!has(&runtime, "dlg-close"), "no close button");

    let mut runtime = open_dialog(App {
        escape: false,
        backdrop: false,
        button: false,
        ..App::default()
    });
    press(&mut runtime, Key::Escape);
    assert_eq!(
        runtime.state.app.closes, 1,
        "with every way out off, Escape stays: a dialog with no way out is a keyboard trap"
    );
}

#[test]
fn tab_stays_inside_and_the_page_behind_takes_nothing() {
    let mut runtime = open_dialog(App::default());
    let mut seen = Vec::new();
    for _ in 0..8 {
        press(&mut runtime, Key::Tab);
        seen.push(focused(&runtime).unwrap_or_default());
    }
    assert!(
        seen.iter().all(|k| k != "open" && k != "other"),
        "never the page behind: {seen:?}"
    );
    assert!(
        seen.iter().any(|k| k == "cancel") && seen.iter().any(|k| k == "confirm"),
        "{seen:?}"
    );
    let (x, y) = runtime.find("other").expect("the page's button");
    tap(&mut runtime, x, y);
    assert_eq!(runtime.state.app.behind, 0, "the page behind is inert");
}

#[test]
fn the_first_tab_reaches_the_first_control() {
    let mut runtime = open_dialog(App::default());
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("dlg-close"));
}

#[test]
fn focus_goes_back_to_what_the_application_names() {
    let mut runtime = open_dialog(App {
        returns: true,
        ..App::default()
    });
    press(&mut runtime, Key::Escape);
    assert!(!has(&runtime, "dlg"));
    assert_eq!(focused(&runtime).as_deref(), Some("open"));
}

#[test]
fn a_body_too_tall_scrolls_and_the_dialog_stays_inside_its_window() {
    let runtime = open_in(
        App {
            open: true,
            tall: true,
            ..App::default()
        },
        (400.0, 300.0),
    );
    let panel = rect(&runtime, "dlg");
    assert!(
        panel[1] >= 0.0 && panel[1] + panel[3] <= 300.5,
        "never taller than the window: {panel:?}"
    );
    let confirm = rect(&runtime, "confirm");
    assert!(
        confirm[1] + confirm[3] <= 300.5,
        "the actions stay in reach: {confirm:?}"
    );
}
