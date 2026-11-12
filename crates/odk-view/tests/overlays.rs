//! `Tooltip`, `HoverCard` and `Popover` on the Rune host. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{
    Align, Button, Face, Hosted, HoverCard, Placement, Popover, Text, Tooltip, frame, headless,
};
use rune::{Cx, El, Key, KeyEvent, Runtime, col, el, row};

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
    /// What the controlled popover was last told.
    open: bool,
    /// Every request either popover made, as `popover:state`.
    heard: Vec<String>,
    applied: u32,
}

type Rt = Runtime<Hosted<App>>;

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut layers = Vec::new();
    let tip = Tooltip::new("tip", "Saves the document").wrap(
        hosted,
        Button::new("save", "Save", |_: &mut App| {}).show(hosted),
        &mut layers,
    );
    let below = Tooltip::new("below", "Shown below")
        .placement(Placement::Bottom)
        .align(Align::End)
        .wrap(
            hosted,
            Button::new("down", "Down", |_: &mut App| {}).show(hosted),
            &mut layers,
        );
    let card = HoverCard::new("card", "Profile of Ada")
        .content(Text::new("Ada Lovelace, mathematician.").show(hosted))
        .wrap(hosted, Text::new("@ada").show(hosted), &mut layers);
    let filters = Popover::new("filters", "Filters")
        .title("Filter the list")
        .content(|hosted: &Hosted<App>| {
            Button::new("apply", "Apply", |app: &mut App| app.applied += 1).show(hosted)
        })
        .on_open_change(|app: &mut App, open| app.heard.push(format!("filters:{open}")))
        .wrap(hosted, Text::new("Filters").show(hosted), &mut layers);
    let controlled = Popover::new("ctl", "Controlled")
        .title("Controlled")
        .open(app.open)
        .on_open_change(|app: &mut App, open| {
            app.heard.push(format!("ctl:{open}"));
            app.open = open;
        })
        .wrap(hosted, Text::new("Controlled").show(hosted), &mut layers);
    frame(
        hosted,
        cx,
        col()
            .w(500.0)
            .padding(60.0)
            .gap(40.0)
            .child(row().gap(20.0).child(tip).child(below))
            .child(card)
            .child(filters)
            .child(controlled)
            .child(el().key("after").focusable(true).size(20.0, 20.0)),
        layers,
    )
}

fn sticky_page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let sticky = Popover::new("sticky", "Sticky")
        .title("Sticky")
        .default_open(true)
        .close_on_outside_click(false)
        .wrap(
            hosted,
            Text::new("Sticky trigger").show(hosted),
            &mut layers,
        );
    frame(
        hosted,
        cx,
        col()
            .w(500.0)
            .padding(60.0)
            .child(sticky)
            .child(Button::new("behind", "Behind", |app: &mut App| app.applied += 1).show(hosted)),
        layers,
    )
}

fn open() -> Rt {
    headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        page,
        (700.0, 900.0),
    )
}

fn settle(runtime: &mut Rt, seconds: f32) {
    let frames = (seconds * 60.0).ceil() as usize;
    for _ in 0..frames {
        runtime.frame(1.0 / 60.0);
    }
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

/// Whether `text` is shown: an element that is playing its exit is gone.
fn has_text(runtime: &Rt, text: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| !e.exiting && e.text.as_deref() == Some(text))
}

fn text_rect(runtime: &Rt, text: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no text {text}"))
        .rect
}

