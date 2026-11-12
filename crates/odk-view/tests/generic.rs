//! The checklist's rules that hold for every component at once, run over the sample registry
//! (`tests/common`): idle (item 35), worst-case content (23), the text size (37), dark and light
//! palettes (4) and accessible names (15). Nothing here opens a window.

mod common;

use common::{CONTENT, SAMPLES, first_frame, open, open_with, settle};
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

/// Where every element is, by its place in the tree.
fn layout(runtime: &rune::Runtime<odk_view::Hosted<()>>) -> Vec<(String, [f32; 4])> {
    runtime
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .map(|e| (format!("{} {:?}", e.path, e.key), e.rect))
        .collect()
}

/// What moved between two layouts, as lines to read: the elements that are in both and are not
/// where they were. A part that appears or goes (a check mark, a list that closes) moves nothing,
/// and is not counted here.
fn moved(
    component: &str,
    before: &[(String, [f32; 4])],
    after: &[(String, [f32; 4])],
) -> Vec<String> {
    let after: std::collections::HashMap<_, _> = after.iter().map(|(n, r)| (n, r)).collect();
    let mut out = Vec::new();
    for (name, a) in before {
        let Some(b) = after.get(name) else { continue };
        if a.iter().zip(b.iter()).any(|(x, y)| (x - y).abs() > 0.01)
            && !MAY_MOVE_PART
                .iter()
                .any(|(c, part, _)| *c == component && name.contains(part))
        {
            out.push(format!("{name}: {a:?} -> {b:?}"));
        }
    }
    out
}

/// The parts that are the state, and so move when it changes: a switch's thumb, the tab
/// indicator that follows the selected tab, and a resize handle's line, which is thicker while
/// it is held. Each is only itself: nothing around it moves.
const MAY_MOVE_PART: &[(&str, &str, &str)] = &[
    (
        "Switch",
        "-thumb",
        "the thumb is where the switch is on or off",
    ),
    (
        "Tabs",
        "sample-tabs-one/#1",
        "the indicator under the selected tab",
    ),
    ("Resizable", "handle-0/#0", "the line is thicker while held"),
];

/// Components where a hover, a press or a focus is meant to show something that was not there
/// (the tooltip and the card), or to move a part (a drag handle), each with what it does.
const MAY_MOVE: &[(&str, &str)] = &[
    ("Tooltip", "a rest of the pointer shows it"),
    ("HoverCard", "a rest of the pointer shows it"),
];

/// Item 32: no state change moves anything. Hovering, pressing and focusing each part of every
/// component, one after another, keeps every element where it was.
#[test]
fn hovering_pressing_and_focusing_move_nothing() {
    let mut out = Vec::new();
    for sample in SAMPLES {
        if !sample.interactive || MAY_MOVE.iter().any(|(n, _)| *n == sample.name) {
            continue;
        }
        let mut runtime = open(sample, "Save", SIZE, None);
        let at_rest = layout(&runtime);
        let targets: Vec<_> = runtime
            .elements()
            .into_iter()
            .filter(|e| e.focusable && !e.inert && !e.exiting)
            .filter_map(|e| e.key)
            .collect();
        for key in targets {
            let Some((x, y)) = runtime.find(&key) else {
                continue;
            };
            // Hover.
            runtime.pointer_move(x, y);
            settle(&mut runtime);
            for line in moved(sample.name, &at_rest, &layout(&runtime)) {
                out.push(format!("{} hover {key}: {line}", sample.name));
            }
            // Press, held, and then let go where it was pressed, which presses the part.
            runtime.pointer_down(x, y);
            settle(&mut runtime);
            for line in moved(sample.name, &at_rest, &layout(&runtime)) {
                out.push(format!("{} press {key}: {line}", sample.name));
            }
            runtime.pointer_up(x, y);
            settle(&mut runtime);
            for line in moved(sample.name, &at_rest, &layout(&runtime)) {
                out.push(format!("{} choose {key}: {line}", sample.name));
            }
        }
        // Focus: Tab through every stop.
        for _ in 0..12 {
            runtime.key(rune::KeyEvent::from(rune::Key::Tab));
            settle(&mut runtime);
            let focused = runtime
                .elements()
                .into_iter()
                .find(|e| e.focused)
                .and_then(|e| e.key)
                .unwrap_or_default();
            for line in moved(sample.name, &at_rest, &layout(&runtime)) {
                out.push(format!("{} focus {focused}: {line}", sample.name));
            }
        }
    }
    assert!(
        out.is_empty(),
        "state changes that moved something:\n{}",
        out.join("\n")
    );
}

