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
    "files.rs",
    "follow.rs",
    "font.rs",
    "hover.rs",
    "icon.rs",
    "images.rs",
    "layout.rs",
    "number.rs",
    "placement.rs",
    "post.rs",
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

/// A file's lines that are code: trimmed, numbered from 1, with comments dropped and everything
/// from the first `#[cfg(test)]` on, which is the file's own tests.
fn code_lines(text: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        if !line.starts_with("//") {
            lines.push((i + 1, line.to_owned()));
        }
    }
    lines
}

/// Item 41: no timing and no corner is written into a component. A `Duration` is built from a
/// number only in a named constant (a behaviour's own, such as a typeahead's reset, never a
/// motion: motion is the foundation's), and a corner is the radius token, never a number of
/// points (zero, which is no corner, is the one number allowed).
#[test]
fn timing_and_corners_come_from_tokens() {
    for (file, text) in sources() {
        for (n, line) in code_lines(&text) {
            let builds_a_duration = ["Duration::from_", "Duration::new("].iter().any(|p| {
                line.match_indices(p).any(|(i, _)| {
                    let rest = &line[i + p.len()..];
                    let args = rest.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_');
                    args.strip_prefix('(')
                        .is_some_and(|a| a.starts_with(|c: char| c.is_ascii_digit()))
                        || (p.ends_with('(') && rest.starts_with(|c: char| c.is_ascii_digit()))
                })
            });
            let named = line.starts_with("const ") || line.starts_with("pub const ");
            assert!(
                !builds_a_duration || named,
                "{file}:{n}: a duration outside a named constant: {line}"
            );
            for corner in [".clip_rounded(", ".rounded(", ".radius("] {
                for (at, _) in line.match_indices(corner) {
                    let arg = line[at + corner.len()..]
                        .split([')', ','])
                        .next()
                        .unwrap_or("")
                        .trim();
                    let number = arg.starts_with(|c: char| c.is_ascii_digit());
                    assert!(
                        !number || arg.parse::<f32>().is_ok_and(|v| v == 0.0),
                        "{file}:{n}: a corner that is not the radius token (`{corner}{arg})`): {line}"
                    );
                }
            }
        }
    }
}

/// Item 42: no component keeps theme data of its own. Process-wide state is what is listed below,
/// each named with its file and what it is, so a new one has to be argued for here. State only
/// for tests (a `#[cfg(test)]` module) is not counted.
const PROCESS_STATE: &[(&str, &str, &str)] = &[
    (
        "follow.rs",
        "COMMANDS",
        "the queue of the follower of the theme, for `reload_theme` and `set_colour_options`",
    ),
    (
        "post.rs",
        "POST",
        "the way a job reaches the window's host from another thread, set by `run`",
    ),
    (
        "post.rs",
        "POST_ANY",
        "the same for what does not name the application's state type",
    ),
    ("toast.rs", "NEXT", "the counter that numbers toasts"),
    (
        "icon.rs",
        "DRAWINGS",
        "each icon's parsed drawing, made once from the embedded files",
    ),
];

/// The name a `static` line declares: `static NAME: ...` or `static mut NAME: ...`.
fn static_name(line: &str) -> Option<&str> {
    let after = line.split_once("static ")?.1;
    let after = after.strip_prefix("mut ").unwrap_or(after);
    let name = after.split([':', ' ']).next()?;
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()))
    .then_some(name)
}

#[test]
fn process_wide_state_is_what_is_listed() {
    assert_eq!(static_name("    static NEXT: Cell<u64> = x;"), Some("NEXT"));
    assert_eq!(static_name("pub static mut ID: u8 = 0;"), Some("ID"));
    assert_eq!(static_name("fn f(x: &'static str)"), None);
    for (file, text) in sources() {
        for (n, line) in code_lines(&text) {
            let Some(name) = static_name(&line) else {
                continue;
            };
            assert!(
                PROCESS_STATE
                    .iter()
                    .any(|(f, listed, _)| *f == file && *listed == name),
                "{file}:{n}: process-wide state `{name}` that PROCESS_STATE does not list: {line}"
            );
        }
    }
    // And nothing listed is gone: a stale row is as wrong as a missing one.
    for (file, name, _) in PROCESS_STATE {
        let found = sources().into_iter().any(|(f, text)| {
            f == *file
                && code_lines(&text)
                    .iter()
                    .any(|(_, line)| static_name(line) == Some(name))
        });
        assert!(
            found,
            "{file}: PROCESS_STATE lists `{name}`, which is not there"
        );
    }
}

/// Item 31: time comes from Rune's clock, which a test controls: no component reads the system's.
#[test]
fn time_comes_from_the_hosts_clock() {
    for (file, text) in sources() {
        for (n, line) in code_lines(&text) {
            assert!(
                !line.contains("Instant::now()") && !line.contains("SystemTime::now()"),
                "{file}:{n}: reads the system clock; use the time Rune hands the poll and the handler: {line}"
            );
        }
    }
}
