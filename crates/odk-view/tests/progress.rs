//! `Progress` and `Spinner` on the Rune host. Nothing here opens a window.

use std::cell::Cell;
use std::sync::{Arc, OnceLock};

use odk_theme::ThemeSnapshot;
use odk_theme::component::ComponentTokens;
use odk_theme::foundation::{FoundationConfig, FoundationTokens};
use odk_view::{Button, Face, Hosted, Progress, Spinner, Tone, frame, headless};
use rune::{Cx, El, Runtime, col};

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

fn tokens() -> ComponentTokens {
    let theme = ThemeSnapshot::emergency();
    let foundation = FoundationTokens::scaled(&FoundationConfig::default(), theme.text);
    ComponentTokens::new(
        Arc::new(foundation),
        theme.semantic.clone(),
        theme.generation,
    )
}

struct App {
    value: f64,
    spinning: bool,
    /// How many times the view was built.
    builds: Cell<u32>,
}

type Rt = Runtime<Hosted<App>>;

const BRAILLE: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    app.builds.set(app.builds.get() + 1);
    let mut body = col()
        .w(400.0)
        .gap(12.0)
        .child(
            Progress::new("upload", "Uploading photos")
                .value(app.value)
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(
            Progress::new("done", "Done")
                .value(100.0)
                .tone(Tone::Success)
                .show(hosted),
        )
        .child(
            Progress::new("paused", "Paused")
                .value(30.0)
                .paused(true)
                .show(hosted),
        )
        .child(
            Progress::new("off", "Disabled")
                .value(30.0)
                .disabled(true)
                .show(hosted),
        )
        .child(
            Progress::new("files", "Copying")
                .value(3.0)
                .max(7.0)
                .value_text("3 of 7 files")
                .show_label(true)
                .show_value(true)
                .show(hosted),
        )
        .child(Progress::new("below", "Below").value(-5.0).show(hosted))
        .child(Progress::new("above", "Above").value(500.0).show(hosted))
        .child(
            Progress::new("nan", "Not a number")
                .value(f64::NAN)
                .show(hosted),
        )
        .child(
            Progress::new("zero", "No most")
                .value(5.0)
                .max(0.0)
                .show(hosted),
        )
        .child(
            col().w(120.0).child(
                Progress::new("long", "A name far longer than the room the line gives it")
                    .value(10.0)
                    .show_label(true)
                    .show_value(true)
                    .show(hosted),
            ),
        )
        .child(Button::new("faster", "Faster", |app: &mut App| app.value = 80.0).show(hosted));
    if app.spinning {
        body = body.child(Spinner::new().label("Syncing").show(hosted));
        body = body
            .child(Button::new("stop", "Stop", |app: &mut App| app.spinning = false).show(hosted));
    }
    frame(hosted, cx, body, [])
}

fn open_with(theme: ThemeSnapshot, spinning: bool) -> Rt {
    headless(
        Hosted::new(
            App {
                value: 42.0,
                spinning,
                builds: Cell::new(0),
            },
            theme,
            face(),
        ),
        page,
        (500.0, 800.0),
    )
}

fn open(spinning: bool) -> Rt {
    open_with(ThemeSnapshot::emergency(), spinning)
}

