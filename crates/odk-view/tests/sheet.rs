//! `Sheet` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Button, Face, Hosted, Sheet, Side, Text, frame, headless};
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
    side: Side,
    modal: bool,
    extent: Option<f32>,
    escape: bool,
    backdrop: bool,
    tall: bool,
    closes: u32,
    behind: u32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            open: true,
            side: Side::Right,
            modal: true,
            extent: None,
            escape: true,
            backdrop: true,
            tall: false,
            closes: 0,
            behind: 0,
        }
    }
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut layers = Vec::new();
    let body = col()
        .key("page")
        .padding(16.0)
        .gap(8.0)
        .child(Button::new("open", "Open", |app: &mut App| app.behind += 1).show(hosted))
        .child(Button::new("other", "Other", |app: &mut App| app.behind += 1).show(hosted));
    let mut sheet = Sheet::<App>::new("sh", "Details")
        .open(app.open)
        .side(app.side)
        .modal(app.modal)
        .close_on_escape(app.escape)
        .close_on_backdrop(app.backdrop)
        .on_close(|app: &mut App| {
            app.closes += 1;
            app.open = false;
        })
        .returns_focus_to("open")
        .content(Button::new("inside", "Inside", |_: &mut App| {}).show(hosted));
    if let Some(extent) = app.extent {
        sheet = sheet.extent(extent);
    }
    if app.tall {
        for i in 0..40 {
            sheet = sheet.content(Text::new(format!("Line {i} of a tall body")).show(hosted));
        }
    }
    sheet.show(hosted, &mut layers);
    frame(hosted, cx, body, layers)
}

fn open(app: App) -> Rt {
    let mut runtime = headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (480.0, 360.0),
    );
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

fn tap(runtime: &mut Rt, x: f32, y: f32) {
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

#[test]
fn a_closed_sheet_draws_nothing() {
    let runtime = open(App {
        open: false,
        ..App::default()
    });
    assert!(!has(&runtime, "sh"));
}

#[test]
fn a_sheet_is_attached_to_its_edge_and_fills_it() {
    for modal in [true, false] {
        let right = rect(
            &open(App {
                modal,
                ..App::default()
            }),
            "sh",
        );
        assert!(
            near(right[0] + right[2], 480.0) && near(right[1], 0.0) && near(right[3], 360.0),
            "right, modal {modal}: {right:?}"
        );
        let left = rect(
            &open(App {
                modal,
                side: Side::Left,
                ..App::default()
            }),
            "sh",
        );
        assert!(
            near(left[0], 0.0) && near(left[1], 0.0) && near(left[3], 360.0),
            "left, modal {modal}: {left:?}"
        );
        let bottom = rect(
            &open(App {
                modal,
                side: Side::Bottom,
                ..App::default()
            }),
            "sh",
        );
        assert!(
            near(bottom[0], 0.0) && near(bottom[2], 480.0) && near(bottom[1] + bottom[3], 360.0),
            "bottom, modal {modal}: {bottom:?}"
        );
    }
}

#[test]
fn the_extent_is_the_width_of_a_side_sheet_and_the_height_of_a_bottom_one() {
    let width = |extent| {
        rect(
            &open(App {
                extent: Some(extent),
                ..App::default()
            }),
            "sh",
        )[2]
    };
    let height = |extent| {
        rect(
            &open(App {
                side: Side::Bottom,
                extent: Some(extent),
                ..App::default()
            }),
            "sh",
        )[3]
    };
    assert!(
        near(width(200.0) * 1.5, width(300.0)),
        "{} {}",
        width(200.0),
        width(300.0)
    );
    assert!(
        near(height(100.0) * 2.0, height(200.0)),
        "{} {}",
        height(100.0),
        height(200.0)
    );
}

#[test]
fn a_sheet_is_never_larger_than_its_window() {
    let runtime = open(App {
        extent: Some(5000.0),
        ..App::default()
    });
    let panel = rect(&runtime, "sh");
    assert!(
        panel[0] >= -0.5 && panel[0] + panel[2] <= 480.5,
        "{panel:?}"
    );
}

#[test]
fn a_modal_sheet_closes_from_escape_the_close_button_and_the_scrim_and_not_from_inside() {
    let mut runtime = open(App::default());
    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.app.closes, 1, "Escape");

    let mut runtime = open(App::default());
    let (x, y) = runtime.find("sh-close").expect("the close button");
    tap(&mut runtime, x, y);
    assert_eq!(runtime.state.app.closes, 1, "the close button");

    let mut runtime = open(App::default());
    tap(&mut runtime, 8.0, 8.0);
    assert_eq!(runtime.state.app.closes, 1, "the scrim");

    let mut runtime = open(App::default());
    let panel = rect(&runtime, "sh");
    tap(
        &mut runtime,
        panel[0] + panel[2] - 6.0,
        panel[1] + panel[3] - 6.0,
    );
    assert_eq!(runtime.state.app.closes, 0, "a press inside the panel");
}

