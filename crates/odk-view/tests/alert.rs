//! `Alert` and `Button`'s icon layouts on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Alert, Button, Face, Hosted, Icon, Tone, frame, headless};
use rune::{Cx, El, Runtime, col, row};

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
    dismissed: u32,
    pressed: u32,
}

type Rt = Runtime<Hosted<App>>;

fn open(view: fn(&Hosted<App>, &mut Cx<Hosted<App>>) -> El, size: (f32, f32)) -> Rt {
    headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        view,
        size,
    )
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
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

fn buttons(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let press = |app: &mut App| app.pressed += 1;
    let line = |button: El| row().child(button);
    frame(
        hosted,
        cx,
        col()
            .child(line(Button::new("text", "Next", press).show(hosted)))
            .child(line(
                Button::new("leading", "Next", press)
                    .leading_icon(Icon::Check)
                    .show(hosted),
            ))
            .child(line(
                Button::new("trailing", "Next", press)
                    .trailing_icon(Icon::ArrowRight)
                    .show(hosted),
            ))
            .child(line(
                Button::icon("only", Icon::Settings, "Settings", press).show(hosted),
            ))
            .child(line(Button::icon("char", "★", "Star", press).show(hosted))),
        [],
    )
}

#[test]
fn an_icon_takes_room_beside_the_label_and_an_icon_alone_is_a_square() {
    let runtime = open(buttons, (400.0, 400.0));
    let width = |key: &str| rect(&runtime, key)[2];
    let height = |key: &str| rect(&runtime, key)[3];
    assert!(
        width("leading") > width("text"),
        "a leading icon takes room"
    );
    assert!(
        width("trailing") > width("text"),
        "a trailing icon takes room"
    );
    assert_eq!(width("leading"), width("trailing"));
    assert_eq!(height("text"), height("only"), "as high as any button");
    assert_eq!(width("only"), height("only"), "an icon alone is a square");
    assert_eq!(width("char"), height("char"), "so is a character");
}

#[test]
fn an_icon_only_button_is_named_by_its_name_and_presses() {
    let mut runtime = open(buttons, (400.0, 400.0));
    let all = nodes(&runtime);
    for name in ["Settings", "Star", "Next"] {
        assert!(
            all.iter()
                .any(|node| node["role"] == "button" && node["name"] == name),
            "no button named {name}: {all:#?}"
        );
    }
    tap(&mut runtime, "only");
    assert_eq!(runtime.state.app.pressed, 1);
}

fn alerts(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .w(500.0)
            .child(
                Alert::new("plain", "Heads up")
                    .description("Something needs your attention.")
                    .on_dismiss(|app: &mut App| app.dismissed += 1)
                    .show(hosted)
                    .key("plain-alert"),
            )
            .child(
                Alert::new("tight", "Heads up")
                    .description("Something needs your attention.")
                    .compact(true)
                    .show(hosted)
                    .key("tight-alert"),
            )
            .child(
                Alert::new(
                    "long",
                    "A title far longer than the room the alert gives it",
                )
                .tone(Tone::Danger)
                .description("Unbreakable_word_with_no_place_to_wrap_at_all_inside_the_alert")
                .action(Button::new("act", "Retry", |_: &mut App| {}).show(hosted))
                .on_dismiss(|app: &mut App| app.dismissed += 1)
                .show(hosted)
                .key("long-alert"),
            ),
        [],
    )
}

#[test]
fn an_alert_describes_itself_and_keeps_its_dismiss_button() {
    let runtime = open(alerts, (600.0, 600.0));
    let all = nodes(&runtime);
    let alert = all
        .iter()
        .find(|node| {
            node["role"] == "alert" && node["name"] == "Heads up: Something needs your attention."
        })
        .unwrap_or_else(|| panic!("no alert: {all:#?}"));
    assert!(alert["name"].is_string());
    assert!(
        all.iter()
            .any(|node| node["role"] == "button" && node["name"] == "Dismiss"),
        "a dismiss button named Dismiss"
    );
    // Its words are the alert's name, not nodes of their own.
    assert!(all.iter().all(|node| node["name"] != "Heads up"));
}

#[test]
fn dismissing_calls_the_handler_and_the_alert_is_not_a_tab_stop() {
    let mut runtime = open(alerts, (600.0, 600.0));
    let focusable: Vec<_> = runtime
        .elements()
        .into_iter()
        .filter(|e| e.focusable)
        .filter_map(|e| e.key)
        .collect();
    assert!(
        !focusable.iter().any(|key| key.ends_with("-alert")),
        "an alert takes no focus: {focusable:?}"
    );
    assert!(
        focusable.contains(&"plain-dismiss".to_string()),
        "{focusable:?}"
    );
    tap(&mut runtime, "plain-dismiss");
    assert_eq!(runtime.state.app.dismissed, 1);
}

#[test]
fn the_mark_leads_the_words_and_the_dismiss_ends_the_alert_on_the_title_line() {
    let runtime = open(alerts, (600.0, 600.0));
    let alert = rect(&runtime, "plain-alert");
    let dismiss = rect(&runtime, "plain-dismiss");
    let all = runtime.elements();
    let title = all
        .iter()
        .find(|e| e.text.as_deref() == Some("Heads up"))
        .expect("the title")
        .rect;
    assert!(
        title[0] > alert[0],
        "the mark and padding come before the title"
    );
    assert!(
        dismiss[0] + dismiss[2] < alert[0] + alert[2],
        "padding after the dismiss"
    );
    assert!(
        dismiss[0] > title[0] + title[2],
        "the dismiss is after the words"
    );
    // The dismiss button's centre is on the title's line, though it is taller than it.
    let (dismiss_centre, title_centre) = (dismiss[1] + dismiss[3] / 2.0, title[1] + title[3] / 2.0);
    assert!(
        (dismiss_centre - title_centre).abs() <= 1.0,
        "dismiss {dismiss_centre}, title {title_centre}"
    );
}

#[test]
fn a_compact_alert_is_tighter() {
    let runtime = open(alerts, (600.0, 600.0));
    assert!(rect(&runtime, "tight-alert")[3] < rect(&runtime, "plain-alert")[3]);
}

#[test]
fn a_long_title_and_an_unbreakable_word_wrap_inside_the_alert_and_the_buttons_keep_their_place() {
    let runtime = open(alerts, (600.0, 600.0));
    let alert = rect(&runtime, "long-alert");
    let (act, dismiss) = (rect(&runtime, "act"), rect(&runtime, "long-dismiss"));
    for text in runtime.elements().into_iter().filter(|e| {
        e.text
            .as_deref()
            .is_some_and(|t| t.starts_with("A title") || t.starts_with("Unbreakable"))
    }) {
        assert!(
            text.rect[0] + text.rect[2] <= act[0],
            "{:?} runs under the action: {:?} {act:?}",
            text.text,
            text.rect
        );
    }
    assert!(
        act[0] + act[2] <= dismiss[0],
        "the action sits before the dismiss"
    );
    assert!(
        dismiss[0] + dismiss[2] <= alert[0] + alert[2],
        "inside the alert"
    );
}
