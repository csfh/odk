//! The notes application against its Plugins, headless: nothing here opens a window, touches the
//! user's directories or makes a request.

use std::fs;
use std::path::Path;

use notes::{Notes, view};
use odk::theme::ThemeSnapshot;
use odk::ui::{Face, Hosted, headless};
use rune::Runtime;

type Rt = Runtime<Hosted<Notes>>;

/// A data directory with the example's own Plugins in it, and `extra` Plugins beside them.
fn data_dir(extra: &[(&str, &str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("plugins");
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins");
    for entry in fs::read_dir(own).unwrap().flatten() {
        let to = plugins.join(entry.file_name());
        fs::create_dir_all(&to).unwrap();
        for file in fs::read_dir(entry.path()).unwrap().flatten() {
            fs::copy(file.path(), to.join(file.file_name())).unwrap();
        }
    }
    for (id, capabilities, main) in extra {
        let to = plugins.join(id);
        fs::create_dir_all(&to).unwrap();
        fs::write(
            to.join("plugin.toml"),
            format!(
                "plugin_manifest_version = 1\nid = \"dev.example.{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\nentry = \"main.luau\"\ncapabilities = [{capabilities}]\n"
            ),
        )
        .unwrap();
        fs::write(to.join("main.luau"), main).unwrap();
    }
    dir
}

fn open(dir: &Path) -> Rt {
    let mut runtime = headless(
        Hosted::new(Notes::new(dir), ThemeSnapshot::emergency(), Face::bundled()),
        view,
        (560.0, 360.0),
    );
    settle(&mut runtime);
    runtime
}

fn settle(runtime: &mut Rt) {
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
}

fn tap(runtime: &mut Rt, key: &str) {
    let (x, y) = runtime.find(key).unwrap_or_else(|| panic!("no {key}"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    settle(runtime);
}

/// The labels of the Actions menu, in order, with the text of each row.
fn menu_labels(runtime: &Rt) -> Vec<String> {
    let mut rows: Vec<(usize, String)> = runtime
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| {
            let at = e
                .key
                .as_deref()?
                .strip_prefix("actions-item-")?
                .parse()
                .ok()?;
            Some((at, e.label.unwrap_or_default()))
        })
        .collect();
    rows.sort();
    rows.into_iter().map(|(_, label)| label).collect()
}

fn status(runtime: &Rt) -> String {
    runtime.state.app.status.clone()
}

#[test]
fn what_the_plugins_added_is_in_the_actions_menu_drawn_by_the_application() {
    let dir = data_dir(&[]);
    let mut runtime = open(dir.path());
    tap(&mut runtime, "actions");
    assert_eq!(
        menu_labels(&runtime),
        ["Fetch a quote", "Count words", "Clear note"]
            .map(String::from)
            .to_vec(),
        "the Plugins' items, in the order the Plugins load, then the application's own"
    );
}

#[test]
fn a_plugin_reads_the_note_the_application_exposed() {
    let dir = data_dir(&[]);
    let mut runtime = open(dir.path());
    runtime.state.app.note = "one two  three\nfour".into();
    tap(&mut runtime, "actions");
    tap(&mut runtime, "actions-item-1");
    assert_eq!(status(&runtime), "4 words");
}

#[test]
fn a_request_is_answered_after_the_call_that_made_it_and_the_plugin_gets_the_answer() {
    let dir = data_dir(&[]);
    let mut runtime = open(dir.path());
    tap(&mut runtime, "actions");
    tap(&mut runtime, "actions-item-0");
    assert_eq!(
        status(&runtime),
        "Make it work, make it right, make it fast."
    );
}

#[test]
fn a_plugin_that_never_ends_is_reported_and_the_application_goes_on() {
    let hostile = "ui.add('note-actions', { label = 'Spin', action = 'spin' })\nfunction spin() while true do end end";
    let dir = data_dir(&[("spin", "\"ui\"", hostile)]);
    let mut runtime = open(dir.path());
    runtime.state.app.note = "a b c".into();
    tap(&mut runtime, "actions");
    let labels = menu_labels(&runtime);
    assert!(labels.contains(&"Spin".to_string()), "{labels:?}");
    let spin = labels.iter().position(|l| l == "Spin").unwrap();
    tap(&mut runtime, &format!("actions-item-{spin}"));
    assert!(
        status(&runtime).contains("time budget") || status(&runtime).contains("spin"),
        "the failure is on the status line: {:?}",
        status(&runtime)
    );
    // The application is where it was: the other Plugins still work.
    tap(&mut runtime, "actions");
    let labels = menu_labels(&runtime);
    assert!(
        !labels.contains(&"Spin".to_string()),
        "the Plugin that ran out of time is unloaded with its items: {labels:?}"
    );
    let count = labels.iter().position(|l| l == "Count words").unwrap();
    tap(&mut runtime, &format!("actions-item-{count}"));
    assert_eq!(status(&runtime), "3 words");
}

#[test]
fn a_plugin_that_does_not_load_is_left_out_and_the_others_are() {
    // A syntax error: the host reports it and loads the rest.
    let dir = data_dir(&[("broken", "\"ui\"", "this is not luau (")]);
    let mut runtime = open(dir.path());
    tap(&mut runtime, "actions");
    assert_eq!(
        menu_labels(&runtime),
        ["Fetch a quote", "Count words", "Clear note"]
            .map(String::from)
            .to_vec()
    );
}

#[test]
fn with_no_plugins_the_application_has_only_its_own_items() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("plugins/nothing")).unwrap();
    let mut runtime = open(dir.path());
    tap(&mut runtime, "actions");
    assert_eq!(menu_labels(&runtime), ["Clear note".to_string()]);
}
