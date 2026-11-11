//! `FormField` on the Rune host, with each control it holds. Nothing here opens a window.

use std::sync::OnceLock;

use odk_theme::ThemeSnapshot;
use odk_view::{
    Checkbox, Face, FormField, Hosted, Input, RadioGroup, Select, Switch, form, frame, headless,
};
use rune::{Cx, El, Key, KeyEvent, Runtime};

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

/// Which fields the page draws, and how.
#[derive(Default, Clone)]
struct App {
    name: String,
    error: Option<&'static str>,
    required: bool,
    optional: bool,
    disabled: bool,
    help: Option<&'static str>,
    agreed: bool,
}

fn page(hosted: &Hosted<App>, cx: &mut Cx<Hosted<App>>) -> El {
    let app = hosted.app.clone();
    let mut layers = Vec::new();
    let mut name = FormField::new("name-field", "Name")
        .description("Pick a unique name")
        .required(app.required)
        .optional(app.optional)
        .disabled(app.disabled)
        .child(Input::new(
            "name",
            "Name",
            |app: &mut App, text| app.name = text,
            |_: &mut App| {},
        ));
    name = match (app.error, app.help) {
        (Some(error), _) => name.error(error),
        (None, Some(help)) => name.help(help),
        (None, None) => name,
    };
    let body = form(hosted)
        .key("page")
        .padding(24.0)
        .child(name.show(hosted, &mut layers))
        .child(
            FormField::new("fruit-field", "Fruit")
                .child(Select::new(
                    "fruit",
                    "Fruit",
                    vec!["Apple".into(), "Banana".into()],
                    None,
                    |_: &mut App, _| {},
                ))
                .error("Choose one")
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("terms-field", "Terms")
                .child(
                    Checkbox::new("terms", "I agree")
                        .checked(app.agreed)
                        .on_change(|app: &mut App, on| app.agreed = on),
                )
                .error("You must agree")
                .required(true)
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("push-field", "Notifications")
                .child(Switch::new("push", "Push"))
                .description("Tell me about activity")
                .show(hosted, &mut layers),
        )
        .child(
            FormField::new("theme-field", "Theme")
                .child(
                    RadioGroup::new("theme", "Theme")
                        .item("light", "Light")
                        .item("dark", "Dark"),
                )
                .error("Pick a theme")
                .show(hosted, &mut layers),
        );
    frame(hosted, cx, body, layers)
}

fn open(app: App) -> Rt {
    headless(
        Hosted::new(app, ThemeSnapshot::emergency(), face()),
        page,
        (480.0, 720.0),
    )
}

