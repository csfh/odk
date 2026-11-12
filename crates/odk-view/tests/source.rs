//! The checklist's source rules, for the components on the Rune host (`docs/spec/component-
//! checklist.md`, items 1, 2, 14 and 34). `odk-ui`'s `checklist/source.rs` reads only that crate,
//! so these keep a literal out of `odk-view` while the port is under way. When GPUI leaves, this
//! is where those rules live.

use std::fs;
use std::path::Path;

/// Files that are the host and not a component: they own the conversion to Rune's colour and
/// the theme's tokens, so they are the one place a colour or a number may be built.
const HOST: &[&str] = &[
    "describe.rs",
    "face.rs",
    "host.rs",
    "ink.rs",
    "lib.rs",
    "store.rs",
    "style.rs",
];

/// Files that are not controls: a mark, the layout groups, the row a checkbox and a switch
/// share, the decoded images a window holds, the rules of a number field and the thread that
/// follows the theme are decoration,
/// structure, state or logic, so no
/// `Describe` (item 14), though every other rule here still reads them.
const NOT_CONTROLS: &[&str] = &[
    "follow.rs",
    "font.rs",
    "hover.rs",
    "icon.rs",
    "images.rs",
    "layout.rs",
    "number.rs",
    "placement.rs",
    "toggle.rs",
    "tone.rs",
];

/// The files whose component draws its description through `toggle::labelled`.
const VIA_TOGGLE: &[&str] = &["checkbox.rs", "switch.rs"];

/// The files whose component hands its description to the control it holds, which draws it with
/// the field's state (`FieldControl::in_field`).
const VIA_CONTROL: &[&str] = &["form_field.rs"];

/// Fields that are an `Input` with parts: the `Input` inside draws the description.
const VIA_INPUT: &[&str] = &[
    "search_input.rs",
    "password_input.rs",
    "number_input.rs",
    "input_group.rs",
    "textarea.rs",
];

fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found: Vec<(String, String)> = fs::read_dir(&dir)
        .expect("odk-view/src")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).expect("a source file");
            (name, text)
        })
        .collect();
    found.sort();
    found
}

fn components() -> Vec<(String, String)> {
    sources()
        .into_iter()
        .filter(|(name, _)| !HOST.contains(&name.as_str()))
        .collect()
}

/// A file's code on one line: no comments, no `const` items, runs of space made one.
fn code(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.starts_with("//")
                && !line.starts_with("const ")
                && !line.starts_with("pub const ")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The builders that take a size in points. A bare number in one is a size nobody named.
const SIZED: &[&str] = &[
    ".w(",
    ".h(",
    ".px(",
    ".py(",
    ".gap(",
    ".padding(",
    ".radius(",
    ".min_w(",
    ".min_h(",
    ".max_w(",
    ".max_h(",
    ".size(",
];

/// The bare numbers `code` puts into a size, and a bare final argument of `.anchored(`.
fn unnamed_numbers(code: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for builder in SIZED {
        for (at, _) in code.match_indices(builder) {
            let rest = &code[at + builder.len()..];
            let arg = rest.split([')', ',']).next().unwrap_or("").trim();
            let bare = arg.starts_with(|c: char| c.is_ascii_digit());
            if bare && arg.parse::<f32>().is_ok_and(|n| n != 0.0) {
                hits.push(format!("{builder}{arg})"));
            }
        }
    }
    for (at, _) in code.match_indices(".anchored(") {
        let call = &code[at..];
        let call = &call[..call.find(')').unwrap_or(call.len())];
        let last = call.rsplit(',').next().unwrap_or("").trim();
        if last.parse::<f32>().is_ok_and(|n| n != 0.0) {
            hits.push(format!(".anchored(.., {last})"));
        }
    }
    hits
}

/// Item 1: no colour is written into a component. `ink.rs` converts the theme's colours.
#[test]
fn no_colour_literals() {
    let forbidden = [
        "rgb(",
        "rgba(",
        "hsl(",
        "hsla(",
        "\"#",
        // Rune's own colour constructors (`rune::color`, `rune::Color`).
        "hex(",
        "oklch(",
        "oklcha(",
        "from_oklab(",
        "Color {",
        "Color::BLACK",
        "Color::WHITE",
        "Color::TRANSPARENT",
    ];
    for (file, text) in components() {
        let code = code(&text);
        for pattern in forbidden {
            assert!(
                !code.contains(pattern),
                "{file}: a colour literal `{pattern}`"
            );
        }
    }
}

/// Item 2: sizes, widths and gaps come from tokens or from a named constant.
#[test]
fn no_unnamed_numbers_in_styles() {
    for (file, text) in components() {
        let hits = unnamed_numbers(&code(&text));
        assert!(
            hits.is_empty(),
            "{file}: unnamed numbers: {}",
            hits.join(", ")
        );
    }
}

#[test]
fn the_number_check_finds_what_it_should() {
    assert_eq!(unnamed_numbers(".w(240.0)"), [".w(240.0)"]);
    assert_eq!(unnamed_numbers("x.h(12).px(8.0)"), [".h(12)", ".px(8.0)"]);
    assert_eq!(
        unnamed_numbers(".anchored(key, Side::Bottom, Align::Start, 4.0)"),
        [".anchored(.., 4.0)"]
    );
    assert!(unnamed_numbers(".w(width).h(field.geometry.height)").is_empty());
    assert!(unnamed_numbers(".outline(width, 0.0, ink(c)).px(0.0)").is_empty());
    assert!(unnamed_numbers(".anchored(key, Side::Bottom, Align::Start, LIST_GAP)").is_empty());
}

/// Item 34: a component draws the component layer's tokens and reads nothing else of the theme.
#[test]
fn components_draw_only_component_tokens() {
    let forbidden = [
        "theme_snapshot()",
        "palette",
        "Palette",
        ".accent",
        ".semantic",
        "with_alpha(",
        ".alpha(",
        ".mix(",
        ".over(",
        "oklch",
        "contrast(",
    ];
    for (file, text) in components() {
        // The one read of the theme: `style::paint` turns it into tokens and a text scale.
        let code = code(&text).replace("style::paint(hosted.theme())", "");
        assert!(
            !code.contains(".theme()"),
            "{file}: reads the theme outside `style::paint`"
        );
        for pattern in forbidden {
            assert!(
                !code.contains(pattern),
                "{file}: reads the theme beside its component tokens (`{pattern}`)"
            );
        }
    }
}

/// Item 14: a component describes itself with `Describe` and draws that description.
#[test]
fn every_component_implements_describe_and_draws_it() {
    for (file, text) in components() {
        if NOT_CONTROLS.contains(&file.as_str()) {
            continue;
        }
        assert!(
            text.contains("Describe for "),
            "{file} does not implement Describe"
        );
        // A checkbox and a switch hand their description to `toggle::labelled`, which applies it.
        let via_toggle = VIA_TOGGLE.contains(&file.as_str()) && text.contains("labelled(");
        // A form field's control draws its description, with the field's state applied.
        let via_control = VIA_CONTROL.contains(&file.as_str()) && text.contains("in_field(");
        // A search or password field is built on `Input`, which draws the same description.
        let via_input = VIA_INPUT.contains(&file.as_str()) && text.contains("Input::new(");
        assert!(
            text.contains("apply(") || via_toggle || via_control || via_input,
            "{file} does not draw its description with describe::apply"
        );
        let code = code(&text);
        assert!(
            !code.contains(".role(\""),
            "{file}: a role string that Describe should have given"
        );
    }
}
