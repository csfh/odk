//! Checklist items every component shares, checked by reading the crate's source.

use std::fs;
use std::path::Path;

use super::assert_known_gap;

/// Every component: its name, its source file, and whether it takes keyboard input of its
/// own (and so must document it, item 25). A component that only contains buttons, such as
/// an alert with a dismiss button, takes the buttons' keyboard behaviour and is not counted.
pub(super) const COMPONENTS: &[(&str, &str, bool)] = &[
    ("Alert", "alert.rs", false),
    ("Badge", "badge.rs", false),
    ("Breadcrumb", "breadcrumb.rs", true),
    ("Button", "button.rs", true),
    ("Checkbox", "checkbox.rs", true),
    ("Command", "command.rs", true),
    ("ContextMenu", "menu.rs", true),
    ("Dialog", "dialog.rs", true),
    ("DropdownMenu", "menu.rs", true),
    ("FormField", "form_field.rs", false),
    ("Heading", "text.rs", false),
    ("HoverCard", "hover_card.rs", true),
    ("Input", "input.rs", true),
    ("InputGroup", "input_group.rs", true),
    ("Kbd", "kbd.rs", false),
    ("NavList", "nav_list.rs", true),
    ("NumberInput", "number_input.rs", true),
    ("PasswordInput", "password_input.rs", true),
    ("Popover", "popover.rs", true),
    ("Progress", "progress.rs", false),
    ("RadioGroup", "radio.rs", true),
    ("SearchInput", "search_input.rs", true),
    ("Select", "select.rs", true),
    ("Separator", "separator.rs", false),
    ("Sheet", "sheet.rs", true),
    ("Skeleton", "skeleton.rs", false),
    ("Spinner", "spinner.rs", false),
    ("StatusIndicator", "status.rs", false),
    ("Switch", "switch.rs", true),
    ("Tabs", "tabs.rs", true),
    ("Text", "text.rs", false),
    ("Textarea", "textarea.rs", true),
    ("Toast", "toast.rs", false),
    ("ToggleGroup", "toggle_group.rs", true),
    ("Tooltip", "tooltip.rs", false),
];

/// The components on component tokens (`docs/plan.md`, "Design language migration", step 7 on),
/// and the helpers they share: the interaction helper, the label row of `toggle.rs`, the list
/// surface of `menu_list.rs` and the link of `link.rs`. `text.rs` holds `Text` and `Heading`.
/// Every component is on it since ADR 0047; a file that is not listed draws nothing of its own.
const ON_COMPONENT_TOKENS: &[&str] = &[
    "alert.rs",
    "badge.rs",
    "breadcrumb.rs",
    "button.rs",
    "checkbox.rs",
    "command.rs",
    "dialog.rs",
    "form_field.rs",
    "hover_card.rs",
    "input.rs",
    "input_group.rs",
    "interaction.rs",
    "kbd.rs",
    "link.rs",
    "menu.rs",
    "menu_list.rs",
    "nav_list.rs",
    "number_input.rs",
    "overlay.rs",
    "password_input.rs",
    "placement.rs",
    "popover.rs",
    "progress.rs",
    "radio.rs",
    "search_input.rs",
    "select.rs",
    "separator.rs",
    "sheet.rs",
    "skeleton.rs",
    "spinner.rs",
    "status.rs",
    "switch.rs",
    "tabs.rs",
    "text.rs",
    "textarea.rs",
    "toast.rs",
    "toggle.rs",
    "toggle_group.rs",
    "tooltip.rs",
];

/// Each source file of the crate, outside the checklist tests, without its `tests` module.
fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<(String, String)> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| {
            let text = fs::read_to_string(e.path()).unwrap();
            let code = match text.find("#[cfg(test)]\nmod tests") {
                Some(i) => text[..i].to_owned(),
                None => text,
            };
            (e.file_name().to_string_lossy().into_owned(), code)
        })
        .collect();
    files.sort();
    files
}

fn source(file: &str) -> String {
    sources()
        .into_iter()
        .find(|(name, _)| name == file)
        .unwrap_or_else(|| panic!("no {file}"))
        .1
}

/// Which component a file's findings are charged to.
fn components_in(file: &str) -> Vec<&'static str> {
    COMPONENTS
        .iter()
        .filter(|(_, f, _)| *f == file)
        .map(|(name, _, _)| *name)
        .collect()
}

/// Lines of code, without comments, with their line numbers.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.starts_with("//"))
}

