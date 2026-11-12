//! The checklist's rules that hold for every component at once, run over the sample registry
//! (`tests/common`): idle (item 35), worst-case content (23), the text size (37), dark and light
//! palettes (4) and accessible names (15). Nothing here opens a window.

mod common;

use common::{CONTENT, SAMPLES, open, open_with, settle};
use odk_theme::ThemeSnapshot;

const SIZE: (f32, f32) = (480.0, 360.0);

/// A palette of `mode`, `background` and `foreground`, and the text size `base` if given.
fn theme(mode: &str, background: &str, foreground: &str, base: Option<u32>) -> ThemeSnapshot {
    let colors = format!(
        r##"
mode = "{mode}"
accent = "#3b82f6"
muted = "#8a8a8a"
background = "{background}"
dark_background = "#101010"
lighter_background = "#e8e8e8"
foreground = "{foreground}"
red = "#dc2626"
yellow = "#ca8a04"
green = "#16a34a"
blue = "#2563eb"
"##
    );
    let shell = base.map(|b| format!("[font]\nbase-size = {b}\n"));
    ThemeSnapshot::from_toml(&colors, shell.as_deref()).expect("a theme")
}

fn dark() -> ThemeSnapshot {
    theme("dark", "#1c1c1c", "#d8d8d8", None)
}

fn light() -> ThemeSnapshot {
    theme("light", "#f6f6f6", "#202020", None)
}

/// Item 35: a window that is idle asks for no wake-up. The components that move are the ones
/// that say so here, each with what it is waiting for.
const MAY_WAKE: &[(&str, &str)] = &[
    ("Spinner", "wakes for its next glyph, not every frame"),
    ("Skeleton", "none; listed only if it ever animates"),
];

#[test]
fn an_idle_window_of_every_component_asks_for_no_wake_up() {
    let mut woke = Vec::new();
    for sample in SAMPLES {
        let mut runtime = open(sample, "Save", SIZE, None);
        for _ in 0..120 {
            runtime.frame(1.0 / 60.0);
        }
        if runtime.wake_deadline().is_some() && !MAY_WAKE.iter().any(|(n, _)| *n == sample.name) {
            woke.push(sample.name);
        }
    }
    assert!(
        woke.is_empty(),
        "an idle window that asks to be woken: {woke:?}"
    );
}

/// Item 23: a long, unbroken, non-Latin or emoji string is cut or wrapped inside its component,
/// never drawn past the window's edge.
const MAY_SCROLL_ACROSS: &[(&str, &str)] = &[
    ("Tabs", "a scrolling list of tabs"),
    (
        "DataTable",
        "a table of rows that scroll, and columns that share the width",
    ),
    ("Chart", "its box is the window's"),
];

#[test]
fn worst_case_content_stays_inside_the_window() {
    let mut out = Vec::new();
    for sample in SAMPLES {
        if MAY_SCROLL_ACROSS.iter().any(|(n, _)| *n == sample.name) {
            continue;
        }
        for content in CONTENT {
            let runtime = open(sample, content.text, SIZE, None);
            for element in runtime.elements() {
                let [x, _, w, _] = element.rect;
                if element.exiting || !x.is_finite() || !w.is_finite() {
                    continue;
                }
                if x + w > SIZE.0 + 1.0 {
                    out.push(format!(
                        "{} with {}: {:?} ends at {:.0}",
                        sample.name,
                        content.name,
                        element.key,
                        x + w
                    ));
                    break;
                }
            }
        }
    }
    assert!(
        out.is_empty(),
        "past the window's edge:\n{}",
        out.join("\n")
    );
}

/// How tall the sample's own box is: the page's content, not the window around it.
fn extent(runtime: &rune::Runtime<odk_view::Hosted<()>>) -> f32 {
    runtime
        .elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("sample-body"))
        .map_or(0.0, |e| e.rect[3])
}

/// Components whose size is the application's to give, so the text size does not change it.
const SIZED_BY_THE_APPLICATION: &[(&str, &str)] = &[
    (
        "ContextMenu",
        "the area the application wraps is the size it set",
    ),
    ("Image", "an image is the size the application gives it"),
];

/// Item 37: the text size, from 9 to 20 pixels, scales every component: at 20 px a page is taller
/// than at 9 px.
#[test]
fn the_text_size_scales_every_component() {
    let small = theme("dark", "#1c1c1c", "#d8d8d8", Some(9));
    let large = theme("dark", "#1c1c1c", "#d8d8d8", Some(20));
    let mut out = Vec::new();
    for sample in SAMPLES {
        if SIZED_BY_THE_APPLICATION
            .iter()
            .any(|(n, _)| *n == sample.name)
        {
            continue;
        }
        let a = extent(&open_with(
            sample,
            "Save",
            (800.0, 800.0),
            small.clone(),
            None,
        ));
        let b = extent(&open_with(
            sample,
            "Save",
            (800.0, 800.0),
            large.clone(),
            None,
        ));
        if b <= a + 0.5 {
            out.push(format!("{}: {a:.0} at 9 px, {b:.0} at 20 px", sample.name));
        }
    }
    assert!(
        out.is_empty(),
        "no taller at a bigger text size:\n{}",
        out.join("\n")
    );
}

/// Item 4: every component draws under a dark and a light palette.
#[test]
fn every_component_draws_on_a_dark_and_a_light_palette() {
    for sample in SAMPLES {
        for palette in [dark(), light()] {
            let mut runtime = open_with(sample, "Save", SIZE, palette, None);
            settle(&mut runtime);
            assert!(
                runtime
                    .elements()
                    .iter()
                    .any(|e| e.key.as_deref() == Some("odk-root")),
                "{} drew nothing",
                sample.name
            );
        }
    }
}

/// The roles a person reaches and a screen reader names: one with no name is a control nobody can
/// tell from the next.
const NAMED_ROLES: &[&str] = &[
    "button",
    "check-box",
    "switch",
    "radio-button",
    "text-input",
    "password-input",
    "search-input",
    "tab",
    "link",
    "menu-item",
    "menu-item-checkbox",
    "menu-item-radio",
    "list-box-option",
    "combo-box",
];

fn nodes(runtime: &rune::Runtime<odk_view::Hosted<()>>) -> Vec<serde_json::Value> {
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

/// Item 15: a control's accessible name is not empty.
#[test]
fn no_control_has_an_empty_accessible_name() {
    let mut out = Vec::new();
    for sample in SAMPLES {
        let runtime = open(sample, "Save", SIZE, None);
        for node in nodes(&runtime) {
            let role = node["role"].as_str().unwrap_or_default();
            let name = node["name"].as_str().unwrap_or_default();
            if NAMED_ROLES.contains(&role) && name.trim().is_empty() {
                out.push(format!(
                    "{}: a {role} with no name ({})",
                    sample.name, node["key"]
                ));
            }
        }
    }
    assert!(out.is_empty(), "controls with no name:\n{}", out.join("\n"));
}
