//! The file chooser (ADR 0068): one request per connection on `files.sock`, answered with the
//! paths the user chose in the desktop's dialog, which `odkd` asks for outside every sandbox.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use odk_theme::chooser::{
    ChooserAnswer, ChooserRequest, MAX_OPEN, MAX_REQUEST, REQUEST_WAIT, StartFolder,
};

/// What shows the dialog: the portal, or a fake in tests.
pub trait Chooser: Send + Sync + 'static {
    /// Shows the dialog `request` asks for, starting in `start` when there is one, and waits for
    /// the user.
    fn choose(&self, request: &ChooserRequest, start: Option<&Path>) -> ChooserAnswer;
}

/// Where a named folder is: the user's home and their `user-dirs.dirs`.
#[derive(Debug, Clone, Default)]
pub struct Folders {
    pub home: Option<PathBuf>,
    pub user_dirs: Option<PathBuf>,
}

impl Folders {
    pub fn from_environment() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|h| h.is_absolute());
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|c| c.is_absolute())
            .or_else(|| home.as_ref().map(|h| h.join(".config")));
        Self {
            home,
            user_dirs: config.map(|c| c.join("user-dirs.dirs")),
        }
    }

    /// The folder to start in, when it is a directory: a path as given, the home folder, or a
    /// named one from `user-dirs.dirs`, which is the home folder when it names none.
    pub fn resolve(&self, start: &StartFolder) -> Option<PathBuf> {
        let dir = match start {
            StartFolder::Path(path) => Some(path.clone()),
            StartFolder::Home => self.home.clone(),
            named => named
                .user_dirs_key()
                .and_then(|key| self.user_dir(key))
                .or_else(|| self.home.clone()),
        }?;
        dir.is_dir().then_some(dir)
    }

    /// `XDG_<NAME>_DIR="…"` in `user-dirs.dirs`, with `$HOME` expanded, as `xdg-user-dirs`
    /// writes it.
    fn user_dir(&self, key: &str) -> Option<PathBuf> {
        let text = fs::read_to_string(self.user_dirs.as_ref()?).ok()?;
        let value = text.lines().find_map(|line| {
            let rest = line.trim().strip_prefix(key)?.strip_prefix('=')?;
            Some(rest.trim().trim_matches('"').to_owned())
        })?;
        let path = match value.strip_prefix("$HOME") {
            Some(rest) => self.home.as_ref()?.join(rest.trim_start_matches('/')),
            None => PathBuf::from(value),
        };
        path.is_absolute().then_some(path)
    }
}

/// Accepts connections on a thread of their own, each answered on a thread of its own.
pub fn serve_in_background(listener: UnixListener, chooser: Arc<dyn Chooser>, folders: Folders) {
    let server = Arc::new(Server {
        chooser,
        folders,
        open: AtomicUsize::new(0),
        wait: REQUEST_WAIT,
    });
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let server = server.clone();
                    thread::spawn(move || server.answer(stream));
                }
                Err(e) => eprintln!("odkd: accept failed on the file chooser: {e}"),
            }
        }
    });
}

struct Server {
    chooser: Arc<dyn Chooser>,
    folders: Folders,
    /// The dialogs open now, for every application together.
    open: AtomicUsize,
    /// How long a client has to send its request.
    wait: Duration,
}

impl Server {
    /// Reads one request from `stream`, shows the dialog and writes the answer. A client that
    /// says nothing in time, or more than a request, is closed.
    fn answer(&self, mut stream: UnixStream) {
        let answer = match self.read_request(&stream) {
            Ok(request) => self.show(&request),
            Err(why) => ChooserAnswer::Failed(why),
        };
        // A client that has gone is not told; there is nothing more to do for it.
        let _ = writeln!(stream, "{}", answer.to_line());
    }

    fn read_request(&self, stream: &UnixStream) -> Result<ChooserRequest, String> {
        stream
            .set_read_timeout(Some(self.wait))
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        BufReader::new(stream.take(MAX_REQUEST as u64 + 1))
            .read_line(&mut line)
            .map_err(|_| "no request came".to_owned())?;
        if !line.ends_with('\n') {
            return Err("the request is not one line, or is too long".into());
        }
        ChooserRequest::from_line(line.trim_end()).map_err(|e| e.to_string())
    }

    fn show(&self, request: &ChooserRequest) -> ChooserAnswer {
        let Some(_slot) = Slot::take(&self.open) else {
            return ChooserAnswer::Failed(format!(
                "the file chooser is busy: {MAX_OPEN} dialogs are open"
            ));
        };
        let start = request
            .start_in
            .as_ref()
            .and_then(|s| self.folders.resolve(s));
        self.chooser.choose(request, start.as_deref())
    }
}

/// One of the [`MAX_OPEN`] dialogs, given back when it is dropped.
struct Slot<'a>(&'a AtomicUsize);

impl<'a> Slot<'a> {
    fn take(open: &'a AtomicUsize) -> Option<Self> {
        // Taken, then given back at once when there was no room: the count may pass the limit
        // for that moment, but no fifth dialog is ever shown.
        let slot = Self(open);
        (open.fetch_add(1, Ordering::AcqRel) < MAX_OPEN).then_some(slot)
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Condvar, Mutex};

    use odk_theme::chooser::ChooserKind;

    use super::*;

    /// Answers every request with one path, after waiting for `release` when it is set.
    #[derive(Default)]
    struct Fake {
        asked: Mutex<Vec<(ChooserRequest, Option<PathBuf>)>>,
        hold: Mutex<bool>,
        released: Condvar,
    }

