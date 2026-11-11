//! `Chart` on the Rune host (ADR 0071, ADR 0075, ADR 0093). Nothing here opens a window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{
    Chart, ChartHeight, ChartPath, Face, Hosted, Ink, StrokeWidth, TextAnchor, TextBaseline, frame,
    headless,
};
use rune::{Cx, El, GesturePhase, Key, KeyEvent, MagnifyEvent, Mods, Runtime, WheelEvent, col};

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
    /// What each `paint` saw: the box and the progress.
    seen: Rc<RefCell<Vec<(f32, f32, f32)>>>,
    generation: u64,
    height: Option<ChartHeight>,
    interactive: bool,
    marks: bool,
    halo: bool,
    builds: Cell<usize>,
    pressed: Vec<(f32, f32)>,
    panned: Vec<(f32, f32)>,
    zoomed: Vec<(f32, f32, f32)>,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    hosted.app.builds.set(hosted.app.builds.get() + 1);
    let seen = hosted.app.seen.clone();
    let (marks, halo) = (hosted.app.marks, hosted.app.halo);
    let mut chart = Chart::<App>::new("chart", "Latency")
        .description("Latency over the last hour")
        .transition(hosted.app.generation)
        .paint(move |scene, progress| {
            seen.borrow_mut()
                .push((scene.width(), scene.height(), progress));
            if !marks {
                return;
            }
            scene.fill_rect(10.0, 10.0, 50.0, 20.0, Ink::Series(0));
            scene.stroke_rect(10.0, 40.0, 50.0, 20.0, Ink::Grid, StrokeWidth::Thin);
            scene.stroke(
                &ChartPath::new()
                    .move_to(0.0, 100.0)
                    .line_to(100.0, 80.0)
                    .curve_to(120.0, 60.0, 140.0, 120.0, 160.0, 90.0),
                Ink::Series(1),
                StrokeWidth::Regular,
            );
            scene.fill(
                &ChartPath::new()
                    .arc(200.0, 100.0, 40.0, 0.0, std::f32::consts::TAU)
                    .close(),
                Ink::SeriesFill(2),
            );
            scene.text(
                150.0,
                20.0,
                "start",
                Ink::Text,
                TextAnchor::Start,
                TextBaseline::Top,
                false,
            );
            scene.text(
                150.0,
                50.0,
                "middle",
                Ink::Text,
                TextAnchor::Middle,
                TextBaseline::Middle,
                halo,
            );
            scene.text(
                150.0,
                80.0,
                "end",
                Ink::TextMuted,
                TextAnchor::End,
                TextBaseline::Bottom,
                false,
            );
        });
    if let Some(height) = hosted.app.height {
        chart = chart.height(height);
    }
    if hosted.app.interactive {
        chart = chart
            .on_press(|app: &mut App, x, y| app.pressed.push((x, y)))
            .on_pan(|app: &mut App, dx, dy| app.panned.push((dx, dy)))
            .on_zoom(|app: &mut App, f, x, y| app.zoomed.push((f, x, y)));
    }
    let body = col()
        .key("page")
        .child(col().key("box").w(300.0).h(200.0).child(chart.show(hosted)));
    frame(hosted, cx, body, [])
}

fn open_with(app: App, theme: ThemeSnapshot) -> Rt {
    let mut runtime = headless(Hosted::new(app, theme, face()), page, (300.0, 200.0));
    settle(&mut runtime);
    runtime
}

fn open(app: App) -> Rt {
    open_with(app, ThemeSnapshot::emergency())
}

