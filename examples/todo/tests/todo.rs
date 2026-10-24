//! The to-do list: its model, the database it is kept in, and the window on GPUI's test platform,
//! driven from the keyboard. Nothing opens a window, and nothing reads the real home: every test
//! keeps its list in memory or in a directory of its own.

use std::path::PathBuf;

use odk::gpui::{
    AppContext as _, Entity, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext,
};
use odk::ui::test_support::open_view;
use tempfile::TempDir;
use todo::{Filter, Item, List, Store, Todo, store_in};

// The model.

#[test]
fn an_item_is_trimmed_on_one_line_and_nothing_is_not_an_item() {
    let mut list = List::default();
    assert_eq!(list.add("  Buy milk ").unwrap().text, "Buy milk");
    assert!(list.add("two\nlines\r\nhere").is_some());
    for nothing in ["", "   ", "\n", " \r\n "] {
        assert!(list.add(nothing).is_none(), "{nothing:?}");
    }
    let texts: Vec<_> = list.visible(Filter::All).map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["Buy milk", "two lines here"]);
}

#[test]
fn items_are_done_removed_cleared_and_counted_and_ids_are_never_reused() {
    let mut list = List::default();
    for text in ["a", "b", "c"] {
        list.add(text);
    }
    assert_eq!((list.len(), list.remaining(), list.finished()), (3, 3, 0));
    list.set_done(0, true);
    list.set_done(2, true);
    list.set_done(99, true); // no such item: nothing happens
    assert_eq!((list.remaining(), list.finished()), (1, 2));

    let shown =
        |list: &List, f| -> Vec<String> { list.visible(f).map(|i| i.text.clone()).collect() };
    assert_eq!(shown(&list, Filter::Active), ["b"]);
    assert_eq!(shown(&list, Filter::Done), ["a", "c"]);

    list.remove(1);
    list.add("d");
    let ids: Vec<_> = list.visible(Filter::All).map(|i| i.id).collect();
    assert_eq!(
        ids,
        [0, 2, 3],
        "the id of a removed item is not given again"
    );
    assert_eq!(list.clear_done(), 2);
    assert_eq!(shown(&list, Filter::All), ["d"]);
    assert_eq!(list.clear_done(), 0);
}

#[test]
fn a_filter_is_found_by_its_value_and_by_no_other() {
    for filter in Filter::ALL {
        assert_eq!(Filter::from_value(filter.value()), Some(filter));
    }
    assert_eq!(Filter::from_value("everything"), None);
}

// What is kept.

#[test]
fn a_list_read_back_continues_its_ids_past_the_largest() {
    let item = |id, text: &str| Item {
        id,
        text: text.into(),
        done: id == 5,
    };
    let mut list = List::from_items(vec![item(2, "a"), item(5, "b")]);
    assert_eq!(list.finished(), 1);
    assert_eq!(list.add("c").unwrap().id, 6);
    assert_eq!(List::from_items(Vec::new()).add("first").unwrap().id, 0);
}

#[test]
fn the_database_is_where_the_environment_says() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| std::ffi::OsString::from(v))
        }
    };
    assert_eq!(
        store_in(env(&[("TODO_FILE", "/x/list.db"), ("HOME", "/h")])),
        Some(PathBuf::from("/x/list.db"))
    );
    assert_eq!(
        store_in(env(&[("XDG_DATA_HOME", "/d"), ("HOME", "/h")])),
        Some(PathBuf::from("/d/todo/todos.db"))
    );
    assert_eq!(
        store_in(env(&[("HOME", "/h")])),
        Some(PathBuf::from("/h/.local/share/todo/todos.db"))
    );
    assert_eq!(
        store_in(env(&[
            ("TODO_FILE", ""),
            ("XDG_DATA_HOME", ""),
            ("HOME", "/h")
        ])),
        Some(PathBuf::from("/h/.local/share/todo/todos.db")),
        "empty is not set"
    );
    assert_eq!(store_in(env(&[])), None);
}

// The window, from the keyboard.

/// The tests share the process's theme and foundation tokens, so they run one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Window<'a> {
    _serial: std::sync::MutexGuard<'static, ()>,
    todo: Entity<Todo>,
    cx: &'a mut VisualTestContext,
}

