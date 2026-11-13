//! A small application that uses ODK's Plugins (ADR 0092, `docs/spec/plugin-api.md`): a note, an
//! Actions menu, and a status line. It is the template for one that does.
//!
//! What it shows of the Plugin host:
//!
//! - **An extension point.** The application registers `note-actions`; a Plugin with `ui` adds a
//!   description (`label`, `action`) to it; the application draws each as an item of its own
//!   Actions menu, with the menu component, and a Plugin draws nothing itself.
//! - **A use.** Choosing an item calls `Host::activate`; what the Plugin's action returns (text) is
//!   the status line.
//! - **What a Plugin may read.** The note is exposed as `note` for a Plugin with `app.state`.
//! - **A request.** A Plugin with `network` asks; the application's handler queues it, and after
//!   the call that made it the application answers (here from a fixed table: ODK ships no HTTP
//!   client, and a real application would make the request, off its window's thread) and reads
//!   the Plugin's `result_text` if it has one.
//! - **Failure.** A Plugin that fails, or runs out of time, is reported to the application, which
//!   shows it, and goes on; nothing a Plugin does stops the application.
//!
//! What it shows of the database (`odk::db`, ADR 0049, `docs/spec/odk-db.md`): the note is kept in
//! `notes.sqlite` in the data directory, read once before the window, and every change is sent
//! without waiting. A change that fails puts a notice under the note; the next one that works
//! takes it away, since each change writes the whole note. A database that cannot be opened is a
//! notice too, and the application goes on with the note in memory.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use odk::db::{Database, Migrations};
use odk::plugin::{CallError, Contribution, Data, Grants, Host, Limits, NetworkRequest};
use odk::prelude::*;
use odk::ui::{Alert, DropdownMenu, Input, Menu, MenuItem, Text, Tone};

/// The extension point the application registers.
pub const POINT: &str = "note-actions";

/// The Plugins that come with the example, written to the Plugin directory when it is empty.
const SAMPLES: &[(&str, &[(&str, &str)])] = &[
    (
        "quote",
        &[
            ("plugin.toml", include_str!("../plugins/quote/plugin.toml")),
            ("main.luau", include_str!("../plugins/quote/main.luau")),
        ],
    ),
    (
        "word-count",
        &[
            (
                "plugin.toml",
                include_str!("../plugins/word-count/plugin.toml"),
            ),
            ("main.luau", include_str!("../plugins/word-count/main.luau")),
        ],
    ),
];

/// The database's schema: one row, the note. A script here is never edited; a change is a new one.
const SCHEMA: Migrations = Migrations::new(&[
    "CREATE TABLE note (id INTEGER PRIMARY KEY CHECK (id = 1), text TEXT NOT NULL)",
]);

/// The application's state.
pub struct Notes {
    /// The note as drawn. Change it with [`Notes::set_note`], which keeps it.
    pub note: String,
    pub status: String,
    pub menu: bool,
    /// The note's database, or `None` when it could not be opened (see [`Notes::notice`]).
    db: Option<Database>,
    /// Why the note is not kept: the last change failed, or the database did not open. Written on
    /// the database's thread, read by the view.
    unsaved: Arc<Mutex<Option<String>>>,
    host: Host,
    /// What the Plugins asked for and nobody has answered.
    requests: Rc<RefCell<Vec<NetworkRequest>>>,
    /// What the host reported of the Plugins since the status line last showed it.
    reports: Rc<RefCell<Vec<String>>>,
}

impl Notes {
    /// The application with the Plugins in `<data_dir>/plugins` (the samples are written there
    /// first when it holds none) and the note in `<data_dir>/notes.sqlite`. `data_dir` is
    /// [`data_dir`]: `$HOME` inside the sandbox, which is the pair's data directory on the host
    /// (ADR 0099).
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let (db, note, unsaved) = open_note(&data_dir.join("notes.sqlite"));
        let plugins = data_dir.join("plugins");
        install_samples(&plugins);
        let reports: Rc<RefCell<Vec<String>>> = Rc::default();
        let heard = reports.clone();
        // The application grants all four capabilities: it is its own Plugin directory, and a
        // real one would grant what its users have agreed to (ADR 0020).
        let mut host = Host::new(&data_dir, Grants::all(), Limits::default(), move |e| {
            heard
                .borrow_mut()
                .push(format!("{}: {}", e.plugin, e.message));
        });
        host.register_point(POINT);
        let requests: Rc<RefCell<Vec<NetworkRequest>>> = Rc::default();
        let queue = requests.clone();
        // The handler runs inside the Plugin's call: it queues and returns.
        host.on_network(move |r| queue.borrow_mut().push(r));
        host.load_dir(&plugins);
        Self {
            note,
            status: String::new(),
            menu: false,
            db,
            unsaved: Arc::new(Mutex::new(unsaved)),
            host,
            requests,
            reports,
        }
    }

    /// Changes the note and sends it to the database without waiting. The write runs on the
    /// database's thread after every one before it; what it says is the notice, and the window is
    /// woken only when the notice changes.
    pub fn set_note(&mut self, text: String) {
        self.note = text;
        let Some(db) = &self.db else {
            return;
        };
        let text = self.note.clone();
        let unsaved = self.unsaved.clone();
        // The future is dropped: the call is already queued and runs anyway (`docs/spec/odk-db.md`).
        drop(db.call(move |c| {
            let written = c.execute(
                "INSERT INTO note (id, text) VALUES (1, ?1) \
                 ON CONFLICT (id) DO UPDATE SET text = excluded.text",
                [&text],
            );
            let now = written
                .err()
                .map(|e| format!("The note was not saved: {e}"));
            let mut slot = unsaved.lock().unwrap_or_else(|e| e.into_inner());
            if *slot != now {
                *slot = now;
                // Only to draw the notice: there is no window under a headless test, and the
                // test draws the next frame itself.
                let _ = odk::ui::on_host::<Notes>(|_| {});
            }
            Ok(())
        }));
    }

    /// Why the note is not kept, when it is not.
    pub fn notice(&self) -> Option<String> {
        self.unsaved
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The note's database, for a test to read it through the handle the application writes with,
    /// which orders the read after every write.
    pub fn database(&self) -> Option<&Database> {
        self.db.as_ref()
    }

    /// The items the Plugins added to the Actions menu.
    pub fn contributions(&self) -> Vec<Contribution> {
        self.host.contributions(POINT)
    }

    /// The user chose a Plugin's item.
    fn use_item(&mut self, plugin: &str, id: u64) {
        self.host.expose("note", Data::Text(self.note.clone()));
        let result = self.host.activate(plugin, id);
        self.status = match result {
            Ok(Data::Text(text)) => text,
            Ok(_) => String::new(),
            Err(CallError::Failed(e)) => format!("{plugin}: {}", e.message),
            Err(other) => other.to_string(),
        };
        self.answer_requests();
        // A Plugin that answers later says so in `result_text`, by this convention.
        if let Ok(Data::Text(text)) = self.host.call(plugin, "result_text", &[]) {
            self.status = text;
        }
        if let Some(report) = self.reports.borrow_mut().pop() {
            self.status = report;
        }
        self.reports.borrow_mut().clear();
    }

    /// Answers what the Plugins asked for, after the call that made the request has returned.
    fn answer_requests(&mut self) {
        let waiting: Vec<NetworkRequest> = self.requests.borrow_mut().drain(..).collect();
        for request in waiting {
            let answer = canned(&request.request);
            // A Plugin that has no `on_network` simply is not told.
            let _ = self.host.answer(&request.plugin, request.id, answer);
        }
    }
}