    impl Chooser for Fake {
        fn choose(&self, request: &ChooserRequest, start: Option<&Path>) -> ChooserAnswer {
            self.asked
                .lock()
                .unwrap()
                .push((request.clone(), start.map(Path::to_owned)));
            let mut hold = self.hold.lock().unwrap();
            while *hold {
                hold = self.released.wait(hold).unwrap();
            }
            ChooserAnswer::Chosen(vec!["/home/me/face.png".into()])
        }
    }

    fn server(fake: Arc<Fake>, folders: Folders) -> Arc<Server> {
        Arc::new(Server {
            chooser: fake,
            folders,
            open: AtomicUsize::new(0),
            wait: Duration::from_millis(200),
        })
    }

    /// Sends `request` (a raw line, newline and all) to a server thread and reads its answer.
    fn ask(server: &Arc<Server>, request: &str) -> ChooserAnswer {
        let (mut client, end) = UnixStream::pair().unwrap();
        let s = server.clone();
        let t = thread::spawn(move || s.answer(end));
        client.write_all(request.as_bytes()).unwrap();
        let mut line = String::new();
        BufReader::new(&client).read_line(&mut line).unwrap();
        t.join().unwrap();
        ChooserAnswer::from_line(line.trim_end()).unwrap()
    }

    #[test]
    fn a_request_is_shown_and_the_choice_is_the_answer() {
        let fake = Arc::new(Fake::default());
        let s = server(fake.clone(), Folders::default());
        let request = ChooserRequest::new(ChooserKind::Open, "Choose a face");
        let answer = ask(&s, &format!("{}\n", request.to_line()));
        assert_eq!(
            answer,
            ChooserAnswer::Chosen(vec!["/home/me/face.png".into()])
        );
        assert_eq!(fake.asked.lock().unwrap()[0].0, request);
    }

    #[test]
    fn a_bad_request_is_answered_and_never_shown() {
        let fake = Arc::new(Fake::default());
        let s = server(fake.clone(), Folders::default());
        for bad in [
            "not json\n".to_owned(),
            r#"{"v":1,"kind":"save","multiple":true}"#.to_owned() + "\n",
            format!("{}\n", "x".repeat(MAX_REQUEST + 10)),
            "no newline".to_owned(),
        ] {
            assert!(
                matches!(ask(&s, &bad), ChooserAnswer::Failed(_)),
                "{}",
                &bad[..bad.len().min(40)]
            );
        }
        assert!(fake.asked.lock().unwrap().is_empty());
    }

    #[test]
    fn a_client_that_says_nothing_is_answered_after_the_wait() {
        let s = server(Arc::new(Fake::default()), Folders::default());
        let (client, end) = UnixStream::pair().unwrap();
        let t = thread::spawn(move || s.answer(end));
        let mut line = String::new();
        BufReader::new(&client).read_line(&mut line).unwrap();
        t.join().unwrap();
        assert!(matches!(
            ChooserAnswer::from_line(line.trim_end()),
            Ok(ChooserAnswer::Failed(_))
        ));
    }

    #[test]
    fn at_most_four_dialogs_are_open_and_a_fifth_is_told_so_at_once() {
        let fake = Arc::new(Fake::default());
        *fake.hold.lock().unwrap() = true;
        let s = server(fake.clone(), Folders::default());
        let line = format!(
            "{}\n",
            ChooserRequest::new(ChooserKind::Open, "Open").to_line()
        );
        let waiting: Vec<_> = (0..MAX_OPEN)
            .map(|_| {
                let (s, line) = (s.clone(), line.clone());
                thread::spawn(move || ask(&s, &line))
            })
            .collect();
        while fake.asked.lock().unwrap().len() < MAX_OPEN {
            thread::yield_now();
        }
        assert!(matches!(ask(&s, &line), ChooserAnswer::Failed(why) if why.contains("busy")));
        *fake.hold.lock().unwrap() = false;
        fake.released.notify_all();
        for w in waiting {
            assert!(matches!(w.join().unwrap(), ChooserAnswer::Chosen(_)));
        }
        assert!(
            matches!(ask(&s, &line), ChooserAnswer::Chosen(_)),
            "room again"
        );
    }

    #[test]
    fn a_named_folder_is_the_users_and_falls_back_to_home() {
        let home = tempfile::tempdir().unwrap();
        let pictures = home.path().join("Pictures");
        fs::create_dir(&pictures).unwrap();
        let dirs = home.path().join("user-dirs.dirs");
        fs::write(
            &dirs,
            "# written by xdg-user-dirs-update\nXDG_PICTURES_DIR=\"$HOME/Pictures\"\nXDG_MUSIC_DIR=\"/nowhere/Music\"\n",
        )
        .unwrap();
        let folders = Folders {
            home: Some(home.path().to_owned()),
            user_dirs: Some(dirs),
        };
        assert_eq!(folders.resolve(&StartFolder::Pictures), Some(pictures));
        let home_dir = Some(home.path().to_owned());
        assert_eq!(folders.resolve(&StartFolder::Home), home_dir);
        assert_eq!(
            folders.resolve(&StartFolder::Documents),
            home_dir,
            "not named"
        );
        assert_eq!(
            folders.resolve(&StartFolder::Music),
            None,
            "not a directory"
        );
        let given = StartFolder::Path(home.path().join("missing"));
        assert_eq!(folders.resolve(&given), None);
    }

    #[test]
    fn the_folder_is_resolved_before_the_dialog_is_shown() {
        let home = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let folders = Folders {
            home: Some(home.path().to_owned()),
            user_dirs: None,
        };
        let s = server(fake.clone(), folders);
        let mut request = ChooserRequest::new(ChooserKind::Save, "Save report");
        request.start_in = Some(StartFolder::Downloads);
        ask(&s, &format!("{}\n", request.to_line()));
        let asked = fake.asked.lock().unwrap();
        assert_eq!(asked[0].1.as_deref(), Some(home.path()));
    }
}
