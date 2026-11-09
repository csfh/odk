//! `Text`, `Heading`, `Separator`, `Kbd` and `Skeleton` on the Rune host. Nothing here opens a
//! window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Face, Flow, Heading, Hosted, Kbd, Separator, Skeleton, Text, frame, headless};
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

fn hosted() -> Hosted<()> {
    Hosted::new((), ThemeSnapshot::emergency(), face())
}

fn rect(runtime: &Runtime<Hosted<()>>, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
}

fn nodes(runtime: &Runtime<Hosted<()>>) -> Vec<serde_json::Value> {
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

fn described(page: fn(&Hosted<()>, &mut Cx<Hosted<()>>) -> El) -> Vec<serde_json::Value> {
    nodes(&headless(hosted(), page, (400.0, 400.0)))
}

fn find<'a>(all: &'a [serde_json::Value], role: &str, label: &str) -> &'a serde_json::Value {
    all.iter()
        .find(|node| node["role"] == role && node["name"] == label)
        .unwrap_or_else(|| panic!("no {role} named {label}: {all:#?}"))
}

fn describes(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .child(Heading::new("Title").show(hosted))
            .child(Heading::new("Section").level(2).show(hosted))
            .child(Heading::new("Deep").level(5).show(hosted))
            .child(Text::new("Some body text.").show(hosted))
            .child(Kbd::new("Ctrl+K").show(hosted))
            .child(Separator::horizontal().show(hosted))
            .child(Skeleton::text(2).label("Loading the list").show(hosted)),
        [],
    )
}

#[test]
fn each_one_describes_itself_to_the_tree() {
    let all = described(describes);
    let level = |label: &str| find(&all, "heading", label)["level"].as_u64();
    assert_eq!(level("Title"), Some(1));
    assert_eq!(level("Section"), Some(2));
    assert_eq!(
        level("Deep"),
        Some(5),
        "levels past 3 draw alike and keep theirs"
    );
    // Rune calls body text a label and a progress bar a progress indicator. It leaves a
    // separator, which has no name, out of the tree (`docs/checks.md`).
    find(&all, "label", "Some body text.");
    find(&all, "code", "Ctrl+K");
    find(&all, "progress-indicator", "Loading the list");
}

fn sizes(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .child(row().child(Text::new("body").show(hosted).key("body")))
            .child(Heading::new("one").show(hosted).key("h1"))
            .child(Heading::new("two").level(2).show(hosted).key("h2"))
            .child(Heading::new("three").level(3).show(hosted).key("h3"))
            .child(Heading::new("four").level(4).show(hosted).key("h4")),
        [],
    )
}

#[test]
fn a_heading_is_larger_by_level_and_levels_past_three_draw_alike() {
    let runtime = headless(hosted(), sizes, (400.0, 400.0));
    let height = |key: &str| rect(&runtime, key)[3];
    assert!(
        height("h1") > height("h2"),
        "a title is larger than a heading"
    );
    assert!(
        height("h2") > height("h3"),
        "a heading is larger than a subtitle"
    );
    assert_eq!(height("h3"), height("h4"));
    assert!(height("body") < height("h1"));
}

const LONG: &str =
    "A sentence that is a good deal longer than the one hundred points the column gives it";

fn flows(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let column = |key: &str, flow: Flow| {
        col().key(key).w(100.0).child(
            Text::new(LONG)
                .flow(flow)
                .show(hosted)
                .key(format!("{key}-text")),
        )
    };
    frame(
        hosted,
        cx,
        row()
            .child(column("wrap", Flow::Wrap))
            .child(column("end", Flow::End))
            .child(column("middle", Flow::Middle))
            .child(column("clamp", Flow::Clamp(2))),
        [],
    )
}

fn unbroken(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col().w(100.0).child(
            Text::new("Unbreakable_word_with_no_place_to_wrap_at_all_in_the_column")
                .show(hosted)
                .key("word"),
        ),
        [],
    )
}

#[test]
fn wrapping_text_breaks_a_word_that_is_wider_than_its_room() {
    let runtime = headless(hosted(), unbroken, (400.0, 400.0));
    let [_, _, width, height] = rect(&runtime, "word");
    assert!(width <= 100.0, "the word overflows its column: {width}");
    assert!(height > 20.0, "it takes more than one line: {height}");
}