/// Every element a crate user can put in a view is in `COMPONENTS`, so the checks below
/// cannot miss one.
#[test]
fn every_component_is_listed() {
    for (file, text) in sources() {
        for line in text.lines() {
            let Some(name) = line.strip_prefix("impl RenderOnce for ") else {
                continue;
            };
            let name = name.trim_end_matches(" {");
            let public = text.contains(&format!("pub struct {name} "))
                || text.contains(&format!("pub struct {name};"));
            if public {
                assert!(
                    COMPONENTS.iter().any(|(n, f, _)| *n == name && *f == file),
                    "{name} in {file} is not in the checklist's COMPONENTS list"
                );
            }
        }
    }
}

/// Item 1: no colour is written into a component. A fully transparent `Color` is allowed.
#[test]
fn no_colour_literals() {
    let forbidden = [
        "rgb(",
        "rgba(",
        "hsl(",
        "hsla(",
        "\"#",
        "Rgba {",
        "Hsla {",
        "::black()",
        "::white()",
        " black()",
        " white()",
        "(black()",
        "(white()",
    ];
    for (file, text) in sources() {
        if file == "theme.rs" {
            continue;
        }
        for (n, line) in code_lines(&text) {
            for pattern in forbidden {
                assert!(
                    !line.contains(pattern),
                    "{file}:{n}: a colour literal (`{pattern}`): {line}"
                );
            }
        }
        for (at, _) in text.match_indices("Color {") {
            let body = &text[at..text.len().min(at + 120)];
            assert!(
                body.contains("a: 0.0"),
                "{file}: a `Color` literal that is not fully transparent"
            );
        }
    }
}

/// Where a line puts a bare number into a size: `px(12.0)` or `token * 14.0`. Zero is not
/// a size, and a named constant is how a number is allowed in.
fn unnamed_number(line: &str) -> bool {
    if line.starts_with("const ") || line.starts_with("pub const ") {
        return false;
    }
    let digit_after = |pattern: &str| {
        line.match_indices(pattern).any(|(i, _)| {
            let rest = &line[i + pattern.len()..];
            rest.starts_with(|c: char| c.is_ascii_digit())
                && !rest.starts_with("0.0)")
                && !rest.starts_with("0)")
        })
    };
    let in_size = line.contains("px(") || line.contains("with_alpha(");
    digit_after("px(") || (in_size && (digit_after("* ") || digit_after("with_alpha(")))
}

/// Item 2: sizes, widths and opacities come from tokens or named constants.
#[test]
fn no_unnamed_numbers_in_styles() {
    for (file, text) in sources() {
        let hits: Vec<String> = code_lines(&text)
            .filter(|(_, l)| unnamed_number(l))
            .map(|(n, l)| format!("{file}:{n}: {l}"))
            .collect();
        if hits.is_empty() {
            continue;
        }
        let owners = components_in(&file);
        assert!(
            !owners.is_empty(),
            "unnamed numbers outside any component:\n{}",
            hits.join("\n")
        );
        for owner in owners {
            assert_known_gap(owner, 2);
        }
    }
}

#[test]
fn the_number_check_finds_what_it_should() {
    assert!(unnamed_number(".w(px(400.0))"));
    assert!(unnamed_number(".max_w(px(t.space.xl * 14.0))"));
    assert!(unnamed_number(".bg(colour(c.with_alpha(0.5)))"));
    assert!(!unnamed_number(".w(px(t.space.xl))"));
    assert!(!unnamed_number("point(px(0.0), px(0.0))"));
    assert!(!unnamed_number("const DEFAULT_EXTENT: f32 = 400.0;"));
    assert!(!unnamed_number(".opacity(amount)"));
}

/// Item 34: a component on component tokens draws only them. It never reads the theme's
/// snapshot or palette, never names a palette colour, computes no colour, and has no round
/// corner of its own.
#[test]
fn components_on_component_tokens_draw_only_them() {
    let forbidden = [
        "theme_snapshot()",
        "palette",
        "Palette",
        ".accent",
        ".red",
        ".yellow",
        ".green",
        ".blue",
        "bright_",
        "with_alpha(",
        ".mix(",
        ".over(",
        "oklch",
        "contrast(",
        "rounded_full",
        "rounded_sm",
        "rounded_md",
        "rounded_lg",
        // The legacy helpers: spacing from the layout enum, and the retired focus and overlay
        // layers.
        "Space::",
        "focus_layer(",
        " layer(",
    ];
    for file in ON_COMPONENT_TOKENS {
        for (n, line) in code_lines(&source(file)) {
            for pattern in forbidden {
                assert!(
                    !draws_with(line, pattern),
                    "{file}:{n}: draws past the component tokens (`{pattern}`): {line}"
                );
            }
        }
    }
}

