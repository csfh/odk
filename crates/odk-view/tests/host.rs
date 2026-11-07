//! The host, on Rune's headless runtime. Nothing here opens a window on the user's display.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, frame, headless, ink};
use rune::{Cx, El, TextStyle, el, text};

struct Page {
    popup: bool,
}

fn font_match(pattern: &str) -> String {
    let output = Command::new("fc-match")
        .args(["-f", "%{file}\n%{family}", pattern])
        .output()
        .expect("fc-match");
    assert!(output.status.success(), "fc-match {pattern}");
    String::from_utf8(output.stdout).expect("fc-match speaks utf-8")
}

fn page(hosted: &Hosted<Page>, cx: &mut Cx<Hosted<Page>>) -> El {
    let mut layers = Vec::new();
    if hosted.app.popup {
        layers.push(el().key("popup").child(text("Pop").key("pop")));
    }
    let body = el()
        .key("page")
        .child(
            text("Hello")
                .key("label")
                .font(TextStyle::new(hosted.theme().text.base_size(), 400.0))
                .color(ink(hosted.theme().semantic.text.primary)),
        )
        .child(el().key("box").h(hosted.theme().text.px(28.0)).w(40.0));
    frame(hosted, cx, body, layers)
}

fn open(theme: ThemeSnapshot, popup: bool) -> rune::Runtime<Hosted<Page>> {
    headless(
        Hosted::new(Page { popup }, theme, Face::bundled()),
        page,
        (480.0, 360.0),
    )
}

fn settle<T>(runtime: &mut rune::Runtime<Hosted<T>>) {
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
}

#[test]
fn the_emergency_theme_paints_the_canvas_and_the_text() {
    let theme = ThemeSnapshot::emergency();
    let runtime = open(theme.clone(), false);
    let root = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("odk-root"))
        .expect("root");
    assert_eq!(root.fill, ink(theme.semantic.surface.canvas));
    let label = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("label"))
        .expect("label");
    assert_eq!(label.text_color, Some(ink(theme.semantic.text.primary)));
    assert!(runtime.find("popup").is_none());
}

#[test]
fn a_popup_is_a_layer_after_the_page() {
    let runtime = open(ThemeSnapshot::emergency(), true);
    let elements = runtime.elements();
    let root = elements
        .iter()
        .position(|element| element.key.as_deref() == Some("odk-root"))
        .expect("root");
    let popup = elements
        .iter()
        .find(|element| element.key.as_deref() == Some("popup"))
        .expect("popup");
    assert_eq!(popup.parent, Some(root));
}

#[test]
fn the_text_size_scales_a_control() {
    let at = |px: &str| {
        let theme = ThemeSnapshot::from_toml(EMERGENCY, Some(px)).expect("shell");
        let runtime = open(theme, false);
        let ([_, _, _, height], _) = runtime.presented("box").expect("box");
        height
    };
    let small = at("[font]\nbase-size = 12\n");
    let large = at("[font]\nbase-size = 18\n");
    assert!((large / small - 1.5).abs() < 0.02, "{large} / {small}");
}

const EMERGENCY: &str = r##"
mode = "dark"
accent = "#d0d0d0"
muted = "#8a8a8a"
background = "#1c1c1c"
dark_background = "#141414"
lighter_background = "#2a2a2a"
foreground = "#d0d0d0"
red = "#d0d0d0"
yellow = "#d0d0d0"
green = "#d0d0d0"
blue = "#d0d0d0"
"##;

#[test]
fn reduced_motion_from_the_snapshot_wins_over_the_desktop() {
    let mut reduced = ThemeSnapshot::emergency();
    reduced.reduced_motion = true;
    let mut runtime = open(reduced, false);
    runtime.set_system_reduced_motion(false);
    settle(&mut runtime);
    assert!(runtime.reduced_motion);

    let mut runtime = open(ThemeSnapshot::emergency(), false);
    runtime.set_system_reduced_motion(true);
    settle(&mut runtime);
    assert!(!runtime.reduced_motion);
}

fn probe(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        text("iiiiiiiiiiWWWWWWWWWW")
            .key("probe")
            .font(TextStyle::new(14.0, 400.0)),
        [],
    )
}

fn width(face: Face) -> f32 {
    let runtime = headless(
        Hosted::new((), ThemeSnapshot::emergency(), face),
        probe,
        (480.0, 80.0),
    );
    runtime.presented("probe").expect("probe").0[2]
}

#[test]
fn a_font_given_as_bytes_is_the_one_that_is_measured() {
    let matched = font_match("monospace");
    let mut lines = matched.lines();
    let path = lines.next().expect("font file");
    let family = lines.next().expect("family").to_string();
    let bytes = std::fs::read(path).expect("font bytes");
    let given = width(Face::leak(bytes, family));
    let bundled = width(Face::bundled());
    assert_ne!(
        given, bundled,
        "the handed-over face should not measure as Inter"
    );
}

#[test]
fn a_missing_wayland_display_returns_instead_of_panicking() {
    let dir = std::env::temp_dir().join(format!("odk-view-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("runtime dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_display-probe"))
        .env("ODK_VIEW_MISS_DISPLAY", "1")
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", "odk-view-missing")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("display probe");
    let mut stderr = child.stderr.take().expect("stderr");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            panic!("start() did not return");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    let _ = std::io::Read::read_to_string(&mut stderr, &mut err);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "missing display: {status}\n{err}");
}