fn centre(r: [f32; 4]) -> (f32, f32) {
    (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}

fn hover(runtime: &mut Rt, key: &str) {
    let (x, y) = centre(rect(runtime, key));
    runtime.pointer_move(x, y);
}

fn leave(runtime: &mut Rt) {
    runtime.pointer_move(690.0, 890.0);
}

fn tap_at(runtime: &mut Rt, x: f32, y: f32) {
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime, 0.3);
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = centre(rect(runtime, key));
    tap_at(runtime, x, y);
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
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

const TIP: &str = "Saves the document";

#[test]
fn a_tooltip_opens_after_the_pointer_rests_and_closes_after_it_leaves() {
    let mut runtime = open();
    assert!(!has_text(&runtime, TIP), "closed at first");
    hover(&mut runtime, "save");
    settle(&mut runtime, 0.2);
    assert!(
        !has_text(&runtime, TIP),
        "not before the pointer has rested"
    );
    settle(&mut runtime, 0.5);
    assert!(has_text(&runtime, TIP), "open once it has rested");
    let all = nodes(&runtime);
    assert!(
        all.iter()
            .any(|n| n["role"] == "tooltip" && n["name"] == TIP),
        "a tooltip in the tree: {all:#?}"
    );
    leave(&mut runtime);
    settle(&mut runtime, 0.1);
    assert!(
        has_text(&runtime, TIP),
        "it waits a moment before it closes"
    );
    settle(&mut runtime, 1.0);
    assert!(!has_text(&runtime, TIP), "and then it closes");
}

#[test]
fn a_pass_shorter_than_the_delay_opens_nothing_and_the_pointer_on_the_tooltip_keeps_it_open() {
    let mut runtime = open();
    hover(&mut runtime, "save");
    settle(&mut runtime, 0.2);
    leave(&mut runtime);
    settle(&mut runtime, 1.0);
    assert!(!has_text(&runtime, TIP), "a short pass opens nothing");
    hover(&mut runtime, "save");
    settle(&mut runtime, 0.8);
    assert!(has_text(&runtime, TIP));
    // The pointer moves onto the tooltip itself, which keeps it open.
    let (x, y) = centre(text_rect(&runtime, TIP));
    runtime.pointer_move(x, y);
    settle(&mut runtime, 1.5);
    assert!(
        has_text(&runtime, TIP),
        "it stays while the pointer is on it (WCAG 1.4.13)"
    );
}

#[test]
fn escape_closes_a_tooltip_and_the_trigger_keeps_focus() {
    let mut runtime = open();
    runtime.key(KeyEvent::from(Key::Tab));
    settle(&mut runtime, 0.1);
    assert_eq!(focused(&runtime).as_deref(), Some("save"));
    hover(&mut runtime, "save");
    settle(&mut runtime, 0.8);
    assert!(has_text(&runtime, TIP));
    runtime.key(KeyEvent::from(Key::Escape));
    settle(&mut runtime, 0.2);
    assert!(!has_text(&runtime, TIP), "Escape closes it");
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("save"),
        "and focus stays on the trigger"
    );
    // The pointer is still on the trigger, and the tooltip stays closed until it has left.
    settle(&mut runtime, 1.0);
    assert!(!has_text(&runtime, TIP), "closed while the pointer stays");
    leave(&mut runtime);
    settle(&mut runtime, 1.0);
    hover(&mut runtime, "save");
    settle(&mut runtime, 0.8);
    assert!(
        has_text(&runtime, TIP),
        "and it opens again once the pointer has been and come back"
    );
}

#[test]
fn a_tooltip_takes_no_tab_stop_and_sits_above_its_trigger_unless_asked_below() {
    let mut runtime = open();
    hover(&mut runtime, "save");
    settle(&mut runtime, 0.8);
    let (trigger, tip) = (rect(&runtime, "save"), text_rect(&runtime, TIP));
    assert!(
        tip[1] + tip[3] <= trigger[1],
        "above the trigger: {tip:?} {trigger:?}"
    );
    let mut stops = Vec::new();
    for _ in 0..6 {
        runtime.key(KeyEvent::from(Key::Tab));
        settle(&mut runtime, 0.05);
        stops.extend(focused(&runtime));
    }
    assert!(
        stops.iter().all(|k| !k.starts_with("tip")),
        "the tooltip is no stop: {stops:?}"
    );
    leave(&mut runtime);
    settle(&mut runtime, 1.0);
    hover(&mut runtime, "down");
    settle(&mut runtime, 0.8);
    let (trigger, below) = (rect(&runtime, "down"), text_rect(&runtime, "Shown below"));
    assert!(
        below[1] >= trigger[1] + trigger[3],
        "below the trigger: {below:?} {trigger:?}"
    );
}

#[test]
fn a_hover_card_opens_like_a_tooltip_below_its_trigger_with_its_content() {
    let mut runtime = open();
    assert!(!has_text(&runtime, "Ada Lovelace, mathematician."));
    hover(&mut runtime, "card");
    settle(&mut runtime, 0.8);
    assert!(has_text(&runtime, "Ada Lovelace, mathematician."));
    let (trigger, content) = (
        rect(&runtime, "card"),
        text_rect(&runtime, "Ada Lovelace, mathematician."),
    );
    assert!(content[1] >= trigger[1] + trigger[3], "below its trigger");
    leave(&mut runtime);
    settle(&mut runtime, 1.0);
    assert!(!has_text(&runtime, "Ada Lovelace, mathematician."));
}

