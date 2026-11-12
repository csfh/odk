//! The host's surface: the application's foundation settings and what reads them. Nothing here
//! opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_theme::foundation::{Density, FoundationConfig, MotionDuration};
use odk_view::prelude::*;
use odk_view::{Input, NoWindow, focused_component, headless, on_host};
use rune::{Key, KeyEvent, Runtime, col};

type Rt = Runtime<Hosted<App>>;

fn face() -> odk_view::Face {
    static FACE: OnceLock<odk_view::Face> = OnceLock::new();
    *FACE.get_or_init(|| {
        let output = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", "sans"])
            .output()
            .expect("fc-match");
        assert!(output.status.success(), "fc-match sans");
        let path = String::from_utf8(output.stdout).expect("fc-match speaks utf-8");
        let bytes = std::fs::read(path.trim()).expect("read the sans font");
        odk_view::Face::leak(bytes, "Inter".into())
    })
}

#[derive(Default)]
struct App {
    seen: Vec<Option<String>>,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let body = col()
        .key("page")
        .padding(8.0)
        .gap(8.0)
        .child(Button::new("go", "Go", |_: &mut App| {}).show(hosted))
        .child(
            Input::new(
                "name",
                "Name",
                |_: &mut App, _: String| {},
                |_: &mut App| {},
            )
            .show(hosted),
        )
        .on_key(|h: &mut Hosted<App>, _: &KeyEvent, tx: &mut rune::Tx| {
            h.app.seen.push(focused_component(tx).map(str::to_owned));
            false
        });
    frame(hosted, cx, body, Vec::new())
}

fn open(config: FoundationConfig) -> Rt {
    let mut runtime = headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()).with_config(config),
        page,
        (320.0, 200.0),
    );
    for _ in 0..6 {
        runtime.frame(1.0 / 60.0);
    }
    runtime
}

fn height(runtime: &Rt, key: &str) -> f32 {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect[3]
}

fn with_density(density: Density) -> FoundationConfig {
    FoundationConfig {
        density,
        ..FoundationConfig::default()
    }
}

#[test]
fn the_applications_density_sizes_every_control_that_has_none_of_its_own() {
    let compact = open(with_density(Density::Compact));
    let normal = open(with_density(Density::Default));
    let roomy = open(with_density(Density::Comfortable));
    for key in ["go", "name-box"] {
        let (c, n, r) = (
            height(&compact, key),
            height(&normal, key),
            height(&roomy, key),
        );
        assert!(c < n && n < r, "{key}: {c} < {n} < {r}");
    }
}

#[test]
fn the_applications_own_reduced_motion_adds_to_the_users_and_reaches_the_tokens() {
    let still = open(FoundationConfig {
        reduced_motion: true,
        ..FoundationConfig::default()
    });
    assert!(
        still
            .state
            .foundation()
            .motion
            .transition(MotionDuration::Normal)
            .is_zero(),
        "transitions are instant"
    );
    let moving = open(FoundationConfig::default());
    assert!(
        !moving
            .state
            .foundation()
            .motion
            .transition(MotionDuration::Normal)
            .is_zero()
    );
}

#[test]
fn the_host_hands_out_the_tokens_it_draws_with_and_changes_its_settings() {
    let mut runtime = open(with_density(Density::Compact));
    assert_eq!(runtime.state.config().density, Density::Compact);
    assert_eq!(runtime.state.components().density(None), Density::Compact);
    runtime
        .state
        .configure(|config| config.density = Density::Comfortable);
    assert_eq!(
        runtime.state.components().density(None),
        Density::Comfortable
    );
    let before = height(&runtime, "go");
    runtime.invalidate();
    for _ in 0..6 {
        runtime.frame(1.0 / 60.0);
    }
    assert!(
        height(&runtime, "go") > before,
        "the next frame is drawn with them"
    );
}

#[test]
fn a_handler_can_ask_which_component_has_focus() {
    let mut runtime = open(FoundationConfig::default());
    let press = |runtime: &mut Rt, key: Key| {
        runtime.key(KeyEvent::from(key.clone()));
        runtime.key_up(KeyEvent::from(key));
        for _ in 0..6 {
            runtime.frame(1.0 / 60.0);
        }
    };
    // Tab moves focus to the button; a key pressed there passes the button on to the page.
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Text("x".into()));
    assert_eq!(
        runtime
            .state
            .app
            .seen
            .last()
            .and_then(Clone::clone)
            .as_deref(),
        Some("go"),
        "{:?}",
        runtime.state.app.seen
    );
}

#[test]
fn nothing_reaches_a_window_the_application_did_not_start_through_run() {
    assert_eq!(on_host::<App>(|_| {}), Err(NoWindow));
    assert_eq!(odk_view::set_reduced_motion::<App>(true), Err(NoWindow));
}