#[test]
fn a_modal_sheets_ways_out_can_be_turned_off_one_by_one() {
    let mut runtime = open(App {
        backdrop: false,
        ..App::default()
    });
    tap(&mut runtime, 8.0, 8.0);
    assert_eq!(runtime.state.app.closes, 0, "the scrim is off");
    let mut runtime = open(App {
        escape: false,
        ..App::default()
    });
    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.app.closes, 0, "Escape is off");
}

#[test]
fn a_modal_sheet_keeps_tab_inside_and_blocks_the_page_behind() {
    let mut runtime = open(App::default());
    let mut seen = Vec::new();
    for _ in 0..6 {
        press(&mut runtime, Key::Tab);
        seen.push(focused(&runtime).unwrap_or_default());
    }
    assert!(seen.iter().all(|k| k != "open" && k != "other"), "{seen:?}");
    assert!(seen.iter().any(|k| k == "inside"), "{seen:?}");
    let (x, y) = runtime.find("other").expect("the page's button");
    tap(&mut runtime, x, y);
    assert_eq!(runtime.state.app.behind, 0, "the page behind is inert");
}

#[test]
fn focus_goes_back_to_what_the_application_names_when_a_modal_sheet_closes() {
    let mut runtime = open(App::default());
    press(&mut runtime, Key::Escape);
    assert_eq!(focused(&runtime).as_deref(), Some("open"));
}

#[test]
fn a_side_panel_leaves_the_page_live_and_takes_no_focus() {
    let mut runtime = open(App {
        modal: false,
        ..App::default()
    });
    assert_eq!(focused(&runtime), None, "it takes no focus on opening");
    let (x, y) = runtime.find("other").expect("the page's button");
    tap(&mut runtime, x, y);
    assert_eq!(runtime.state.app.behind, 1, "the page stays live");
    assert_eq!(
        runtime.state.app.closes, 0,
        "and a press elsewhere does not close it"
    );
    assert!(has(&runtime, "sh"));
}

#[test]
fn a_side_panel_closes_on_escape_while_focus_is_inside_it_and_not_otherwise() {
    let mut runtime = open(App {
        modal: false,
        ..App::default()
    });
    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.app.closes, 0, "focus is not inside it");
    let (x, y) = runtime.find("inside").expect("a control inside");
    tap(&mut runtime, x, y);
    assert_eq!(focused(&runtime).as_deref(), Some("inside"));
    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.app.closes, 1);
}

#[test]
fn a_tall_body_scrolls_inside_the_window() {
    let runtime = open(App {
        tall: true,
        ..App::default()
    });
    let panel = rect(&runtime, "sh");
    assert!(
        panel[1] >= -0.5 && panel[1] + panel[3] <= 360.5,
        "{panel:?}"
    );
}

#[test]
fn it_reports_a_dialog_named_by_its_title() {
    let runtime = open(App::default());
    fn walk(node: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
        into.push(node.clone());
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, into);
        }
    }
    let mut nodes = Vec::new();
    walk(&runtime.accessibility_json(), &mut nodes);
    let sheet = nodes
        .into_iter()
        .find(|n| n["key"] == "sh")
        .expect("the sheet");
    assert_eq!(
        (sheet["role"].as_str(), sheet["name"].as_str()),
        (Some("dialog"), Some("Details")),
        "{sheet}"
    );
}