fn settle(runtime: &mut Rt) {
    for _ in 0..30 {
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

fn bar_share(runtime: &Rt, id: &str) -> f32 {
    rect(runtime, &format!("{id}-bar"))[2] / rect(runtime, &format!("{id}-track"))[2]
}

#[test]
fn a_bar_reports_its_value_in_the_tree_and_its_width_is_the_share_done() {
    let runtime = open(false);
    let all = nodes(&runtime);
    let upload = all
        .iter()
        .find(|n| n["role"] == "progress-indicator" && n["name"] == "Uploading photos")
        .unwrap_or_else(|| panic!("no progress bar: {all:#?}"));
    assert_eq!(upload["value"], "42%", "the tree reports the per cent");
    let files = all
        .iter()
        .find(|n| n["name"] == "Copying")
        .expect("the copying bar");
    assert_eq!(files["value"], "3 of 7 files", "or the words it was given");
    assert!((bar_share(&runtime, "upload") - 0.42).abs() < 0.01);
    assert!((bar_share(&runtime, "files") - 3.0 / 7.0).abs() < 0.01);
    assert_eq!(bar_share(&runtime, "done"), 1.0);
    // The track is as wide as its column.
    assert_eq!(rect(&runtime, "upload-track")[2], 400.0);
}

#[test]
fn no_value_can_break_a_bar() {
    let runtime = open(false);
    assert_eq!(bar_share(&runtime, "below"), 0.0);
    assert_eq!(bar_share(&runtime, "above"), 1.0);
    assert_eq!(bar_share(&runtime, "nan"), 0.0);
    assert_eq!(bar_share(&runtime, "zero"), 0.0);
}

#[test]
fn the_value_is_shown_in_words_or_per_cent_and_a_long_name_is_cut() {
    let runtime = open(false);
    let texts: Vec<String> = runtime
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect();
    assert!(texts.contains(&"42%".to_string()), "{texts:?}");
    assert!(texts.contains(&"3 of 7 files".to_string()));
    assert!(!texts.contains(&"43%".to_string()));
    let long = runtime
        .elements()
        .into_iter()
        .find(|e| {
            e.text
                .as_deref()
                .is_some_and(|t| t.starts_with("A name far longer"))
        })
        .expect("the long name")
        .rect;
    let track = rect(&runtime, "long-track");
    assert!(
        long[0] + long[2] <= track[0] + track[2],
        "the name overflows its line: {long:?}"
    );
    let one_line = runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("Uploading photos"))
        .expect("a one-line name")
        .rect[3];
    assert_eq!(long[3], one_line, "cut to one line");
}

#[test]
fn a_new_value_moves_the_bar_on_a_spring_and_at_once_with_reduced_motion() {
    for reduced in [false, true] {
        let mut theme = ThemeSnapshot::emergency();
        theme.reduced_motion = reduced;
        let mut runtime = open_with(theme, false);
        let (x, y) = runtime.find("faster").unwrap();
        runtime.pointer_down(x, y);
        runtime.pointer_up(x, y);
        runtime.frame(1.0 / 60.0);
        let share = bar_share(&runtime, "upload");
        if reduced {
            assert!((share - 0.8).abs() < 0.01, "at its place at once: {share}");
        } else {
            assert!(
                share < 0.79,
                "the bar is still on its way after one frame: {share}"
            );
        }
        settle(&mut runtime);
        assert!(
            (bar_share(&runtime, "upload") - 0.8).abs() < 0.01,
            "and arrives"
        );
    }
}

#[test]
fn a_spinner_is_a_progress_indicator_named_for_what_is_loading() {
    let runtime = open(true);
    let all = nodes(&runtime);
    assert!(
        all.iter()
            .any(|n| n["role"] == "progress-indicator" && n["name"] == "Syncing"),
        "{all:#?}"
    );
}

fn glyph(runtime: &Rt) -> Option<usize> {
    runtime
        .elements()
        .iter()
        .find_map(|e| BRAILLE.iter().position(|g| e.text.as_deref() == Some(*g)))
}

#[test]
fn a_spinner_walks_its_frames_once_a_turn_and_the_view_runs_only_when_the_glyph_changes() {
    let turn = tokens().spinner(tokens().density(None)).turn.as_secs_f32();
    let mut runtime = open(true);
    assert_eq!(glyph(&runtime), Some(0));
    let builds = runtime.state.app.builds.get();
    // Two turns at 60 frames a second, as a display would draw.
    let mut seen = vec![0usize];
    let frames = (2.0 * turn * 60.0) as usize;
    for _ in 0..frames {
        runtime.frame(1.0 / 60.0);
        let now = glyph(&runtime).expect("a glyph");
        if seen.last() != Some(&now) {
            seen.push(now);
        }
    }
    // Every frame of the turn was shown, in order, twice round.
    let expected: Vec<usize> = (0..=16).map(|n| n % 8).collect();
    assert_eq!(
        &seen[..seen.len().min(expected.len())],
        &expected[..seen.len().min(expected.len())]
    );
    assert!(seen.len() >= 16, "{seen:?}");
    let rebuilt = runtime.state.app.builds.get() - builds;
    assert!(
        rebuilt <= 18,
        "the view ran {rebuilt} times in {frames} frames; it should run once for each glyph (16)"
    );
}

#[test]
fn a_window_with_no_spinner_is_never_woken_and_one_with_a_spinner_wakes_for_the_next_glyph() {
    let turn = tokens().spinner(tokens().density(None)).turn.as_secs_f64();
    let mut idle = open(false);
    settle(&mut idle);
    assert_eq!(
        idle.wake_deadline(),
        None,
        "nothing is drawing, so nothing wakes"
    );
    let mut busy = open(true);
    busy.frame(1.0 / 60.0);
    let wake = busy.wake_deadline().expect("a spinner asks for a wake");
    let now = busy.state.app.builds.get(); // keep the runtime in use
    let _ = now;
    // The next glyph is a step (an eighth of a turn) away, not the next frame: the wake is not
    // for 60 frames a second.
    let step = turn / 8.0;
    assert!(
        wake > 0.8 * step && wake <= step + 0.05,
        "the wake is for the next glyph: {wake} for a step of {step} s"
    );
}

#[test]
fn once_the_spinner_is_gone_the_wakes_stop() {
    let mut runtime = open(true);
    runtime.frame(1.0 / 60.0);
    assert!(runtime.wake_deadline().is_some());
    let (x, y) = runtime.find("stop").unwrap();
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    // The frame that builds the page without it, and the poll after it.
    for _ in 0..5 {
        runtime.frame(1.0 / 60.0);
    }
    // A wake already asked for may still come once; none is asked for after it.
    for _ in 0..200 {
        runtime.frame(1.0 / 60.0);
    }
    assert_eq!(runtime.wake_deadline(), None, "no spinner, no wake");
}

#[test]
fn a_spinner_stands_still_with_reduced_motion_and_asks_for_nothing() {
    let mut theme = ThemeSnapshot::emergency();
    theme.reduced_motion = true;
    let mut runtime = open_with(theme, true);
    for _ in 0..120 {
        runtime.frame(1.0 / 60.0);
    }
    assert!(
        runtime
            .elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("⣿")),
        "a still glyph, which is visible"
    );
    assert_eq!(glyph(&runtime), None, "no turning glyph");
    assert_eq!(runtime.wake_deadline(), None, "and no wake");
}
