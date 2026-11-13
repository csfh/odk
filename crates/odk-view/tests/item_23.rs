//! Checklist item 23 for the controls with a label beside them or in a segment: an Arabic label,
//! and one that mixes Arabic with Latin and digits, sits on the control's row, beside the box or
//! inside its segment, never below it or cut by the layout around it, and the accessibility tree
//! has the whole of it. Seven options in a 260-point column stay inside it and each is reached by
//! the arrow keys. Nothing here opens a window.
//!
//! The text is shaped by Rune's Parley with HarfRust, which joins Arabic letters, orders a
//! right-to-left run and takes the paragraph's direction from its text; the glyphs come from Rune's
//! script fallback (Noto Sans Arabic on Omarchy), so the widths here are the Arabic font's.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{Checkbox, Face, Hosted, Switch, ToggleGroup, frame, headless};
use rune::{Cx, El, Key, KeyEvent, Runtime, col};

const ARABIC: &str = "حفظ جميع التغييرات في المستند قبل إغلاق النافذة والانتقال إلى الصفحة التالية";
const MIXED: &str = "Open ملف التقرير السنوي 2026 in Notes";
const SHORT_ARABIC: &str = "حفظ";

/// The seven options of a week, in Arabic: a segmented control with more options than its room.
const DAYS: [&str; 7] = [
    "الأحد",
    "الاثنين",
    "الثلاثاء",
    "الأربعاء",
    "الخميس",
    "الجمعة",
    "السبت",
];

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

/// Whether this machine has a font Rune's Arabic fallback finds. Without one the glyphs are
/// missing-glyph boxes and the widths say nothing, so the tests say so and stop.
fn arabic_font() -> bool {
    let output = std::process::Command::new("fc-list")
        .args([":lang=ar", "file"])
        .output()
        .expect("fc-list");
    let found = !output.stdout.is_empty();
    if !found {
        eprintln!("skipped: no font with Arabic is installed");
    }
    found
}

type Rt = Runtime<Hosted<()>>;

thread_local! {
    /// The column's width for the page being built.
    static WIDTH: std::cell::Cell<f32> = const { std::cell::Cell::new(560.0) };
}

/// A page of the three controls in a column [`WIDTH`] points wide, at a 600-point window.
fn page(hosted: &Hosted<()>, cx: &mut Cx<Hosted<()>>) -> El {
    let width = WIDTH.with(|w| w.get());
    {
        let days = DAYS
            .iter()
            .enumerate()
            .fold(ToggleGroup::<()>::new("days", "Day"), |group, (n, day)| {
                group.item(format!("d{n}"), *day)
            })
            .default_value(["d0"]);
        let segments = ToggleGroup::<()>::new("files", "File")
            .item("arabic", ARABIC)
            .item("mixed", MIXED)
            .item("short", SHORT_ARABIC);
        frame(
            hosted,
            cx,
            col()
                .key("column")
                .w(width)
                .gap(12.0)
                .child(Checkbox::new("ar-check", ARABIC).show(hosted))
                .child(Checkbox::new("mixed-check", MIXED).show(hosted))
                .child(Switch::new("ar-switch", ARABIC).show(hosted))
                .child(Switch::new("mixed-switch", MIXED).show(hosted))
                .child(segments.show(hosted))
                .child(days.show(hosted)),
            vec![],
        )
    }
}

fn open(width: f32) -> Rt {
    WIDTH.with(|w| w.set(width));
    let mut runtime = headless(
        Hosted::new((), ThemeSnapshot::emergency(), face()),
        page,
        (600.0, 800.0),
    );
    settle(&mut runtime);
    runtime
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

/// The drawn text that is `text`, inside the element `within`.
fn text_in(runtime: &Rt, within: &str, text: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some(text) && e.path.split('/').any(|part| part == within))
        .unwrap_or_else(|| panic!("no text {text:?} in {within}"))
        .rect
}

/// Every node of the accessibility tree as its role and its name.
fn names(runtime: &Rt) -> Vec<(String, String)> {
    fn walk(node: &serde_json::Value, into: &mut Vec<(String, String)>) {
        if let (Some(role), Some(name)) = (node["role"].as_str(), node["name"].as_str()) {
            into.push((role.to_string(), name.to_string()));
        }
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, into);
        }
    }
    let mut all = Vec::new();
    walk(&runtime.accessibility_json(), &mut all);
    all
}

fn inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.5
        && inner[1] >= outer[1] - 0.5
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.5
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.5
}

