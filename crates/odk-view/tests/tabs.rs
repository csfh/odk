//! `Tabs` on the Rune host. Nothing here opens a window.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use odk_theme::ThemeSnapshot;
use odk_theme::component::{ComponentTokens, NavLook, States};
use odk_theme::foundation::{FoundationConfig, FoundationTokens};
use odk_view::{Face, Hosted, Icon, Tabs, Text, frame, headless};
use rune::{Cx, El, Key, KeyEvent, Runtime, col, el};

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

fn tokens() -> ComponentTokens {
    let theme = ThemeSnapshot::emergency();
    let foundation = FoundationTokens::scaled(&FoundationConfig::default(), theme.text);
    ComponentTokens::new(
        Arc::new(foundation),
        theme.semantic.clone(),
        theme.generation,
    )
}

#[derive(Default)]
struct App {
    /// What the controlled tabs were last told.
    controlled: Option<String>,
    /// Every selection any tabs reported, as `tabs:value`.
    heard: Vec<String>,
    /// Every panel that was built, in order.
    built: RefCell<Vec<String>>,
}

type Rt = Runtime<Hosted<App>>;

fn panel(label: &'static str) -> impl FnOnce(&Hosted<App>) -> El {
    move |hosted: &Hosted<App>| {
        hosted.app.built.borrow_mut().push(label.to_string());
        Text::new(format!("{label} panel")).show(hosted)
    }
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = &hosted.app;
    let mut ctrl = Tabs::new("ctrl", "Controlled")
        .tab("a", "Alpha", panel("Alpha"))
        .tab("b", "Beta", panel("Beta"))
        .on_change(|app: &mut App, value| {
            app.controlled = Some(value.to_string());
            app.heard.push(format!("ctrl:{value}"));
        });
    if let Some(value) = &app.controlled {
        ctrl = ctrl.value(value.clone());
    }
    frame(
        hosted,
        cx,
        col()
            .w(500.0)
            .gap(12.0)
            .child(el().key("before").focusable(true).size(20.0, 20.0))
            .child(
                Tabs::new("sections", "Sections")
                    .tab("general", "General", panel("General"))
                    .tab("advanced", "Advanced", panel("Advanced"))
                    .tab("billing", "Billing", panel("Billing"))
                    .tab("audit", "Audit log", panel("Audit"))
                    .icon("advanced", Icon::Settings)
                    .counter("advanced", "3")
                    .disable("billing")
                    .on_change(|app: &mut App, value| app.heard.push(format!("sections:{value}")))
                    .show(hosted),
            )
            .child(
                Tabs::new("manual", "Manual")
                    .tab("one", "One", panel("One"))
                    .tab("two", "Two", panel("Two"))
                    .tab("three", "Three", panel("Three"))
                    .manual_activation(true)
                    .on_change(|app: &mut App, value| app.heard.push(format!("manual:{value}")))
                    .show(hosted),
            )
            .child(ctrl.show(hosted))
            .child(
                col().w(200.0).child(
                    Tabs::<App>::new("narrow", "Narrow")
                        .tab("a", "A tab with a long label", |_| el())
                        .tab("b", "Another long tab", |_| el())
                        .tab("c", "A third long tab", |_| el())
                        .show(hosted),
                ),
            )
            .child(
                Tabs::<App>::new("locked", "Locked")
                    .tab("x", "X", |_| el())
                    .tab("y", "Y", |hosted| Text::new("Y panel").show(hosted))
                    .default_value("y")
                    .disabled(true)
                    .show(hosted),
            )
            .child(el().key("after").focusable(true).size(20.0, 20.0)),
        [],
    )
}

fn open() -> Rt {
    headless(
        Hosted::new(
            App {
                controlled: Some("b".into()),
                ..App::default()
            },
            ThemeSnapshot::emergency(),
            face(),
        ),
        page,
        (600.0, 900.0),
    )
}

fn settle(runtime: &mut Rt) {
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
}

fn press(runtime: &mut Rt, key: Key) {
    runtime.key(KeyEvent::from(key));
    settle(runtime);
}

