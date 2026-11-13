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

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use odk::plugin::{CallError, Contribution, Data, Grants, Host, Limits, NetworkRequest};
use odk::prelude::*;
use odk::ui::{DropdownMenu, Input, Menu, MenuItem, Text};

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

/// The application's state.
pub struct Notes {
    pub note: String,
    pub status: String,
    pub menu: bool,
    host: Host,
    /// What the Plugins asked for and nobody has answered.
    requests: Rc<RefCell<Vec<NetworkRequest>>>,
    /// What the host reported of the Plugins since the status line last showed it.
    reports: Rc<RefCell<Vec<String>>>,
}

impl Notes {
    /// The application with the Plugins in `<data_dir>/plugins` (the samples are written there
    /// first when it holds none). `data_dir` is [`data_dir`]: `$HOME` inside the sandbox, which is
    /// the pair's data directory on the host (ADR 0099).
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
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
            note: String::new(),
            status: String::new(),
            menu: false,
            host,
            requests,
            reports,
        }
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
        |app: &mut Notes, text: String| app.note = text,
        |_: &mut Notes| {},
    )
    .placeholder("Write a note")
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
            app.note.clear();
            app.status.clear();
        }));
    let actions = DropdownMenu::<Notes>::new("actions", "Actions")
        .menu(menu)
        .open(app.menu)
        .on_open_change(|app: &mut Notes, open| app.menu = open)
        .show(hosted, &mut layers);

    let body = layout
        .stack(Space::Md)
        .key("page")
        .child(note)
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