fn settle(runtime: &mut Rt) {
    for _ in 0..8 {
        runtime.frame(1.0 / 60.0);
    }
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

fn node(runtime: &Rt, role: &str, name: &str) -> serde_json::Value {
    nodes(runtime)
        .into_iter()
        .find(|n| n["role"] == role && n["name"] == name)
        .unwrap_or_else(|| panic!("no {role} named {name}"))
}

fn has(node: &serde_json::Value, state: &str) -> bool {
    node["states"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|s| s == state)
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

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("no element {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

/// How many elements sit under the one keyed `key`.
fn descendants(runtime: &Rt, key: &str) -> usize {
    let elements = runtime.elements();
    let root = elements
        .iter()
        .position(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element {key}"));
    (0..elements.len())
        .filter(|&i| {
            let mut at = i;
            while let Some(parent) = elements[at].parent {
                if parent == root {
                    return true;
                }
                at = parent;
            }
            false
        })
        .count()
}

fn texts(runtime: &Rt) -> Vec<String> {
    runtime
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect()
}

fn shown(app: App) -> Rt {
    let mut runtime = open(app);
    settle(&mut runtime);
    runtime
}

#[test]
fn a_field_stacks_its_label_its_control_and_its_message() {
    let runtime = shown(App {
        error: Some("Taken"),
        ..App::default()
    });
    let label = rect(&runtime, "name-field-label");
    let control = rect(&runtime, "name-box");
    let message = rect(&runtime, "name-field-message");
    assert!(label[1] + label[3] <= control[1], "label above the control");
    assert!(
        control[1] + control[3] <= message[1],
        "message under the control"
    );
    // The fields of a form are `form_gap` apart, more than a field's own parts.
    let next = rect(&runtime, "fruit-field");
    let this = rect(&runtime, "name-field");
    assert!(next[1] >= this[1] + this[3], "fields do not overlap");
}

#[test]
fn the_description_and_the_message_reach_the_control_and_only_an_error_marks_it_invalid() {
    let wrong = shown(App {
        error: Some("Taken"),
        ..App::default()
    });
    let name = node(&wrong, "text-input", "Name");
    assert_eq!(name["description"], "Pick a unique name. Taken");
    assert!(has(&name, "invalid"), "{name}");

    let help = shown(App {
        help: Some("Letters only"),
        ..App::default()
    });
    let name = node(&help, "text-input", "Name");
    assert_eq!(name["description"], "Pick a unique name. Letters only");
    assert!(!has(&name, "invalid"), "help leaves the control valid");

    let plain = shown(App::default());
    let name = node(&plain, "text-input", "Name");
    assert_eq!(name["description"], "Pick a unique name");
}

#[test]
fn required_is_a_star_after_the_label_and_optional_the_word() {
    let required = texts(&shown(App {
        required: true,
        ..App::default()
    }));
    assert!(required.iter().any(|t| t == "*"), "{required:?}");
    assert!(!required.iter().any(|t| t == "(optional)"));

    let optional = texts(&shown(App {
        optional: true,
        ..App::default()
    }));
    assert!(optional.iter().any(|t| t == "(optional)"), "{optional:?}");

    let both = texts(&shown(App {
        required: true,
        optional: true,
        ..App::default()
    }));
    assert!(
        both.iter().any(|t| t == "*") && !both.iter().any(|t| t == "(optional)"),
        "a required field is never also optional: {both:?}"
    );
}

#[test]
fn a_press_on_a_text_fields_label_focuses_it() {
    let mut runtime = shown(App::default());
    assert_eq!(focused(&runtime), None);
    tap(&mut runtime, "name-field-label");
    assert_eq!(focused(&runtime).as_deref(), Some("name"));
}

#[test]
fn a_press_on_the_label_of_a_control_with_its_own_label_focuses_nothing() {
    let mut runtime = shown(App::default());
    tap(&mut runtime, "terms-field-label");
    assert_eq!(focused(&runtime), None, "a checkbox's label is its legend");
    tap(&mut runtime, "fruit-field-label");
    assert_eq!(
        focused(&runtime),
        None,
        "Rune cannot move focus to a select by key"
    );
}

#[test]
fn a_disabled_field_disables_its_control_and_its_label_does_nothing() {
    let mut runtime = shown(App {
        disabled: true,
        ..App::default()
    });
    let name = node(&runtime, "text-input", "Name");
    assert!(has(&name, "disabled"), "{name}");
    tap(&mut runtime, "name-field-label");
    assert_eq!(focused(&runtime), None);
    runtime.key(KeyEvent::from(Key::Tab));
    settle(&mut runtime);
    assert_ne!(
        focused(&runtime).as_deref(),
        Some("name"),
        "a disabled field is out of the Tab order"
    );
}

#[test]
fn every_control_reports_what_the_field_says() {
    let runtime = shown(App::default());
    let terms = node(&runtime, "check-box", "I agree");
    assert!(has(&terms, "invalid"), "{terms}");
    assert_eq!(terms["description"], "You must agree");
    let fruit = node(&runtime, "combo-box", "Fruit");
    assert!(has(&fruit, "invalid"), "{fruit}");
    assert_eq!(fruit["description"], "Choose one");
    let push = node(&runtime, "switch", "Push");
    assert_eq!(push["description"], "Tell me about activity");
    let theme = node(&runtime, "radio-group", "Theme");
    assert!(has(&theme, "invalid"), "{theme}");
    assert_eq!(theme["description"], "Pick a theme");
}

#[test]
fn an_error_has_a_mark_so_it_never_shows_by_colour_alone() {
    let wrong = shown(App {
        error: Some("Taken"),
        ..App::default()
    });
    let help = shown(App {
        help: Some("Taken"),
        ..App::default()
    });
    assert_eq!(
        descendants(&wrong, "name-field-message"),
        descendants(&help, "name-field-message") + 2,
        "the mark and its line box come with an error, and plain help has neither"
    );
}

#[test]
fn the_labels_and_messages_are_not_read_twice() {
    let runtime = shown(App {
        error: Some("Taken"),
        required: true,
        ..App::default()
    });
    let words: Vec<_> = nodes(&runtime)
        .into_iter()
        .filter(|n| n["key"] == "name-field-label" || n["key"] == "name-field-message")
        .collect();
    assert!(words.is_empty(), "{words:?}");
}

#[test]
fn a_field_describes_its_control_with_its_own_state() {
    use odk_view::{Describe, Role};
    let field = FormField::<App>::new("f", "Name")
        .description("Pick a unique name")
        .required(true)
        .disabled(true)
        .error("Taken")
        .child(Input::new(
            "name",
            "Name",
            |_: &mut App, _| {},
            |_: &mut App| {},
        ));
    let a = field.accessible();
    assert_eq!(a.role, Role::TextInput);
    assert_eq!(a.name, "Name");
    assert!(a.invalid && a.required && a.disabled);
    assert_eq!(a.description.as_deref(), Some("Pick a unique name. Taken"));

    let empty = FormField::<App>::new("f", "Group").accessible();
    assert_eq!(empty.role, Role::Group);
    assert_eq!(empty.name, "Group");
}

#[test]
fn a_label_press_moves_focus_from_wherever_it_was() {
    let mut runtime = shown(App::default());
    // Focus somewhere else first: a checkbox, then the select's trigger.
    tap(&mut runtime, "terms");
    assert_eq!(focused(&runtime).as_deref(), Some("terms"));
    tap(&mut runtime, "name-field-label");
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("name"),
        "the label takes focus from the checkbox"
    );
    runtime.key(KeyEvent::from(Key::Tab));
    settle(&mut runtime);
    runtime.key(KeyEvent::from(Key::Tab));
    settle(&mut runtime);
    tap(&mut runtime, "name-field-label");
    assert_eq!(
        focused(&runtime).as_deref(),
        Some("name"),
        "and again from a later stop"
    );
}

#[test]
fn focus_the_label_gave_is_not_kept_when_the_person_leaves() {
    let mut runtime = shown(App::default());
    tap(&mut runtime, "name-field-label");
    assert_eq!(focused(&runtime).as_deref(), Some("name"));
    runtime.key(KeyEvent::from(Key::Tab));
    for _ in 0..6 {
        settle(&mut runtime);
    }
    assert_ne!(
        focused(&runtime).as_deref(),
        Some("name"),
        "Tab leaves the field and nothing pulls focus back"
    );
    // Tab away again and rebuild a few more frames.
    runtime.invalidate();
    for _ in 0..6 {
        settle(&mut runtime);
    }
    assert_ne!(focused(&runtime).as_deref(), Some("name"));
}
