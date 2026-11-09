//! Checklist item 23 for the controls on the Rune host: a label longer than its room is cut
//! inside the control, never drawn past it. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Hosted, Select, frame, headless};
use rune::{Cx, El, Runtime, col};

const LONG: &str = "A label far too long for the room the list gives it, however wide that is";
const UNBROKEN: &str = "Unbreakable_word_with_no_place_to_wrap_at_all_in_the_list";
const JAPANESE: &str = "日本語の文字を含む、とても長いラベルで、リストの幅に収まらないもの";

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
    chosen: Option<usize>,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let mut layers = Vec::new();
    let options = [LONG, UNBROKEN, JAPANESE]
        .into_iter()
        .map(str::to_string)
        .collect();
    let select = Select::new(
        "worst",
        "Worst case",
        options,
        hosted.app.chosen,
        |app: &mut App, index| app.chosen = Some(index),
    )
    .show(hosted, &mut layers);
    frame(hosted, cx, col().child(select), layers)
}

fn right_edge(runtime: &Runtime<Hosted<App>>, key: &str) -> f32 {
    let rect = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect;
    rect[0] + rect[2]
}

fn text_right_edge(runtime: &Runtime<Hosted<App>>, shown: &str) -> f32 {
    let rect = runtime
        .elements()
        .into_iter()
        .find(|element| element.text.as_deref() == Some(shown))
        .unwrap_or_else(|| panic!("no text {shown}"))
        .rect;
    rect[0] + rect[2]
}

fn open() -> Runtime<Hosted<App>> {
    let mut runtime = headless(
        Hosted::new(App::default(), ThemeSnapshot::emergency(), face()),
        page,
        (600.0, 400.0),
    );
    let (x, y) = runtime.find("worst-trigger").expect("the trigger");
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
    runtime
}

#[test]
fn a_long_option_is_cut_inside_the_list() {
    let runtime = open();
    let list = right_edge(&runtime, "worst-list");
    for (index, label) in [LONG, UNBROKEN, JAPANESE].into_iter().enumerate() {
        let row = right_edge(&runtime, &format!("worst-{index}"));
        assert!(row <= list, "row {index} is wider than the list");
        let shown = text_right_edge(&runtime, label);
        assert!(
            shown <= row,
            "{label:?} is drawn past its row: {shown} > {row}"
        );
    }
}