fn settle(runtime: &mut Rt) {
    for _ in 0..8 {
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

fn texts(runtime: &Rt, text: &str) -> Vec<[f32; 4]> {
    runtime
        .elements()
        .into_iter()
        .filter(|e| e.text.as_deref() == Some(text))
        .map(|e| e.rect)
        .collect()
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
fn paint_gets_the_box_of_the_last_layout_and_is_not_called_before_it() {
    let runtime = open(App::default());
    let seen = runtime.state.app.seen.borrow().clone();
    assert!(!seen.is_empty(), "painted once the box was known");
    assert!(
        seen.iter().all(|(w, h, _)| *w > 0.0 && *h > 0.0),
        "never with a zero box: {seen:?}"
    );
    let (w, h, progress) = *seen.last().unwrap();
    assert_eq!((w, h), (300.0, 200.0), "it fills its parent");
    assert_eq!(progress, 1.0, "no transition has run");
}

#[test]
fn the_marks_are_drawn_over_the_chart() {
    let none = open(App::default());
    let drawn = open(App {
        marks: true,
        ..App::default()
    });
    assert!(
        drawn.elements().len() > none.elements().len(),
        "the shapes and the text are elements"
    );
}

#[test]
fn a_text_is_placed_by_its_anchor_and_its_baseline() {
    let runtime = open(App {
        marks: true,
        ..App::default()
    });
    let start = texts(&runtime, "start")[0];
    assert!(
        (start[0] - 150.0).abs() < 1.5,
        "start: left at x, {start:?}"
    );
    assert!((start[1] - 20.0).abs() < 1.5, "top: top at y, {start:?}");
    let middle = texts(&runtime, "middle")[0];
    assert!(
        (middle[0] + middle[2] / 2.0 - 150.0).abs() < 1.5,
        "middle: centred on x, {middle:?}"
    );
    assert!(
        (middle[1] + middle[3] / 2.0 - 50.0).abs() < 1.5,
        "middle: centred on y, {middle:?}"
    );
    let end = texts(&runtime, "end")[0];
    assert!(
        (end[0] + end[2] - 150.0).abs() < 1.5,
        "end: right at x, {end:?}"
    );
    assert!(
        (end[1] + end[3] - 80.0).abs() < 1.5,
        "bottom: bottom at y, {end:?}"
    );
}

#[test]
fn a_halo_is_four_rims_under_the_text() {
    let plain = open(App {
        marks: true,
        ..App::default()
    });
    assert_eq!(texts(&plain, "middle").len(), 1);
    let haloed = open(App {
        marks: true,
        halo: true,
        ..App::default()
    });
    assert_eq!(texts(&haloed, "middle").len(), 5, "four rims and the text");
}

#[test]
fn a_chart_that_asks_for_a_height_is_that_tall_and_as_wide_as_its_parent() {
    let at = |height| {
        let runtime = open(App {
            height: Some(height),
            ..App::default()
        });
        rect(&runtime, "chart")
    };
    let (compact, regular, tall) = (
        at(ChartHeight::Compact),
        at(ChartHeight::Regular),
        at(ChartHeight::Tall),
    );
    for r in [compact, regular, tall] {
        assert_eq!(r[2], 300.0, "as wide as its parent");
    }
    // ADR 0075: s8, twice s12 and four times s12, which are 32, 96 and 192 at the default size.
    assert!(
        (regular[3] - 3.0 * compact[3]).abs() < 0.5,
        "{compact:?} {regular:?}"
    );
    assert!(
        (tall[3] - 2.0 * regular[3]).abs() < 0.5,
        "{regular:?} {tall:?}"
    );
    assert!(compact[3] < 200.0, "not the parent's height");
}

#[test]
fn a_transition_runs_from_0_to_1_and_then_the_chart_asks_for_nothing() {
    let mut runtime = open(App::default());
    runtime.state.app.seen.borrow_mut().clear();
    runtime.state.app.generation = 1;
    runtime.invalidate();
    for _ in 0..90 {
        runtime.frame(1.0 / 60.0);
    }
    let seen: Vec<f32> = runtime
        .state
        .app
        .seen
        .borrow()
        .iter()
        .map(|s| s.2)
        .collect();
    assert!(
        seen.first().is_some_and(|p| *p < 0.5),
        "starts near 0: {seen:?}"
    );
    assert_eq!(seen.last().copied(), Some(1.0), "ends at 1: {seen:?}");
    assert!(
        seen.windows(2).all(|w| w[1] >= w[0]),
        "never goes back: {seen:?}"
    );
    assert!(
        seen.iter().any(|p| *p > 0.0 && *p < 1.0),
        "passes through between: {seen:?}"
    );
    let builds = runtime.state.app.builds.get();
    for _ in 0..60 {
        runtime.frame(1.0 / 60.0);
    }
    assert_eq!(
        runtime.state.app.builds.get(),
        builds,
        "a chart at rest asks for no frames (item 35)"
    );
}

#[test]
fn reduced_motion_has_a_transition_done_at_once() {
    let mut theme = ThemeSnapshot::emergency();
    theme.reduced_motion = true;
    let mut runtime = open_with(App::default(), theme);
    runtime.state.app.seen.borrow_mut().clear();
    runtime.state.app.generation = 1;
    runtime.invalidate();
    settle(&mut runtime);
    let seen: Vec<f32> = runtime
        .state
        .app
        .seen
        .borrow()
        .iter()
        .map(|s| s.2)
        .collect();
    assert!(
        !seen.is_empty() && seen.iter().all(|p| *p == 1.0),
        "{seen:?}"
    );
}

#[test]
fn a_chart_with_no_pointer_handlers_is_not_a_tab_stop() {
    let mut runtime = open(App::default());
    press(&mut runtime, Key::Tab);
    assert_ne!(focused(&runtime).as_deref(), Some("chart"));
}

#[test]
fn the_keys_pan_zoom_and_press() {
    let mut runtime = open(App {
        interactive: true,
        ..App::default()
    });
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("chart"));
    press(&mut runtime, Key::Right);
    press(&mut runtime, Key::Up);
    press(&mut runtime, Key::Text("+".into()));
    press(&mut runtime, Key::Text("-".into()));
    press(&mut runtime, Key::Enter);
    press(&mut runtime, Key::Text(" ".into()));
    let app = &runtime.state.app;
    assert_eq!(
        app.panned,
        [(15.0, 0.0), (0.0, -10.0)],
        "a twentieth of the box"
    );
    assert_eq!(app.zoomed.len(), 2);
    assert!(app.zoomed[0].0 > 1.0 && app.zoomed[1].0 < 1.0);
    assert_eq!(
        (app.zoomed[0].1, app.zoomed[0].2),
        (150.0, 100.0),
        "about the middle"
    );
    assert_eq!(app.pressed, [(150.0, 100.0), (150.0, 100.0)], "the middle");
}

#[test]
fn a_drag_reports_each_move_and_escape_pans_back() {
    let mut runtime = open(App {
        interactive: true,
        ..App::default()
    });
    let (x, y) = runtime.find("chart").expect("the chart");
    runtime.pointer_down(x, y);
    runtime.pointer_move(x + 10.0, y);
    runtime.pointer_move(x + 25.0, y + 5.0);
    settle(&mut runtime);
    let moved: (f32, f32) = runtime
        .state
        .app
        .panned
        .iter()
        .fold((0.0, 0.0), |a, d| (a.0 + d.0, a.1 + d.1));
    assert_eq!(moved, (25.0, 5.0), "{:?}", runtime.state.app.panned);
    assert!(runtime.state.app.panned.len() >= 2, "each move is reported");
    assert_eq!(
        runtime.state.app.pressed.len(),
        1,
        "the press is reported once"
    );
    runtime.key(KeyEvent::from(Key::Escape));
    settle(&mut runtime);
    runtime.pointer_up(x + 25.0, y + 5.0);
    settle(&mut runtime);
    let total: (f32, f32) = runtime
        .state
        .app
        .panned
        .iter()
        .fold((0.0, 0.0), |a, d| (a.0 + d.0, a.1 + d.1));
    assert_eq!(
        total,
        (0.0, 0.0),
        "back where it began: {:?}",
        runtime.state.app.panned
    );
}

#[test]
fn the_wheel_zooms_about_the_pointer_and_shift_pans() {
    let mut runtime = open(App {
        interactive: true,
        ..App::default()
    });
    let (x, y) = runtime.find("chart").expect("the chart");
    runtime.wheel_event(WheelEvent::new(0.0, 40.0).at(x, y));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.zoomed.len(), 1);
    assert!(runtime.state.app.zoomed[0].0 > 1.0, "the wheel up zooms in");
    let shift = Mods {
        shift: true,
        ..Mods::default()
    };
    runtime.wheel_event(WheelEvent::new(0.0, 30.0).at(x, y).mods(shift));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.panned, [(0.0, 30.0)]);
    assert_eq!(
        runtime.state.app.zoomed.len(),
        1,
        "a shifted wheel does not zoom"
    );
    runtime.magnify_event(MagnifyEvent::new(0.5).at(x, y).phase(GesturePhase::Moved));
    settle(&mut runtime);
    assert_eq!(runtime.state.app.zoomed.len(), 2);
    assert!(
        (runtime.state.app.zoomed[1].0 - 1.5).abs() < 1e-4,
        "a pinch of 0.5 scales by 1.5"
    );
}

#[test]
fn a_chart_reports_an_image_with_its_name_and_description() {
    let runtime = open(App::default());
    let chart = nodes(&runtime)
        .into_iter()
        .find(|n| n["key"] == "chart")
        .expect("the chart in the tree");
    assert_eq!(chart["role"], "image", "{chart}");
    assert_eq!(chart["name"], "Latency");
    assert_eq!(chart["description"], "Latency over the last hour");
}
