//! A to-do list, to show what an ODK application with some state looks like: a text field that
//! adds an item on Enter, check boxes, a segmented filter, buttons that name themselves for
//! assistive technology, a notice when the list cannot be kept, and a list that is kept between
//! runs in a SQLite database (`odk::db`) in the application's own data directory, so the Bundle
//! asks for no permission but `gpu`.
//!
//! The list itself, [`List`], knows nothing of ODK or of the database. The view, [`Todo`], draws
//! it with components only: no colour, size or duration is written here. It keeps what it draws
//! in memory, reads the database once before the window opens, and sends each change on without
//! waiting, so a slow disk never holds up the window.

use std::path::{Path, PathBuf};

use odk::db::{Connection, Database, Migrations};
use odk::prelude::*;

/// One thing to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: u64,
    pub text: String,
    pub done: bool,
}

/// Which items the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Active,
    Done,
}

impl Filter {
    pub const ALL: [Self; 3] = [Self::All, Self::Active, Self::Done];

    /// The value the segmented control carries for it.
    pub fn value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Done => "done",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Active => "Active",
            Self::Done => "Done",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.value() == value)
    }

    fn matches(self, item: &Item) -> bool {
        match self {
            Self::All => true,
            Self::Active => !item.done,
            Self::Done => item.done,
        }
    }

    /// What an empty list says under this filter.
    fn empty(self) -> &'static str {
        match self {
            Self::All => "Nothing yet. Type above and press Enter.",
            Self::Active => "Nothing left to do.",
            Self::Done => "Nothing is done yet.",
        }
    }
}

/// The items, in the order they were added.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct List {
    items: Vec<Item>,
    next: u64,
}

impl List {
    /// A list of `items` as they were kept, in the order given. A new item's id is past every
    /// one of theirs.
    pub fn from_items(items: Vec<Item>) -> Self {
        let next = items.iter().map(|i| i.id + 1).max().unwrap_or(0);
        Self { items, next }
    }

    /// Adds `text` as an open item, trimmed and on one line, and gives the item. Nothing is added
    /// for a text with nothing in it.
    pub fn add(&mut self, text: &str) -> Option<Item> {
        let text = one_line(text);
        if text.is_empty() {
            return None;
        }
        let item = Item {
            id: self.next,
            text,
            done: false,
        };
        self.items.push(item.clone());
        self.next += 1;
        Some(item)
    }

    pub fn set_done(&mut self, id: u64, done: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.done = done;
        }
    }

    pub fn remove(&mut self, id: u64) {
        self.items.retain(|i| i.id != id);
    }

    /// Removes every done item and says how many there were.
    pub fn clear_done(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|i| !i.done);
        before - self.items.len()
    }

    pub fn visible(&self, filter: Filter) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |i| filter.matches(i))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.items.iter().filter(|i| !i.done).count()
    }

    pub fn finished(&self) -> usize {
        self.len() - self.remaining()
    }
}