#[test]
fn text_flows_as_it_is_asked_when_it_is_longer_than_its_room() {
    let runtime = headless(hosted(), flows, (600.0, 400.0));
    let [_, _, wrap_w, wrap_h] = rect(&runtime, "wrap-text");
    let one_line = rect(&runtime, "end-text")[3];
    assert!(
        wrap_w <= 100.0 && wrap_h > 2.0 * one_line,
        "wrap: {wrap_w} x {wrap_h}"
    );
    for key in ["end-text", "middle-text"] {
        let [_, _, width, height] = rect(&runtime, key);
        assert!(width <= 100.0, "{key} overflows: {width}");
        assert_eq!(height, one_line, "{key} stays on one line");
    }
    let clamp = rect(&runtime, "clamp-text");
    assert_eq!(clamp[3], 2.0 * one_line, "clamped to two lines");
    // The whole string stays for a screen reader whatever is cut.
    let all = nodes(&runtime);
    assert!(
        all.iter().filter(|node| node["name"] == LONG).count() >= 4,
        "the full text is in the tree: {all:#?}"
    );
}

fn lines(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .w_full()
            .h(60.0)
            .child(Separator::horizontal().show(hosted).key("across"))
            .child(Separator::horizontal().inset().show(hosted).key("inset"))
            .child(
                row()
                    .h(40.0)
                    .child(Separator::vertical().show(hosted).key("down")),
            ),
        [],
    )
}

#[test]
fn a_separator_is_a_hairline_that_spans_its_group_and_inset_stops_short() {
    let runtime = headless(hosted(), lines, (300.0, 200.0));
    let across = rect(&runtime, "across");
    let inset = rect(&runtime, "inset");
    let down = rect(&runtime, "down");
    assert_eq!(across[3], down[2], "one thickness both ways");
    assert!(across[3] > 0.0 && across[3] <= 2.0);
    assert_eq!(across[2], 300.0, "spans the column");
    assert!(
        inset[2] < across[2] && inset[0] > across[0],
        "inset stops short of each end"
    );
    let (left, right) = (
        inset[0] - across[0],
        (across[0] + across[2]) - (inset[0] + inset[2]),
    );
    assert!(
        left > 0.0 && left == right,
        "inset by the same at both ends: {left} {right}"
    );
    assert_eq!(down[3], 40.0, "a vertical one spans its row");
}

fn caps(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .child(row().child(Kbd::new("Ctrl+Shift+K").show(hosted).key("chord")))
            .child(row().child(Kbd::new("K").show(hosted).key("one"))),
        [],
    )
}

#[test]
fn a_chord_is_one_keycap_per_key() {
    let runtime = headless(hosted(), caps, (400.0, 200.0));
    let chord = rect(&runtime, "chord");
    let one = rect(&runtime, "one");
    assert!(
        chord[2] > 2.0 * one[2],
        "three keys are wider than one: {chord:?} {one:?}"
    );
    assert_eq!(chord[3], one[3]);
}

fn skeletons(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    frame(
        hosted,
        cx,
        col()
            .w(300.0)
            .child(Skeleton::line().show(hosted).key("line"))
            .child(Skeleton::text(3).show(hosted).key("para"))
            .child(Skeleton::block(64.0, 48.0).show(hosted).key("block"))
            .child(Skeleton::control().show(hosted).key("control")),
        [],
    )
}

#[test]
fn a_skeleton_is_the_size_of_what_it_stands_in_for() {
    let runtime = headless(hosted(), skeletons, (400.0, 600.0));
    let line = rect(&runtime, "line");
    let para = rect(&runtime, "para");
    assert_eq!(line[2], 300.0, "a line is as wide as its column");
    assert_eq!(
        para[3],
        3.0 * line[3],
        "each line of a paragraph is a line of text high"
    );
    let block = rect(&runtime, "block");
    assert_eq!((block[2], block[3]), (64.0, 48.0));
    let control = rect(&runtime, "control");
    assert!(
        control[2] > control[3],
        "a control's stand-in is wider than it is high"
    );
}