/// Opens the note's database at `path` and reads the note, before the window. A database that
/// cannot be opened, or read, leaves the note empty and in memory, with a notice that says why.
fn open_note(path: &Path) -> (Option<Database>, String, Option<String>) {
    let read = Database::open(path, &SCHEMA).and_then(|db| {
        let note = db.call_blocking(|c| {
            let mut statement = c.prepare("SELECT text FROM note WHERE id = 1")?;
            let mut rows = statement.query([])?;
            Ok(match rows.next()? {
                Some(row) => row.get(0)?,
                None => String::new(),
            })
        })?;
        Ok((db, note))
    });
    match read {
        Ok((db, note)) => (Some(db), note, None),
        Err(e) => (
            None,
            String::new(),
            Some(format!("Notes are not kept: {e}")),
        ),
    }
}

/// What the example answers to a request: one place, and nowhere else.
fn canned(request: &Data) -> Result<Data, String> {
    let Data::Map(fields) = request else {
        return Err("not a request".into());
    };
    match fields.get("url") {
        Some(Data::Text(url)) if url == "https://quotes.example/random" => Ok(Data::Text(
            "Make it work, make it right, make it fast.".into(),
        )),
        _ => Err("this example makes no real requests".into()),
    }
}

/// Writes the sample Plugins into `plugins` when there is nothing in it.
fn install_samples(plugins: &Path) {
    let empty = std::fs::read_dir(plugins).map_or(true, |mut d| d.next().is_none());
    if !empty {
        return;
    }
    for (name, files) in SAMPLES {
        let dir = plugins.join(name);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        for (file, text) in *files {
            let _ = std::fs::write(dir.join(file), text);
        }
    }
}

/// The page: the note, the Actions menu with what the Plugins added, and the status line.
pub fn view(hosted: &Hosted<Notes>, cx: &mut Cx<Hosted<Notes>>) -> El {
    let app = &hosted.app;
    let mut layers = Vec::new();
    let layout = Layout::of(hosted);

    let note = Input::new(
        "note",
        "Note",
        |app: &mut Notes, text: String| app.set_note(text),
        |_: &mut Notes| {},
    )
    .placeholder("Write a note")
    .default_value(app.note.clone())
    .show(hosted);

    let mut menu = Menu::<Notes>::new();
    for item in app.contributions() {
        let Data::Map(description) = &item.description else {
            continue;
        };
        let Some(Data::Text(label)) = description.get("label") else {
            continue;
        };
        let (plugin, id) = (item.plugin.clone(), item.id);
        menu = menu.item(
            MenuItem::new(label.clone())
                .on_select(move |app: &mut Notes| app.use_item(&plugin, id)),
        );
    }
    menu = menu
        .separator()
        .item(MenuItem::new("Clear note").on_select(|app: &mut Notes| {
            app.set_note(String::new());
            app.status.clear();
        }));
    let actions = DropdownMenu::<Notes>::new("actions", "Actions")
        .menu(menu)
        .open(app.menu)
        .on_open_change(|app: &mut Notes, open| app.menu = open)
        .show(hosted, &mut layers);

    let mut body = layout.stack(Space::Md).key("page").child(note);
    if let Some(notice) = app.notice() {
        body = body.child(
            Alert::new("unsaved", notice)
                .tone(Tone::Danger)
                .show(hosted),
        );
    }
    let body = body
        .child(actions)
        .child(Text::new(app.status.clone()).show(hosted));
    frame(hosted, cx, body, layers)
}

/// The application's data directory. Under `odk oma run` the program is at `/app` and its data
/// directory is `$HOME` (`docs/spec/sandbox-baseline.md`); run by hand, it is a directory of its
/// own in the user's data home.
pub fn data_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let sandboxed = std::env::current_exe().is_ok_and(|exe| exe.starts_with("/app"));
    if sandboxed && let Some(home) = home {
        return home;
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("odk-examples/dev.example.notes")
}