/// The rings Rune drew in the last frame: instances with no fill and a border.
fn rings(runtime: &rune::Runtime<odk_view::Hosted<()>>) -> usize {
    runtime
        .instances
        .iter()
        .filter(|i| i.fill[3] == 0.0 && i.border[3] > 0.0 && i.params[0] > 0.0)
        .count()
}

/// Parts whose focus Rune shows another way, each with how: a text field by its caret, an item of
/// a menu or a list by its highlight.
const FOCUS_SHOWN_OTHERWISE: &[(&str, &str)] = &[];

/// Item 11: the keyboard's focus shows a ring on the part that has it. Tab visits each stop of
/// every interactive component, and each time one ring is drawn. (Its width, offset, colour and
/// radius are the component tokens', set in one place: `style::focus_ring`, whose unit test
/// measures a ring.)
#[test]
fn the_keyboards_focus_shows_a_ring_on_the_part_that_has_it() {
    let mut out = Vec::new();
    let mut stops = 0;
    for sample in SAMPLES {
        if !sample.interactive {
            continue;
        }
        let mut runtime = open(sample, "Save", SIZE, None);
        for _ in 0..12 {
            runtime.key(rune::KeyEvent::from(rune::Key::Tab));
            settle(&mut runtime);
            let Some(focused) = runtime.elements().into_iter().find(|e| e.focused) else {
                continue;
            };
            stops += 1;
            let key = focused.key.clone().unwrap_or_default();
            if FOCUS_SHOWN_OTHERWISE
                .iter()
                .any(|(c, k)| *c == sample.name && key.contains(k))
            {
                continue;
            }
            let drawn = rings(&runtime);
            let line = format!("{}: {key} has focus and no ring", sample.name);
            if drawn == 0 && !out.contains(&line) {
                out.push(line);
            }
        }
    }
    assert!(stops > 40, "only {stops} stops were visited");
    assert!(out.is_empty(), "{}", out.join("\n"));
}

/// What the first frame of a window busy with one component may take: a quarter of a 60 Hz
/// frame. `DRAW_BUDGET_MS` replaces it on a machine that is slower or busy, as for `odk-ui`.
fn budget() -> std::time::Duration {
    std::env::var("DRAW_BUDGET_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(
            std::time::Duration::from_millis(4),
            std::time::Duration::from_millis,
        )
}

/// Item 36: a window with a component's sample in it is built and laid out from scratch within
/// the budget. (Rune's headless runtime has no GPU: this is the cost of the tree, not of the
/// pixels, which `tools/idle-cpu` and a real frame measure.)
#[test]
fn a_window_busy_with_each_component_is_built_within_the_budget() {
    let mut over = Vec::new();
    for sample in SAMPLES {
        let least = first_frame(sample, "Save", SIZE, 5);
        if least > budget() {
            over.push(format!("{} ({least:?})", sample.name));
        }
    }
    assert!(
        over.is_empty(),
        "over the budget of {:?}: {}",
        budget(),
        over.join(", ")
    );
}

fn thousand(hosted: &odk_view::Hosted<()>, cx: &mut rune::Cx<odk_view::Hosted<()>>) -> rune::El {
    let mut list = rune::col()
        .key("thousand")
        .gap(4.0)
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        });
    for n in 0..1000 {
        list = list.child(
            odk_view::Button::new(format!("b{n}"), format!("Button {n}"), |_: &mut ()| {})
                .show(hosted),
        );
    }
    odk_view::frame(hosted, cx, list, [])
}

/// Item 36: a thousand controls in one window are built and laid out within a ceiling of six
/// frames. No page has that many; it keeps a big window from running away.
#[test]
fn a_thousand_controls_stay_within_the_ceiling() {
    let least = (0..3)
        .map(|_| {
            let hosted = odk_view::Hosted::new((), ThemeSnapshot::emergency(), common::face());
            let started = std::time::Instant::now();
            let runtime = odk_view::headless(hosted, thousand, SIZE);
            let took = started.elapsed();
            drop(runtime);
            took
        })
        .min()
        .expect("a try");
    eprintln!("a thousand buttons: {least:?}");
    assert!(
        least <= std::time::Duration::from_millis(100).max(budget() * 25),
        "a thousand controls took {least:?}"
    );
}