/// `text` trimmed, with each line break a space, since an item is one line.
fn one_line(text: &str) -> String {
    text.split(['\n', '\r'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The schema: one script, and a new one for each later change to it (`odk::db::Migrations`).
pub const SCHEMA: Migrations = Migrations::new(&["CREATE TABLE items (
        id   INTEGER PRIMARY KEY,
        text TEXT NOT NULL,
        done INTEGER NOT NULL DEFAULT 0
    )"]);

/// Where the database is: `$TODO_FILE` when set (for a test or a tool), otherwise `todo/todos.db`
/// in the data directory. Inside the sandbox `$HOME` is the application's own data directory, so
/// this is private to it and kept between runs.
pub fn default_store() -> Option<PathBuf> {
    store_in(|name| std::env::var_os(name))
}

/// [`default_store`] for the environment `var` gives, so that it can be tried without the
/// process's own.
pub fn store_in(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    match var("TODO_FILE").filter(|v| !v.is_empty()) {
        Some(file) => Some(file.into()),
        None => odk::db::data_file_in(var, "todo", "todos.db"),
    }
}

/// What the window starts from: the list, and the database that keeps it when there is one.
pub struct Store {
    db: Option<Database>,
    list: List,
    /// Why there is no database.
    notice: Option<String>,
}

impl Store {
    /// A list that is not kept anywhere: it lives in the window.
    pub fn none() -> Self {
        Self {
            db: None,
            list: List::default(),
            notice: None,
        }
    }

    /// A list kept in a database in memory, which is gone with the window: for a test.
    pub fn memory() -> Self {
        Self::from_database(Database::memory(&SCHEMA))
    }

    /// The list kept in the file `path`; none is [`Self::none`]. A file that cannot be opened or
    /// read does not stop the application: it starts with an empty list that is not kept, and
    /// says why. Blocks while it reads, which is right before the window is open.
    pub fn open(path: Option<&Path>) -> Self {
        match path {
            Some(path) => Self::from_database(Database::open(path, &SCHEMA)),
            None => Self::none(),
        }
    }

    fn from_database(opened: odk::db::Result<Database>) -> Self {
        let read = opened.and_then(|db| {
            let items = db.call_blocking(|c| {
                let mut statement = c.prepare("SELECT id, text, done FROM items ORDER BY id")?;
                let rows = statement.query_map([], |row| {
                    Ok(Item {
                        id: row.get::<_, i64>(0)? as u64,
                        text: row.get(1)?,
                        done: row.get::<_, i64>(2)? != 0,
                    })
                })?;
                Ok(rows.collect::<odk::db::rusqlite::Result<Vec<_>>>()?)
            })?;
            Ok((db, List::from_items(items)))
        });
        match read {
            Ok((db, list)) => Self {
                db: Some(db),
                list,
                notice: None,
            },
            Err(e) => Self {
                notice: Some(format!("Could not open your list: {e}")),
                ..Self::none()
            },
        }
    }
}

/// The window's content.
pub struct Todo {
    list: List,
    filter: Filter,
    /// What the new-item field holds.
    draft: SharedString,
    db: Option<Database>,
    /// Why the list could not be opened or kept: shown until the database has been made to match
    /// the list again.
    notice: Option<String>,
    /// Whether a write failed, so that the database may hold less than the list does.
    out_of_step: bool,
}

impl Todo {
    pub fn new(store: Store) -> Self {
        Self {
            list: store.list,
            filter: Filter::default(),
            draft: SharedString::default(),
            db: store.db,
            notice: store.notice,
            out_of_step: false,
        }
    }

    pub fn list(&self) -> &List {
        &self.list
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// The database that keeps the list, so that a test can wait until what was sent to it has
    /// been written: a call on it runs after every one sent before.
    pub fn database(&self) -> Option<&Database> {
        self.db.as_ref()
    }

    /// Adds what the field holds and empties the field.
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.list.add(&self.draft) else {
            return;
        };
        self.draft = SharedString::default();
        self.write(cx, move |c| {
            c.execute(
                "INSERT INTO items (id, text, done) VALUES (?1, ?2, 0)",
                odk::db::params![item.id as i64, item.text],
            )?;
            Ok(())
        });
    }

    pub fn set_done(&mut self, id: u64, done: bool, cx: &mut Context<Self>) {
        self.list.set_done(id, done);
        self.write(cx, move |c| {
            c.execute(
                "UPDATE items SET done = ?1 WHERE id = ?2",
                odk::db::params![done, id as i64],
            )?;
            Ok(())
        });
    }

    pub fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        self.list.remove(id);
        self.write(cx, move |c| {
            c.execute(
                "DELETE FROM items WHERE id = ?1",
                odk::db::params![id as i64],
            )?;
            Ok(())
        });
    }

    pub fn clear_done(&mut self, cx: &mut Context<Self>) {
        if self.list.clear_done() > 0 {
            self.write(cx, |c| {
                c.execute("DELETE FROM items WHERE done = 1", [])?;
                Ok(())
            });
        }
    }

    pub fn set_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    /// The list changed: draw it again, and send the change to the database without waiting for
    /// it. The database runs changes in the order they were sent.
    ///
    /// A change that fails comes back as a notice, and from then on the database may hold less
    /// than the list does, so the next change sent is the whole list instead: it replaces what is
    /// in the database in one transaction, and only that working takes the notice away. A later
    /// change that merely worked would hide that an earlier one was lost.
    fn write(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Connection) -> odk::db::Result<()> + Send + 'static,
    ) {
        cx.notify();
        let Some(db) = &self.db else {
            return;
        };
        let resync = self.out_of_step;
        let change: Change = if resync {
            let items: Vec<Item> = self.list.visible(Filter::All).cloned().collect();
            Box::new(move |c| replace_all(c, &items))
        } else {
            Box::new(change)
        };
        let done = db.call(change);
        cx.spawn(async move |this, cx| {
            let result = done.await;
            let _ = this.update(cx, |todo, cx| {
                match result {
                    Err(e) => {
                        todo.out_of_step = true;
                        todo.notice = Some(format!("Could not save your list: {e}"));
                    }
                    Ok(()) if resync => {
                        todo.out_of_step = false;
                        todo.notice = None;
                    }
                    Ok(()) => return,
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// A change to the database, chosen when it is sent.
type Change = Box<dyn FnOnce(&mut Connection) -> odk::db::Result<()> + Send>;

/// Makes the table hold `items` and nothing else, or leaves it as it was.
fn replace_all(c: &mut Connection, items: &[Item]) -> odk::db::Result<()> {
    let tx = c.transaction()?;
    tx.execute("DELETE FROM items", [])?;
    for item in items {
        tx.execute(
            "INSERT INTO items (id, text, done) VALUES (?1, ?2, ?3)",
            odk::db::params![item.id as i64, item.text, item.done],
        )?;
    }
    tx.commit()?;
    Ok(())
}

impl Render for Todo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let summary = match (self.list.len(), self.list.remaining()) {
            (0, _) => "No items yet".to_owned(),
            (total, 0) => format!("All {total} done"),
            (total, left) => format!("{left} left of {total}"),
        };
        let rows: Vec<_> = self
            .list
            .visible(self.filter)
            .map(|item| {
                let (id, done, text) = (item.id, item.done, item.text.clone());
                let toggle = this.clone();
                h_flex()
                    .gap(Space::Sm)
                    .child(
                        div().flex_1().min_w_0().child(
                            Checkbox::new(("item", id), text.clone())
                                .checked(done)
                                .on_change(move |checked, _, cx| {
                                    toggle.update(cx, |todo, cx| todo.set_done(id, checked, cx));
                                }),
                        ),
                    )
                    .child(
                        Button::icon(("remove", id), "×", format!("Delete {text}"))
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |todo, _, _, cx| todo.remove(id, cx))),
                    )
            })
            .collect();
        let none_shown = rows.is_empty();

        root()
            .p(Space::Lg)
            .gap(Space::Md)
            .child(Heading::new("Todo").level(1))
            .child(Text::new(summary).muted())
            .children(self.notice.clone().map(|notice| {
                Alert::new("notice", "Your list is not being kept")
                    .variant(Tone::Danger)
                    .description(notice)
            }))
            .child(
                h_flex()
                    .gap(Space::Sm)
                    .child(
                        div().flex_1().child(
                            Input::new("new", "New item")
                                .placeholder("What needs doing?")
                                .value(self.draft.clone())
                                .on_change(cx.listener(|todo, text: &str, _, cx| {
                                    todo.draft = text.to_owned().into();
                                    cx.notify();
                                }))
                                .on_submit(cx.listener(|todo, _: &str, _, cx| todo.submit(cx))),
                        ),
                    )
                    .child(
                        Button::new("add")
                            .label("Add")
                            .variant(ButtonVariant::Primary)
                            .disabled(self.draft.trim().is_empty())
                            .on_click(cx.listener(|todo, _, _, cx| todo.submit(cx))),
                    ),
            )
            .child(
                ToggleGroup::new("filter", "Show")
                    .item("all", Filter::All.label())
                    .item("active", Filter::Active.label())
                    .item("done", Filter::Done.label())
                    .value(vec![SharedString::from(self.filter.value())])
                    .on_change(cx.listener(|todo, value: &[SharedString], _, cx| {
                        // Choosing the chosen one again keeps it.
                        if let Some(filter) = value.first().and_then(|v| Filter::from_value(v)) {
                            todo.set_filter(filter, cx);
                        }
                    })),
            )
            .child(Separator::horizontal())
            .child(
                v_flex()
                    .id("items")
                    .flex_1()
                    .min_h_0()
                    .gap(Space::Xs)
                    .overflow_y_scroll()
                    .children(rows)
                    .children(none_shown.then(|| Text::new(self.filter.empty()).muted())),
            )
            .child(Separator::horizontal())
            .child(
                h_flex()
                    .justify_between()
                    .child(Badge::new(format!("{} done", self.list.finished())))
                    .child(
                        Button::new("clear")
                            .label("Clear done")
                            .variant(ButtonVariant::Outline)
                            .disabled(self.list.finished() == 0)
                            .on_click(cx.listener(|todo, _, _, cx| todo.clear_done(cx))),
                    ),
            )
    }
}