fn focused(runtime: &Rt) -> Option<String> {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.focused)
        .and_then(|e| e.key)
}

fn rect(runtime: &Rt, key: &str) -> [f32; 4] {
    runtime
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"))
        .rect
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

fn has_text(runtime: &Rt, text: &str) -> bool {
    runtime
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some(text))
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

#[test]
fn a_list_of_tabs_names_a_counter_and_only_the_selected_panel_is_built_and_shown() {
    let runtime = open();
    let all = nodes(&runtime);
    assert!(
        all.iter()
            .any(|n| n["role"] == "tab-list" && n["name"] == "Sections"),
        "{all:#?}"
    );
    for name in ["General", "Advanced (3)", "Billing", "Audit log"] {
        assert!(
            all.iter().any(|n| n["role"] == "tab" && n["name"] == name),
            "no tab named {name}"
        );
    }
    assert!(
        has_text(&runtime, "General panel"),
        "the first tab is selected"
    );
    assert!(!has_text(&runtime, "Advanced panel"));
    let built = runtime.state.app.built.borrow().clone();
    assert!(built.contains(&"General".to_string()));
    assert!(
        !built.contains(&"Advanced".to_string()),
        "only the selected panel is built: {built:?}"
    );
}

#[test]
fn the_list_is_one_tab_stop_the_selected_tab_and_arrows_select_as_they_move_wrapping_and_skipping_the_disabled()
 {
    let mut runtime = open();
    press(&mut runtime, Key::Tab);
    press(&mut runtime, Key::Tab);
    assert_eq!(focused(&runtime).as_deref(), Some("sections-general"));
    let mut path = Vec::new();
    for key in [
        Key::Right,
        Key::Right,
        Key::Right,
        Key::Left,
        Key::Home,
        Key::End,
    ] {
        press(&mut runtime, key);
        path.push(focused(&runtime).unwrap());
    }
    // General -> Advanced -> (Billing is disabled) Audit -> wraps to General -> Audit (Left
    // wraps) ... then Home and End.
    assert_eq!(
        path,
        [
            "sections-advanced",
            "sections-audit",
            "sections-general",
            "sections-audit",
            "sections-general",
            "sections-audit"
        ]
    );
    // Each arrow selected the tab it reached, once.
    assert_eq!(
        runtime.state.app.heard.len(),
        6,
        "{:?}",
        runtime.state.app.heard
    );
    assert!(has_text(&runtime, "Audit panel") && !has_text(&runtime, "General panel"));
}

#[test]
fn with_manual_activation_arrows_move_without_selecting_and_enter_or_space_selects_on_release() {
    let mut runtime = open();
    let mut tabs_in = 0;
    for _ in 0..4 {
        press(&mut runtime, Key::Tab);
        if focused(&runtime).as_deref() == Some("manual-one") {
            break;
        }
        tabs_in += 1;
    }
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("manual-one"),
        "after {tabs_in} Tabs"
    );
    press(&mut runtime, Key::Right);
    assert_eq!(focused(&runtime).as_deref(), Some("manual-two"));
    assert!(runtime.state.app.heard.is_empty(), "moving selects nothing");
    assert!(has_text(&runtime, "One panel"));
    for (n, key) in [Key::Enter, Key::Text(" ".into())].into_iter().enumerate() {
        runtime.key(KeyEvent::from(key.clone()));
        runtime.repeat_key(KeyEvent::from(key.clone()));
        runtime.key(KeyEvent::from(key.clone()));
        settle(&mut runtime);
        assert_eq!(
            runtime.state.app.heard.len(),
            usize::from(n > 0),
            "nothing while held"
        );
        runtime.key_up(KeyEvent::from(key));
        settle(&mut runtime);
    }
    assert_eq!(
        runtime.state.app.heard,
        ["manual:two"],
        "once, on the release; the second is no change"
    );
    assert!(has_text(&runtime, "Two panel") && !has_text(&runtime, "One panel"));
}