#[test]
fn an_arabic_label_sits_beside_its_box_on_its_row_and_wraps_inside_the_column() {
    if !arabic_font() {
        return;
    }
    // One line of the mixed label, in a column wide enough for it.
    let one_line = text_in(&open(560.0), "mixed-check", MIXED)[3];
    for width in [560.0, 240.0] {
        let runtime = open(width);
        let column = rect(&runtime, "column");
        for (key, label) in [
            ("ar-check", ARABIC),
            ("mixed-check", MIXED),
            ("ar-switch", ARABIC),
            ("mixed-switch", MIXED),
        ] {
            let control = rect(&runtime, key);
            let text = text_in(&runtime, key, label);
            assert!(
                inside(control, column),
                "{key} at {width}: {control:?} leaves the column {column:?}"
            );
            assert!(
                inside(text, control),
                "{key} at {width}: its label {text:?} is drawn outside it {control:?}"
            );
            // Beside the box: the label starts at the row's top, past the box, not below it.
            assert!(
                (text[1] - control[1]).abs() < 4.0,
                "{key} at {width}: the label starts below the box, {text:?} in {control:?}"
            );
            assert!(
                text[0] > control[0] + 12.0,
                "{key} at {width}: the label is not beside the box, {text:?} in {control:?}"
            );
            assert!(
                text[2] > 20.0,
                "{key} at {width}: the label has no width, so its glyphs were not found"
            );
        }
        // Wide enough, the long label is one line; in a narrow column it wraps.
        let long = text_in(&runtime, "ar-check", ARABIC)[3];
        if width < 300.0 {
            assert!(
                long > one_line * 1.5,
                "the long label did not wrap at {width}"
            );
        } else {
            assert!(long < one_line * 1.5, "the long label wrapped at {width}");
        }
    }
}

#[test]
fn the_accessibility_tree_has_each_whole_label() {
    if !arabic_font() {
        return;
    }
    let runtime = open(240.0);
    let names = names(&runtime);
    let named = |role: &str, label: &str| {
        assert!(
            names.iter().any(|(r, n)| r == role && n == label),
            "no {role} is named {label:?}: {names:?}"
        );
    };
    for label in [ARABIC, MIXED] {
        named("check-box", label);
        named("switch", label);
    }
    // An option is a button in its group, whatever role the group has (`docs/checks.md`).
    for label in [ARABIC, MIXED, SHORT_ARABIC].into_iter().chain(DAYS) {
        assert!(
            names.iter().any(|(r, n)| n == label && r == "button"),
            "no option is named {label:?}: {names:?}"
        );
    }
}

#[test]
fn a_long_arabic_segment_is_cut_inside_its_segment_on_one_line() {
    if !arabic_font() {
        return;
    }
    let runtime = open(560.0);
    let group = rect(&runtime, "files");
    let column = rect(&runtime, "column");
    assert!(inside(group, column), "{group:?} leaves {column:?}");
    let short = rect(&runtime, "files-short");
    for value in ["arabic", "mixed", "short"] {
        let segment = rect(&runtime, &format!("files-{value}"));
        assert!(inside(segment, group), "files-{value} leaves the group");
        assert_eq!(segment[3], short[3], "files-{value} is not one line high");
    }
}

#[test]
fn seven_options_in_a_260_point_column_stay_inside_it_and_the_arrows_reach_each() {
    if !arabic_font() {
        return;
    }
    let mut runtime = open(260.0);
    let column = rect(&runtime, "column");
    let group = rect(&runtime, "days");
    assert!(
        inside(group, column),
        "the group {group:?} leaves {column:?}"
    );
    let segments: Vec<[f32; 4]> = (0..DAYS.len())
        .map(|n| rect(&runtime, &format!("days-d{n}")))
        .collect();
    for (n, segment) in segments.iter().enumerate() {
        assert!(
            inside(*segment, group),
            "days-d{n} {segment:?} leaves {group:?}"
        );
        assert!(segment[2] > 0.0, "days-d{n} has no width");
    }
    for pair in segments.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(
            a[0] + a[2] <= b[0] + 0.5 || b[1] >= a[1] + a[3] - 0.5,
            "segments overlap: {a:?} and {b:?}"
        );
    }

    // The group is one tab stop: Tab reaches the chosen day, and the arrows the rest.
    let mut seen = Vec::new();
    for _ in 0..12 {
        runtime.key(KeyEvent::from(Key::Tab));
        settle(&mut runtime);
        let focused = runtime
            .elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key);
        if focused.as_deref() == Some("days-d0") {
            break;
        }
    }
    for _ in 0..DAYS.len() {
        let focused = runtime
            .elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
            .expect("something has the focus");
        seen.push(focused);
        runtime.key(KeyEvent::from(Key::Right));
        settle(&mut runtime);
    }
    let expected: Vec<String> = (0..DAYS.len()).map(|n| format!("days-d{n}")).collect();
    assert_eq!(seen, expected);
}