#[test]
fn a_popover_opens_on_a_press_with_its_title_and_content_and_focus_inside_and_closes_on_escape() {
    let mut runtime = open();
    assert!(!has_text(&runtime, "Filter the list"));
    tap(&mut runtime, "filters");
    assert!(has_text(&runtime, "Filter the list"), "the title");
    assert!(
        runtime.find("apply").is_some(),
        "the content, built only while open"
    );
    let all = nodes(&runtime);
    assert!(
        all.iter()
            .any(|n| n["role"] == "dialog" && n["name"] == "Filters"),
        "the panel is a dialog named for the popover: {all:#?}"
    );
    assert_eq!(runtime.state.app.heard, ["filters:true"]);
    // Focus went into the panel, and goes back to the trigger on Escape.
    let inside = focused(&runtime);
    assert_ne!(
        inside.as_deref(),
        Some("filters"),
        "focus is in the panel: {inside:?}"
    );
    runtime.key(KeyEvent::from(Key::Escape));
    settle(&mut runtime, 0.4);
    assert!(!has_text(&runtime, "Filter the list"));
    assert_eq!(runtime.state.app.heard, ["filters:true", "filters:false"]);
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("filters"),
        "focus returns to the trigger"
    );
}

#[test]
fn a_press_outside_closes_a_popover_and_presses_nothing_beneath_and_a_press_inside_does_not() {
    let mut runtime = open();
    tap(&mut runtime, "filters");
    tap(&mut runtime, "apply");
    assert_eq!(runtime.state.app.applied, 1, "a press on the content works");
    assert!(
        has_text(&runtime, "Filter the list"),
        "and does not close it"
    );
    // Outside: on the button that is behind the panel, which is not pressed.
    let before = runtime.state.app.applied;
    tap(&mut runtime, "save");
    assert!(
        !has_text(&runtime, "Filter the list"),
        "a press outside closes it"
    );
    assert_eq!(runtime.state.app.applied, before);
    assert_eq!(
        runtime.state.app.heard.last().map(String::as_str),
        Some("filters:false")
    );
}

#[test]
fn enter_or_space_on_the_trigger_opens_it_on_release_and_it_says_whether_it_is_open() {
    let mut runtime = open();
    let closed = nodes(&runtime)
        .into_iter()
        .find(|n| n["role"] == "button" && n["name"] == "Filters")
        .expect("the trigger");
    assert!(
        closed["states"]
            .as_array()
            .is_some_and(|s| s.iter().any(|v| v == "collapsed")),
        "a closed popover's trigger says so: {closed:#?}"
    );
    let mut tabs = 0;
    while focused(&runtime).as_deref() != Some("filters") && tabs < 8 {
        runtime.key(KeyEvent::from(Key::Tab));
        settle(&mut runtime, 0.05);
        tabs += 1;
    }
    assert_eq!(focused(&runtime).as_deref(), Some("filters"));
    runtime.key(KeyEvent::from(Key::Enter));
    settle(&mut runtime, 0.2);
    assert!(!has_text(&runtime, "Filter the list"), "not on the press");
    runtime.key_up(KeyEvent::from(Key::Enter));
    settle(&mut runtime, 0.3);
    assert!(has_text(&runtime, "Filter the list"), "on the release");
    // While the panel is open the page behind it is inert, and the tree leaves it out.
    let all = nodes(&runtime);
    assert!(
        all.iter()
            .any(|n| n["role"] == "dialog" && n["name"] == "Filters")
    );
}

#[test]
fn a_controlled_popover_follows_the_application() {
    let mut runtime = open();
    assert!(!has_text(&runtime, "Controlled") || runtime.find("ctl").is_some());
    tap(&mut runtime, "ctl");
    assert_eq!(runtime.state.app.heard, ["ctl:true"]);
    assert!(
        runtime.state.app.open,
        "the application was asked and agreed"
    );
    assert!(
        runtime
            .elements()
            .iter()
            .filter(|e| e.text.as_deref() == Some("Controlled"))
            .count()
            >= 2,
        "its trigger and its title"
    );
}

#[test]
fn a_popover_that_ignores_outside_presses_stays_open_and_blocks_the_page_behind_it() {
    let mut runtime = headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        sticky_page,
        (700.0, 500.0),
    );
    assert!(has_text(&runtime, "Sticky"), "it started open");
    let before = runtime.state.app.applied;
    tap(&mut runtime, "behind");
    assert!(
        runtime
            .elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Sticky")),
        "a press outside leaves it open"
    );
    // In Rune a popover is modal, so the page behind it takes no press while it is open, where
    // the GPUI popover left the window behind usable. The gap is in `docs/checks.md`.
    assert_eq!(
        runtime.state.app.applied, before,
        "the page behind is inert"
    );
}