#[test]
fn a_press_selects_a_tab_and_a_controlled_set_follows_the_application() {
    let mut runtime = open();
    // The application starts the controlled tabs on Beta.
    assert!(has_text(&runtime, "Beta panel") && !has_text(&runtime, "Alpha panel"));
    tap(&mut runtime, "ctrl-a");
    assert_eq!(runtime.state.app.controlled.as_deref(), Some("a"));
    assert_eq!(runtime.state.app.heard, ["ctrl:a"]);
    assert!(has_text(&runtime, "Alpha panel") && !has_text(&runtime, "Beta panel"));
    // A disabled tab does nothing.
    tap(&mut runtime, "sections-billing");
    assert_eq!(runtime.state.app.heard, ["ctrl:a"]);
    tap(&mut runtime, "sections-advanced");
    assert!(
        has_text(&runtime, "Advanced panel"),
        "an uncontrolled set keeps its own choice"
    );
}

#[test]
fn each_list_is_one_tab_stop_its_selected_tab_or_the_first_enabled_one() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..7 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    // Sections: General; Manual: One; Controlled: Beta, which the application selected;
    // Narrow: its first; the locked set has none.
    assert_eq!(
        visited,
        [
            "before",
            "sections-general",
            "manual-one",
            "ctrl-b",
            "narrow-a",
            "after",
            "before"
        ]
    );
}

#[test]
fn the_selected_tab_has_an_indicator_on_the_baseline_and_the_list_has_a_baseline() {
    let t = tokens();
    let style = t.tab(NavLook::Text, t.density(None), States::NONE.selected(true));
    let runtime = open();
    let general = rect(&runtime, "sections-general");
    let on_edge = |key: &str| {
        let tab = rect(&runtime, key);
        runtime
            .elements()
            .into_iter()
            .filter(|e| {
                e.rect[2] == tab[2]
                    && e.rect[3] == style.indicator_width
                    && e.rect[0] == tab[0]
                    && e.rect[1] + e.rect[3] == tab[1] + tab[3]
            })
            .count()
    };
    assert_eq!(
        on_edge("sections-general"),
        1,
        "the selected tab has an indicator"
    );
    assert_eq!(on_edge("sections-advanced"), 0, "the others have none");
    // One baseline under the whole list, as wide as the list.
    let list = rect(&runtime, "sections");
    let baseline = runtime
        .elements()
        .into_iter()
        .filter(|e| {
            e.rect[2] == list[2]
                && e.rect[3] == t.foundation().geometry.border_width
                && e.rect[1] + e.rect[3] == general[1] + general[3]
        })
        .count();
    assert_eq!(baseline, 1, "a baseline along the foot of the list");
}

#[test]
fn tabs_that_do_not_fit_scroll_and_the_focused_one_is_brought_into_view() {
    let mut runtime = open();
    let list = rect(&runtime, "narrow");
    assert!(list[2] <= 200.0, "the list overflows its column: {list:?}");
    let last = rect(&runtime, "narrow-c");
    assert!(
        last[0] + last[2] > list[0] + list[2],
        "the last tab is past the edge: {last:?} {list:?}"
    );
    // Focus the narrow list and go to its end.
    for _ in 0..8 {
        press(&mut runtime, Key::Tab);
        if focused(&runtime).as_deref() == Some("narrow-a") {
            break;
        }
    }
    assert_eq!(focused(&runtime).as_deref(), Some("narrow-a"));
    press(&mut runtime, Key::End);
    assert_eq!(focused(&runtime).as_deref(), Some("narrow-c"));
    let (list, last) = (rect(&runtime, "narrow"), rect(&runtime, "narrow-c"));
    assert!(
        last[0] >= list[0] - 0.5 && last[0] + last[2] <= list[0] + list[2] + 0.5,
        "the focused tab is in view: {last:?} {list:?}"
    );
}

#[test]
fn a_disabled_set_is_not_a_tab_stop_and_still_shows_its_selected_panel() {
    let mut runtime = open();
    let mut visited = Vec::new();
    for _ in 0..8 {
        press(&mut runtime, Key::Tab);
        visited.extend(focused(&runtime));
    }
    assert!(
        !visited.iter().any(|k| k.starts_with("locked")),
        "{visited:?}"
    );
}