fn open<'a>(app: &'a mut TestAppContext, store: Store) -> Window<'a> {
    let serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (todo, cx) = open_view(app, |_, cx| cx.new(|_| Todo::new(store)));
    Window {
        _serial: serial,
        todo,
        cx,
    }
}

impl Window<'_> {
    fn press(&mut self, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        self.cx
            .update(|window, cx| window.dispatch_keystroke(keystroke.clone(), cx));
        self.cx.simulate_event(KeyUpEvent { keystroke });
        self.cx.run_until_parked();
    }

    fn type_in(&mut self, text: &str) {
        self.cx.simulate_input(text);
        self.cx.run_until_parked();
    }

    fn focused(&mut self) -> String {
        self.cx.update(|window, cx| {
            odk::ui::focused_component(window, cx)
                .map(|s| s.to_string())
                .unwrap_or_default()
        })
    }

    /// Tabs until the focus is on `part`, giving up after a lap.
    fn tab_to(&mut self, part: &str) {
        for _ in 0..16 {
            if self.focused() == part {
                return;
            }
            self.press("tab");
        }
        panic!(
            "Tab does not reach {part}; the focus is on {}",
            self.focused()
        );
    }

    fn texts(&mut self) -> Vec<String> {
        self.todo.read_with(self.cx, |t, _| {
            t.list()
                .visible(Filter::All)
                .map(|i| i.text.clone())
                .collect()
        })
    }

    fn add(&mut self, text: &str) {
        self.tab_to("new/focus");
        self.type_in(text);
        self.press("enter");
    }
}

impl Window<'_> {
    /// What the database holds, as `(text, done)` in order, read after every change sent to it.
    fn kept(&mut self) -> Vec<(String, bool)> {
        let db = self
            .todo
            .read_with(self.cx, |t, _| t.database().cloned())
            .expect("this window has a database");
        db.call_blocking(|c| {
            let mut statement = c.prepare("SELECT text, done FROM items ORDER BY id")?;
            let rows = statement.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)))?;
            Ok(rows.collect::<odk::db::rusqlite::Result<Vec<_>>>()?)
        })
        .unwrap()
    }

    /// Waits for what has been sent to the database, then lets the window hear how it went.
    fn settle(&mut self) {
        let db = self
            .todo
            .read_with(self.cx, |t, _| t.database().cloned())
            .expect("this window has a database");
        db.call_blocking(|_| Ok(())).unwrap();
        self.cx.run_until_parked();
    }
}

fn row(text: &str, done: bool) -> (String, bool) {
    (text.to_owned(), done)
}

#[test]
fn typing_and_enter_add_an_item_and_empty_the_field() {
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::memory());
    w.add("Buy milk");
    w.add("Call mum");
    assert_eq!(
        w.texts(),
        ["Buy milk", "Call mum"],
        "the field was empty for the second"
    );
    assert_eq!(w.todo.read_with(w.cx, |t, _| t.draft().to_owned()), "");
    assert_eq!(w.kept(), [row("Buy milk", false), row("Call mum", false)]);
    // Enter on an empty field adds nothing.
    w.press("enter");
    assert_eq!(w.texts().len(), 2);
}

#[test]
fn space_checks_an_item_and_clear_done_removes_what_is_done() {
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::memory());
    w.add("one");
    w.add("two");

    w.tab_to("item-1/focus");
    w.press("space");
    assert_eq!(w.kept(), [row("one", false), row("two", true)]);
    assert_eq!(w.todo.read_with(w.cx, |t, _| t.list().finished()), 1);

    w.tab_to("clear/focus");
    w.press("enter");
    assert_eq!(w.texts(), ["one"]);
    assert_eq!(w.kept(), [row("one", false)]);
    // Nothing is done now, so the button is out of the Tab order.
    for _ in 0..10 {
        w.press("tab");
        assert_ne!(w.focused(), "clear/focus");
    }
}

#[test]
fn an_item_is_deleted_by_its_named_button_from_the_keyboard() {
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::memory());
    w.add("one");
    w.add("two");
    w.tab_to("remove-0/focus");
    w.press("enter");
    assert_eq!(w.texts(), ["two"]);
    assert_eq!(w.kept(), [row("two", false)]);
}

