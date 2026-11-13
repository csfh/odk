//! The notes application against its Plugins and its database, headless: nothing here opens a
//! window, touches the user's directories or makes a request.

use std::fs;
use std::path::Path;

use notes::{Notes, view};
use odk::db::rusqlite::OptionalExtension;
use odk::theme::ThemeSnapshot;
use odk::ui::{Face, Hosted, headless};
use rune::{Key, KeyEvent, Runtime};

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

fn type_text(runtime: &mut Rt, text: &str) {
    for c in text.chars() {
        runtime.key(KeyEvent::from(Key::Text(c.to_string())));
        runtime.key_up(KeyEvent::from(Key::Text(c.to_string())));
    }
    settle(runtime);
}

/// The note in the database, read through the application's own handle, so after every write.
fn kept(runtime: &Rt) -> Option<String> {
    let db = runtime.state.app.database().expect("the database is open");
    db.call_blocking(|c| {
        Ok(
            c.query_row("SELECT text FROM note WHERE id = 1", [], |row| row.get(0))
                .optional()?,
        )
    })
    .unwrap()
}

/// The key of the Actions menu's row labelled `label`: a separator takes a number too.
fn item_key(runtime: &Rt, label: &str) -> String {
    runtime
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .find(|e| {
            e.label.as_deref() == Some(label)
                && e.key
                    .as_deref()
                    .is_some_and(|k| k.starts_with("actions-item-"))
        })
        .and_then(|e| e.key)
        .unwrap_or_else(|| panic!("no item {label}"))
}

/// Whether a notice that the note is not kept is drawn.
fn notice_drawn(runtime: &Rt) -> bool {
    runtime.elements().into_iter().any(|e| {
        [e.text.as_deref(), e.label.as_deref()]
            .into_iter()
            .flatten()
            .any(|t| t.contains("not saved") || t.contains("not kept"))
    })
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

#[test]
fn a_typed_note_is_kept_and_is_there_when_the_application_opens_again() {
    let dir = data_dir(&[]);
    let mut runtime = open(dir.path());
    tap(&mut runtime, "note");
    type_text(&mut runtime, "milk, eggs");
    assert_eq!(kept(&runtime).as_deref(), Some("milk, eggs"));
    assert!(!notice_drawn(&runtime));
    drop(runtime);

    let mut again = open(dir.path());
    assert_eq!(again.state.app.note, "milk, eggs");
    // The field starts with it, and an edit goes on from it.
    tap(&mut again, "note");
    again.key(KeyEvent::from(Key::End));
    type_text(&mut again, "!");
    assert_eq!(kept(&again).as_deref(), Some("milk, eggs!"));
}

#[test]
fn clearing_the_note_keeps_an_empty_note() {
    let dir = data_dir(&[]);
    let mut runtime = open(dir.path());
    tap(&mut runtime, "note");
    type_text(&mut runtime, "gone soon");
    tap(&mut runtime, "actions");
    let labels = menu_labels(&runtime);
    assert!(labels.contains(&"Clear note".to_string()), "{labels:?}");
    let clear = item_key(&runtime, "Clear note");
    tap(&mut runtime, &clear);
    assert_eq!(kept(&runtime).as_deref(), Some(""));
}

#[test]
fn a_change_that_fails_shows_a_notice_and_the_next_that_works_takes_it_away() {
    let dir = data_dir(&[]);
    let mut runtime = open(dir.path());
    tap(&mut runtime, "note");
    type_text(&mut runtime, "a");
    let db = runtime.state.app.database().unwrap().clone();
    db.call_blocking(|c| Ok(c.execute_batch("DROP TABLE note")?))
        .unwrap();
    type_text(&mut runtime, "b");
    // The write is on the database's thread; a read through the handle waits for it. In a
    // window, the write then wakes it through `on_host`, which is Rune's `update`; a headless
    // runtime has no window to post to, so the test makes that update.
    db.call_blocking(|_| Ok(())).unwrap();
    runtime.update(|_, _| {});
    settle(&mut runtime);
    let notice = runtime.state.app.notice().expect("a notice");
    assert!(notice.contains("not saved"), "{notice}");
    assert!(notice_drawn(&runtime), "the notice is drawn");

    db.call_blocking(|c| {
        Ok(c.execute_batch(
            "CREATE TABLE note (id INTEGER PRIMARY KEY CHECK (id = 1), text TEXT NOT NULL)",
        )?)
    })
    .unwrap();
    type_text(&mut runtime, "c");
    // The change that worked wrote the whole note, so nothing is missing.
    assert_eq!(kept(&runtime).as_deref(), Some("abc"));
    runtime.update(|_, _| {});
    settle(&mut runtime);
    assert_eq!(runtime.state.app.notice(), None);
    assert!(!notice_drawn(&runtime), "the notice is gone");
}

#[test]
fn a_database_that_does_not_open_is_a_notice_and_the_application_goes_on() {
    let dir = data_dir(&[]);
    // A directory where the file would be: SQLite cannot open it.
    fs::create_dir_all(dir.path().join("notes.sqlite")).unwrap();
    let mut runtime = open(dir.path());
    assert!(runtime.state.app.database().is_none());
    let notice = runtime.state.app.notice().expect("a notice");
    assert!(notice.contains("not kept"), "{notice}");
    assert!(notice_drawn(&runtime));
    tap(&mut runtime, "note");
    type_text(&mut runtime, "one two");
    assert_eq!(runtime.state.app.note, "one two");
    tap(&mut runtime, "actions");
    let labels = menu_labels(&runtime);
    let count = labels.iter().position(|l| l == "Count words").unwrap();
    tap(&mut runtime, &format!("actions-item-{count}"));
    assert_eq!(status(&runtime), "2 words");
}
