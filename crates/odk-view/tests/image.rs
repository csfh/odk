//! `Image` on the Rune host. Nothing here opens a window.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, Image, ImageFit, frame, headless};
use rune::{Cx, El, Runtime, col};

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

const PNG: &[u8] = include_bytes!("fixtures/two.png");
const JPEG: &[u8] = include_bytes!("fixtures/two.jpg");
const WEBP: &[u8] = include_bytes!("fixtures/two.webp");
const GIF: &[u8] = include_bytes!("fixtures/two.gif");
const BMP: &[u8] = include_bytes!("fixtures/two.bmp");

#[derive(Default)]
struct App {
    source: Option<Arc<[u8]>>,
    avatar: bool,
    fit: Option<ImageFit>,
    /// How many times the view was built.
    builds: std::cell::Cell<usize>,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    hosted.app.builds.set(hosted.app.builds.get() + 1);
    let mut image = if hosted.app.avatar {
        Image::avatar("pic", "Your avatar")
    } else {
        Image::new("pic", "Current background")
    };
    if let Some(bytes) = &hosted.app.source {
        image = image.source(bytes.clone());
    }
    if let Some(fit) = hosted.app.fit {
        image = image.fit(fit);
    }
    let body = col()
        .key("page")
        .padding(24.0)
        .child(col().key("box").w(200.0).h(120.0).child(image.show(hosted)));
    frame(hosted, cx, body, [])
}

fn open(app: App) -> Rt {
    headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (320.0, 240.0),
    )
}

fn bytes(data: &[u8]) -> Option<Arc<[u8]>> {
    Some(Arc::from(data))
}

/// Frames until the decode has landed and the view has run again, or two seconds.
fn until_decoded(runtime: &mut Rt) {
    let started = Instant::now();
    let builds = runtime.state.app.builds.get();
    while runtime.state.app.builds.get() == builds && started.elapsed() < Duration::from_secs(2) {
        runtime.frame(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(5));
    }
    // The placeholder's fill fades out on Rune's own transition.
    for _ in 0..60 {
        runtime.frame(1.0 / 60.0);
    }
}

fn element(runtime: &Rt) -> rune::ElementInfo {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("pic"))
        .expect("the image")
}

/// Whether the image still draws its placeholder fill: it is gone once pixels are drawn.
fn placeholder(runtime: &Rt) -> bool {
    element(runtime).fill.a > 0.0
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

fn described(runtime: &Rt) -> serde_json::Value {
    nodes(runtime)
        .into_iter()
        .find(|n| n["key"] == "pic")
        .expect("the image in the tree")
}

#[test]
fn every_format_adr_0072_names_is_decoded_and_drawn() {
    for (name, data) in [
        ("png", PNG),
        ("jpeg", JPEG),
        ("webp", WEBP),
        ("gif", GIF),
        ("bmp", BMP),
    ] {
        let mut runtime = open(App {
            source: bytes(data),
            ..App::default()
        });
        assert!(
            placeholder(&runtime),
            "{name}: a placeholder while decoding"
        );
        until_decoded(&mut runtime);
        assert!(!placeholder(&runtime), "{name}: drawn once decoded");
        assert!(
            described(&runtime)["description"].is_null(),
            "{name}: nothing to apologise for"
        );
    }
}

#[test]
fn it_fills_the_box_its_parent_gives_it() {
    let mut runtime = open(App {
        source: bytes(PNG),
        ..App::default()
    });
    until_decoded(&mut runtime);
    let image = element(&runtime).rect;
    assert_eq!((image[2], image[3]), (200.0, 120.0));
}

#[test]
fn an_avatar_is_a_circle_of_the_foundations_size() {
    let mut runtime = open(App {
        source: bytes(PNG),
        avatar: true,
        ..App::default()
    });
    until_decoded(&mut runtime);
    let e = element(&runtime);
    assert_eq!(e.rect[2], e.rect[3], "square");
    assert!(e.rect[2] > 0.0);
    assert_eq!(e.radius, e.rect[2] / 2.0, "a circle");
}

#[test]
fn before_its_bytes_arrive_it_is_a_busy_placeholder() {
    let runtime = open(App::default());
    assert!(placeholder(&runtime));
    let node = described(&runtime);
    assert_eq!(node["role"], "image");
    assert_eq!(node["name"], "Current background");
}

#[test]
fn bytes_that_are_not_an_image_say_so_and_draw_a_mark() {
    let runtime = open(App {
        source: bytes(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
        ..App::default()
    });
    assert!(placeholder(&runtime), "the placeholder stays");
    assert_eq!(
        described(&runtime)["description"],
        "The image could not be shown"
    );
    let none = open(App::default());
    let count = |r: &Rt| r.elements().len();
    assert!(count(&runtime) > count(&none), "and a mark is drawn on it");
}

#[test]
fn a_file_that_looks_like_an_image_and_does_not_decode_ends_as_the_marked_placeholder() {
    let mut truncated = PNG[..24].to_vec();
    truncated.extend_from_slice(b"not the rest of a png");
    let mut runtime = open(App {
        source: bytes(&truncated),
        ..App::default()
    });
    let before = runtime.elements().len();
    until_decoded(&mut runtime);
    assert!(placeholder(&runtime), "never drawn as pixels");
    assert!(
        runtime.elements().len() > before,
        "the mark arrives when the decode fails"
    );
}

#[test]
fn a_settled_image_asks_for_no_more_views() {
    let mut runtime = open(App {
        source: bytes(PNG),
        ..App::default()
    });
    until_decoded(&mut runtime);
    let builds = runtime.state.app.builds.get();
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
    assert_eq!(
        runtime.state.app.builds.get(),
        builds,
        "idle frames rebuild nothing (item 35)"
    );
}

#[test]
fn the_same_bytes_are_not_decoded_again_when_the_view_runs_again() {
    let mut runtime = open(App {
        source: bytes(PNG),
        fit: Some(ImageFit::Contain),
        ..App::default()
    });
    until_decoded(&mut runtime);
    runtime.invalidate();
    runtime.frame(1.0 / 60.0);
    assert!(
        !placeholder(&runtime),
        "a rebuilt view draws the kept image at once, with no placeholder frame"
    );
}