#[test]
fn the_filter_decides_what_is_shown_and_never_what_is_kept() {
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::memory());
    w.add("one");
    w.add("two");
    w.tab_to("item-0/focus");
    w.press("space");

    w.todo
        .update(w.cx, |t, cx| t.set_filter(Filter::Active, cx));
    w.cx.run_until_parked();
    let shown = |w: &mut Window| -> Vec<String> {
        let todo = w.todo.clone();
        todo.read_with(w.cx, |t, _| {
            t.list()
                .visible(t.filter())
                .map(|i| i.text.clone())
                .collect()
        })
    };
    assert_eq!(shown(&mut w), ["two"]);
    w.todo.update(w.cx, |t, cx| t.set_filter(Filter::Done, cx));
    assert_eq!(shown(&mut w), ["one"]);
    assert_eq!(
        w.kept(),
        [row("one", true), row("two", false)],
        "the database holds all of it"
    );
}

#[test]
fn a_list_in_a_file_comes_back_at_the_next_start_and_new_items_go_on_after_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deeper/todos.db");
    {
        let mut app = TestAppContext::single();
        let mut w = open(&mut app, Store::open(Some(&path)));
        w.add("first");
        w.add("second");
        w.tab_to("item-0/focus");
        w.press("space");
        w.settle();
    }
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::open(Some(&path)));
    assert_eq!(w.texts(), ["first", "second"]);
    assert_eq!(w.todo.read_with(w.cx, |t, _| t.list().finished()), 1);
    w.add("third");
    assert_eq!(
        w.kept(),
        [
            row("first", true),
            row("second", false),
            row("third", false)
        ],
        "the new item's id does not collide with the kept ones"
    );
}

#[test]
fn a_database_that_cannot_be_opened_says_so_and_the_list_still_works_in_the_window() {
    let dir = TempDir::new().unwrap();
    // A directory where the file should be: SQLite cannot open it as a database.
    let path = dir.path().join("todos.db");
    std::fs::create_dir(&path).unwrap();
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::open(Some(&path)));
    let notice = w.todo.read_with(w.cx, |t, _| t.notice().map(str::to_owned));
    assert!(notice.unwrap().starts_with("Could not open your list"));
    w.add("only in the window");
    assert_eq!(w.texts(), ["only in the window"]);
}

#[test]
fn a_change_that_cannot_be_kept_is_a_notice_and_the_next_one_saves_the_whole_list() {
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::memory());
    w.add("kept before");
    let db = w
        .todo
        .read_with(w.cx, |t, _| t.database().cloned())
        .unwrap();
    // The table is gone: the next change fails.
    db.call_blocking(|c| Ok(c.execute_batch("DROP TABLE items")?))
        .unwrap();
    w.add("not kept");
    w.settle();
    assert_eq!(
        w.texts(),
        ["kept before", "not kept"],
        "the window does not wait for the disk to agree"
    );
    let notice = w.todo.read_with(w.cx, |t, _| t.notice().map(str::to_owned));
    assert!(notice.unwrap().starts_with("Could not save your list"));

    // The table is back. Nothing is saved until the user does something, and the notice stays.
    db.call_blocking(|c| {
        Ok(c.execute_batch(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, text TEXT NOT NULL, done INTEGER NOT NULL DEFAULT 0)",
        )?)
    })
    .unwrap();
    w.settle();
    assert!(w.todo.read_with(w.cx, |t, _| t.notice().is_some()));

    // The next change saves all of the list, the lost one with it, and only that takes the
    // notice away.
    w.add("kept after");
    w.settle();
    assert!(w.todo.read_with(w.cx, |t, _| t.notice().is_none()));
    assert_eq!(
        w.kept(),
        [
            row("kept before", false),
            row("not kept", false),
            row("kept after", false)
        ],
        "what the window shows is what is kept"
    );

    // And after that changes are sent one at a time again.
    w.tab_to("item-0/focus");
    w.press("space");
    assert_eq!(w.kept()[0], row("kept before", true));
}

#[test]
fn without_a_database_the_list_lives_only_in_the_window() {
    let mut app = TestAppContext::single();
    let mut w = open(&mut app, Store::none());
    w.add("only here");
    assert_eq!(w.texts(), ["only here"]);
    assert!(w.todo.read_with(w.cx, |t, _| t.notice().is_none()));
}