/// Whether `line` uses `pattern`. A pattern that starts with a name, such as `t.text.`, only
/// counts where that name starts, so `layout.text.` is not the legacy `t.text.`; one that ends
/// with a letter only where the name ends, so `.redo()` is not the palette's `.red`.
fn draws_with(line: &str, pattern: &str) -> bool {
    let name = |c: char| c.is_alphanumeric() || c == '_';
    let starts_a_name = pattern.starts_with(name);
    let ends_a_name = pattern.ends_with(char::is_alphanumeric);
    line.match_indices(pattern).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + pattern.len()..].chars().next();
        !(starts_a_name && before.is_some_and(name) || ends_a_name && after.is_some_and(name))
    })
}

#[test]
fn the_component_token_check_finds_what_it_should() {
    assert!(draws_with("let ink = t.text.primary;", "t.text."));
    assert!(!draws_with(
        "let line = layout.text.line_height();",
        "t.text."
    ));
    assert!(draws_with(".bg(colour(p.red))", ".red"));
    assert!(!draws_with("e.redo();", ".red"));
    assert!(draws_with("p.bright_red", "bright_"));
    let caught = |line: &str| {
        [
            "theme_snapshot()",
            ".accent",
            "rounded_full",
            "with_alpha(",
            "Space::",
            " layer(",
        ]
        .iter()
        .any(|p| line.contains(p))
    };
    assert!(caught("let s = theme_snapshot();"));
    assert!(caught(".gap(Space::Sm)"));
    assert!(caught(
        "let frame = layer(child_id(id, \"layer\"), open, true, window, cx);"
    ));
    assert!(!caught(
        "let frame = layer_over(id, open, true, duration, window, cx);"
    ));
    assert!(caught(".bg(colour(t.accent.base))"));
    assert!(caught(".rounded_full()"));
    assert!(!caught(".control_geometry(&g)"));
    assert!(!caught("let c = components();"));
}

/// Whether the attributes just above line `at` keep it out of a release build: a test, a debug
/// build or the `test-support` feature.
fn gated_by_attribute(lines: &[&str], at: usize) -> bool {
    lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("#[") || l.starts_with("///") || l.is_empty())
        .any(|l| {
            l.starts_with("#[cfg(")
                && (l.contains("test") || l.contains("debug_assertions"))
                && !l.contains("not(")
        })
}

/// Whether line `at` is kept out of a release build, by an attribute of its own or of an item
/// that contains it (found by its indentation).
fn build_gated(lines: &[&str], at: usize) -> bool {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut at = at;
    loop {
        if gated_by_attribute(lines, at) {
            return true;
        }
        let k = indent(lines[at]);
        if k == 0 {
            return false;
        }
        match (0..at).rev().find(|&j| {
            !lines[j].trim().is_empty() && indent(lines[j]) < k && !lines[j].trim().starts_with('#')
        }) {
            Some(j) => at = j,
            None => return false,
        }
    }
}

/// The files `lib.rs` declares for tests or the `test-support` feature only.
fn gated_files() -> Vec<String> {
    let lib = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs")).unwrap();
    let lines: Vec<&str> = lib.lines().collect();
    (0..lines.len())
        .filter(|&i| gated_by_attribute(&lines, i))
        .filter_map(|i| {
            let line = lines[i].trim();
            let name = line
                .strip_prefix("pub mod ")
                .or_else(|| line.strip_prefix("pub(crate) mod "))
                .or_else(|| line.strip_prefix("mod "))?;
            Some(format!("{}.rs", name.trim_end_matches(';')))
        })
        .collect()
}

/// The files that may hold the theme: the store, and the two that draw the window's own canvas.
/// Every other file gets its colours from `components()` and the `StyledExt` helpers.
const MAY_READ_THE_SNAPSHOT: &[&str] = &["app.rs", "layout.rs", "theme.rs"];

/// Item 40: a colour is never worked out where it is drawn. In no file of the crate, outside
/// the store in `theme.rs`, does code name the palette, do colour maths (OKLCH, contrast,
/// mixing, alpha) or read a palette colour by name (item 34 says the same of the component
/// files; this holds the helpers too: `editor.rs`, `layout.rs`, `focus.rs`, ...). Only
/// `MAY_READ_THE_SNAPSHOT` read the snapshot.
#[test]
fn colour_is_resolved_in_the_theme_layers_and_nowhere_else() {
    let forbidden = [
        "Palette",
        "palette",
        "oklch",
        "Oklch",
        "Oklab",
        "contrast(",
        "drawn_contrast",
        ".mix(",
        ".over(",
        "with_alpha(",
        "bright_",
    ];
    for (file, text) in sources() {
        if file == "theme.rs" {
            continue;
        }
        for (n, line) in code_lines(&text) {
            for pattern in forbidden {
                assert!(
                    !draws_with(line, pattern),
                    "{file}:{n}: resolves a colour outside the theme layers (`{pattern}`): {line}"
                );
            }
            assert!(
                !line.contains("theme_snapshot()")
                    || MAY_READ_THE_SNAPSHOT.contains(&file.as_str()),
                "{file}:{n}: reads the theme snapshot; take a component token instead: {line}"
            );
        }
    }
}

/// Item 41: no timing and no corner is written into a component. A `Duration` is built only in
/// a named constant (a behaviour's own, such as `select.rs`'s typeahead reset, never a motion:
/// motion is `foundation().motion`), and no rectangle takes one of GPUI's named corner sizes,
/// whatever they are in pixels.
#[test]
fn timing_and_corners_come_from_tokens() {
    let corners = [
        "rounded_sm(",
        "rounded_md(",
        "rounded_lg(",
        "rounded_xl(",
        "rounded_2xl(",
        "rounded_3xl(",
        "rounded_full(",
        "rounded_none(",
        ".rounded_sm()",
        ".rounded_md()",
        ".rounded_lg()",
        ".rounded_xl()",
        ".rounded_full()",
    ];
    for (file, text) in sources() {
        for (n, line) in code_lines(&text) {
            // A duration worked out from a variable is arithmetic; one written as a number is a
            // timing.
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
            for pattern in corners {
                assert!(
                    !line.contains(pattern),
                    "{file}:{n}: a corner that is not the radius token (`{pattern}`): {line}"
                );
            }
        }
    }
}

/// Item 42: no component keeps theme data of its own. Process-wide state is the theme store in
/// `theme.rs` and the few statics below, each named with its file and what it is, so a new one
/// has to be argued for here. State kept only for tests or debug builds (`#[cfg(test)]` and
/// the like) is not counted.
const PROCESS_STATE: &[(&str, &str, &str)] = &[
    (
        "theme.rs",
        "CURRENT",
        "the theme snapshot and its component tokens",
    ),
    ("theme.rs", "FOUNDATION", "the foundation tokens"),
    (
        "theme.rs",
        "SYSTEM_MONO",
        "the system monospace family, resolved once",
    ),
    ("toast.rs", "LAST_ID", "the counter that numbers toasts"),
    (
        "overlay.rs",
        "INSIDE",
        "the nesting depth of overlays being drawn",
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
fn process_wide_state_is_the_theme_store_and_what_is_listed() {
    let gated = gated_files();
    assert!(gated.contains(&"drawn.rs".to_owned()), "{gated:?}");
    assert_eq!(
        static_name("    static DRAWN: RefCell<u8> = x;"),
        Some("DRAWN")
    );
    assert_eq!(static_name("pub static mut ID: u8 = 0;"), Some("ID"));
    assert_eq!(static_name("fn f(x: &'static str)"), None);
    for (file, text) in sources() {
        if gated.contains(&file) {
            continue;
        }
        let lines: Vec<&str> = text.lines().collect();
        for (i, raw) in lines.iter().enumerate() {
            let line = raw.trim();
            if line.starts_with("//") {
                continue;
            }
            let Some(name) = static_name(line) else {
                continue;
            };
            if build_gated(&lines, i)
                || PROCESS_STATE
                    .iter()
                    .any(|(f, n, _)| *f == file && *n == name)
            {
                continue;
            }
            panic!(
                "{file}:{}: process-wide state `{name}` that PROCESS_STATE does not list: {line}",
                i + 1
            );
        }
    }
}

/// Item 12: focus moves through `move_focus`, which redraws so the ring follows.
#[test]
fn focus_moves_through_move_focus() {
    for (file, text) in sources() {
        if file == "focus.rs" {
            continue;
        }
        for (n, line) in code_lines(&text) {
            assert!(
                !line.contains(".focus(window)"),
                "{file}:{n}: focus moved without a redraw; use move_focus: {line}"
            );
        }
    }
}

/// Item 31: time comes from GPUI's clock, which tests control.
#[test]
fn time_comes_from_gpui() {
    for (file, text) in sources() {
        for (n, line) in code_lines(&text) {
            assert!(
                !line.contains("Instant::now()"),
                "{file}:{n}: reads the system clock; use cx.background_executor().now(): {line}"
            );
        }
    }
}

/// Item 15: nothing reports an empty name, except a separator, which has none by nature.
#[test]
fn no_empty_names() {
    for (file, text) in sources() {
        for (n, line) in code_lines(&text) {
            if line.contains("Accessible::new(") && line.contains(", \"\")") {
                assert!(
                    line.contains("Role::Separator"),
                    "{file}:{n}: an empty accessible name: {line}"
                );
            }
        }
    }
}

/// Item 14: every component describes itself.
#[test]
fn every_component_implements_describe() {
    let all: String = sources().into_iter().map(|(_, t)| t).collect();
    for (name, _, _) in COMPONENTS {
        if !all.contains(&format!("impl Describe for {name} ")) {
            assert_known_gap(name, 14);
        }
    }
}

/// The doc comment directly above `pub struct name`.
fn doc_comment(text: &str, name: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines
        .iter()
        .position(|l| l.starts_with(&format!("pub struct {name} ")))
    else {
        return String::new();
    };
    let mut doc = Vec::new();
    for line in lines[..at].iter().rev() {
        let line = line.trim();
        if let Some(text) = line.strip_prefix("///") {
            doc.push(text);
        } else if !line.starts_with("#[") {
            break;
        }
    }
    doc.reverse();
    doc.join(" ")
}

/// Item 25: a component that takes keyboard input says how in its doc comment.
#[test]
fn keyboard_behaviour_is_documented() {
    let keys = ["Tab", "Enter", "Space", "Escape", "arrow", "key"];
    for (name, file, interactive) in COMPONENTS {
        if !interactive {
            continue;
        }
        let doc = doc_comment(&source(file), name);
        if !keys.iter().any(|k| doc.contains(k)) {
            assert_known_gap(name, 25);
        }
    }
}

/// Item 26: every component is in the prelude, so `use odk::prelude::*` reaches it.
#[test]
fn every_component_is_in_the_prelude() {
    #[allow(unused_imports)]
    use crate::prelude::*;
    let _ = (
        Alert::new("a", "t"),
        Badge::new("3"),
        Breadcrumb::new("b"),
        Button::new("c"),
        Checkbox::new("cb", "Wrap lines"),
        Command::new("cmd", "Commands").item(CommandItem::new("Open")),
        ContextMenu::new("d", "Row actions", Text::new("t")),
        Dialog::new("dl", "Rename file"),
        DropdownMenu::new("e", "l"),
        FormField::new("ff", "Name").child(Input::new("ffi", "Name")),
        Heading::new("h"),
        HoverCard::new("f", "Details", Text::new("t")),
        Input::new("in", "Name"),
        InputGroup::new("ig", "Website"),
        Kbd::new("K"),
        NavList::new("nl", "Settings"),
        NumberInput::new("ni", "Quantity"),
        PasswordInput::new("pw", "Password"),
        Progress::new("pr", "Uploading"),
        RadioGroup::new("rg", "Theme"),
        SearchInput::new("si", "Search"),
        Select::new("sl", "Country"),
        Separator::horizontal(),
        Sheet::new("g", "t"),
        Skeleton::line(),
        Spinner::new("s"),
        StatusIndicator::new(Status::Active, "Online"),
        Switch::new("sw", "Wi-Fi"),
        Tabs::new("t", "Sections"),
        Text::new("t"),
        Textarea::new("ta", "Notes"),
        Toast::new("t"),
        ToggleGroup::new("u", "Options"),
        Tooltip::new("tt", "Save the file", Button::new("save")),
    );
    let prelude = source("lib.rs");
    let prelude = &prelude[prelude.find("pub mod prelude").unwrap()..];
    for (name, _, _) in COMPONENTS {
        assert!(prelude.contains(name), "{name} is not in the prelude");
    }
}

/// Item 35: nothing asks for a frame but `interaction::request_frame`, which counts them, and
/// nothing animates with GPUI's `with_animation`, which asks for one at every frame for as long as
/// it runs. A component that moves while a value changes uses `tween` or `progress`; one that
/// changes on a schedule wakes its view with a timer (`Spinner`). The performance tests count the
/// requests, which only works while every request is counted.
#[test]
fn only_the_counted_wrapper_asks_for_frames() {
    let forbidden = ["request_animation_frame", "with_animation", "AnimationExt"];
    for (file, code) in sources() {
        if file == "interaction.rs" {
            continue;
        }
        // Comments may name them; code may not.
        for (n, line) in code.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for word in forbidden {
                assert!(
                    !code.contains(word),
                    "{file}:{}: {word} goes around `interaction::request_frame`",
                    n + 1
                );
            }
        }
    }
}
