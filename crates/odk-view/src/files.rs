//! The desktop's file chooser, opened by `odkd` for the application (ADR 0068), so that a
//! sandboxed application, which has no session bus, can still ask the user for a file or a folder.
//!
//! The answer comes back on a thread of its own and is handed to the application's state through
//! Rune's `AppHandle::update`, which [`crate::run`] installs for the process.

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use crate::host::Hosted;
use crate::post::{self, Post};
pub use odk_theme::chooser::StartFolder;
use odk_theme::chooser::{
    ChooserAnswer, ChooserKind, ChooserRequest, FileFilter, MAX_ANSWER, files_socket_path,
};

/// Asks the user to choose a file to open, a file to save or a folder, in the desktop's own file
/// chooser, which `odkd` opens outside the sandbox. The answer is a path, a name and not access:
/// a sandboxed application still reads only what its permissions grant, so a Bundle hands the path
/// to its daemon (ADR 0050), which runs outside the sandbox.
///
/// ```ignore
/// FileChooser::open("Choose a face")
///     .filter("Images", ["*.png", "*.jpg", "*.jpeg"])
///     .start_in(StartFolder::Pictures)
///     .choose(|app: &mut App, chosen| match chosen {
///         Ok(Some(paths)) => { /* the user chose these */ }
///         Ok(None) => { /* the user cancelled */ }
///         Err(e) => { /* no dialog was shown: say why, with a toast */ }
///     })?;
/// ```
///
/// The dialog is the desktop's: a window of its own, not attached to the application's, and the
/// keyboard works in it as the desktop's chooser has it.
#[derive(Debug, Clone, PartialEq)]
pub struct FileChooser {
    request: ChooserRequest,
}

impl FileChooser {
    /// A dialog that chooses one existing file (several with `multiple`), titled `title`.
    pub fn open(title: impl Into<String>) -> Self {
        Self::new(ChooserKind::Open, title)
    }

    /// A dialog that chooses a file to write, which may not exist yet.
    pub fn save(title: impl Into<String>) -> Self {
        Self::new(ChooserKind::Save, title)
    }

    /// A dialog that chooses a folder (several with `multiple`).
    pub fn folder(title: impl Into<String>) -> Self {
        Self::new(ChooserKind::Folder, title)
    }

    fn new(kind: ChooserKind, title: impl Into<String>) -> Self {
        Self {
            request: ChooserRequest::new(kind, title),
        }
    }

    /// Several files or folders may be chosen. A save dialog always chooses one.
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.request.multiple = multiple && self.request.kind != ChooserKind::Save;
        self
    }

    /// A name filter the user can pick, such as `filter("Images", ["*.png", "*.jpg"])`. The first
    /// is the one shown first.
    pub fn filter(
        mut self,
        label: impl Into<String>,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.request.filters.push(FileFilter {
            label: label.into(),
            patterns: patterns.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// The label of the button that chooses, such as "Set background".
    pub fn accept_label(mut self, label: impl Into<String>) -> Self {
        self.request.accept_label = Some(label.into());
        self
    }

    /// The file name a save dialog suggests, such as `omarchy-debug.txt`.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.request.name = Some(name.into());
        self
    }

    /// Where the dialog starts: one of the user's folders, which `odkd` looks up outside the
    /// sandbox, or a path the application was given. A folder that is not there is left out.
    pub fn start_in(mut self, folder: StartFolder) -> Self {
        self.request.start_in = Some(folder);
        self
    }

    /// Shows the dialog and calls `done` on the application's state once the user has chosen
    /// (`Ok(Some(paths))`) or cancelled (`Ok(None)`), or when no dialog can be shown (`Err`): there
    /// is no `odkd`, or it refused the request. The window stays live meanwhile. Returns an error
    /// at once, and never calls `done`, when the application did not start through
    /// [`crate::run`], so there is no window to answer to.
    pub fn choose<T: 'static>(
        self,
        done: impl FnOnce(&mut T, Result<Option<Vec<PathBuf>>, FileChooserError>) + Send + 'static,
    ) -> Result<(), FileChooserError> {
        let post = post::current::<T>()
            .ok_or_else(|| FileChooserError::Failed("there is no window to answer to".into()))?;
        choose_at(files_socket_path(), self.request, post, done);
        Ok(())
    }
}

/// [`FileChooser::choose`] on the socket at `socket`: on a thread of its own, since the user may
/// take as long as they like, and back on the application with the answer through `post`.
fn choose_at<T: 'static>(
    socket: Option<PathBuf>,
    request: ChooserRequest,
    post: Post,
    done: impl FnOnce(&mut T, Result<Option<Vec<PathBuf>>, FileChooserError>) + Send + 'static,
) {
    let spawned = std::thread::Builder::new()
        .name("odk-file-chooser".into())
        .spawn(move || {
            let answer = ask(socket, &request);
            post::send::<T>(&post, move |host: &mut Hosted<T>| {
                done(&mut host.app, answer)
            });
        });
    // A thread that cannot start is an answer too, but there is no state to give it to from
    // here; the caller saw `Ok` and the window simply never hears. It is the same as a refused
    // request, and rare enough not to carry a second path.
    drop(spawned);
}

/// Asks `odkd` on `socket` and waits for the user.
fn ask(
    socket: Option<PathBuf>,
    request: &ChooserRequest,
) -> Result<Option<Vec<PathBuf>>, FileChooserError> {
    let socket = socket.ok_or(FileChooserError::NoHost)?;
    let mut stream = UnixStream::connect(&socket).map_err(|_| FileChooserError::NoHost)?;
    writeln!(stream, "{}", request.to_line())
        .map_err(|e| FileChooserError::Failed(e.to_string()))?;
    let mut line = String::new();
    BufReader::new(stream.take(MAX_ANSWER))
        .read_line(&mut line)
        .map_err(|e| FileChooserError::Failed(e.to_string()))?;
    if line.is_empty() {
        return Err(FileChooserError::Failed(
            "odkd closed without an answer".into(),
        ));
    }
    match ChooserAnswer::from_line(line.trim_end())
        .map_err(|e| FileChooserError::Failed(e.to_string()))?
    {
        ChooserAnswer::Chosen(paths) => Ok(Some(paths)),
        ChooserAnswer::Cancelled => Ok(None),
        ChooserAnswer::Failed(why) => Err(FileChooserError::Failed(why)),
    }
}

/// Why no file chooser was shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChooserError {
    /// There is no `odkd` to open it, or it is older than the file chooser (ADR 0068).
    NoHost,
    /// `odkd` was asked and could not show one, and says why: too many are open, the request was
    /// out of bounds, or the desktop has no file chooser.
    Failed(String),
}

impl fmt::Display for FileChooserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHost => f.write_str("there is no odkd to open a file chooser"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for FileChooserError {}

/// The request this builder makes, for a test.
#[cfg(test)]
impl FileChooser {
    pub(crate) fn request(&self) -> &ChooserRequest {
        &self.request
    }
}

#[cfg(test)]
pub(crate) fn choose_for_test<T: 'static>(
    socket: Option<PathBuf>,
    chooser: FileChooser,
    post: Post,
    done: impl FnOnce(&mut T, Result<Option<Vec<PathBuf>>, FileChooserError>) + Send + 'static,
) {
    choose_at(socket, chooser.request, post, done);
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::face::Face;

    const WAIT: Duration = Duration::from_secs(5);

    /// A stand-in for `odkd`'s `files.sock` that answers one request with `answer` and keeps the
    /// request it read.
    fn fake_odkd(dir: &Path, answer: ChooserAnswer) -> (PathBuf, Arc<Mutex<String>>) {
        let path = dir.join("files.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let kept = seen.clone();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            *kept.lock().unwrap() = line;
            writeln!(stream, "{}", answer.to_line()).unwrap();
        });
        (path, seen)
    }

    /// A directory of its own, removed when the test ends.
    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("odk-view-files-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The application's state in these tests, and a poster that runs a job on it at once, on the
    /// thread that posts, as Rune's own would on the window's.
    #[derive(Default)]
    struct App {
        heard: Vec<Result<Option<Vec<PathBuf>>, FileChooserError>>,
    }

    fn answer_to_app(
        request: FileChooser,
        socket: Option<PathBuf>,
    ) -> Result<Option<Vec<PathBuf>>, FileChooserError> {
        let (tx, rx) = channel::<Result<Option<Vec<PathBuf>>, FileChooserError>>();
        // As Rune's own poster would: the job goes to the thread that owns the host, which runs
        // it there.
        let (jobs, queue) = channel::<Box<dyn FnOnce(&mut dyn Any) + Send>>();
        let post: Post = Arc::new(move |job| {
            let _ = jobs.send(job);
        });
        let mut host = Hosted::new(
            App::default(),
            odk_theme::ThemeSnapshot::emergency(),
            Face::bundled(),
        );
        choose_for_test::<App>(socket, request, post, move |app, answer| {
            app.heard.push(answer.clone());
            let _ = tx.send(answer);
        });
        let job = queue.recv_timeout(WAIT).expect("a job for the host");
        job(&mut host as &mut dyn Any);
        let answer = rx.recv_timeout(WAIT).expect("an answer");
        assert_eq!(
            host.app.heard.len(),
            1,
            "it reached the application's state, once"
        );
        answer
    }

    #[test]
    fn the_request_is_what_the_builder_says() {
        let chooser = FileChooser::save("Save diagnostic report")
            .name("omarchy-debug.txt")
            .filter("Text", ["*.txt"])
            .accept_label("Save")
            .start_in(StartFolder::Pictures)
            .multiple(true);
        let r = chooser.request();
        assert_eq!(r.kind, ChooserKind::Save);
        assert!(!r.multiple, "a save dialog chooses one file");
        assert_eq!(r.name.as_deref(), Some("omarchy-debug.txt"));
        assert_eq!(r.filters[0].patterns, ["*.txt"]);
        assert_eq!(r.start_in, Some(StartFolder::Pictures));
        assert!(
            FileChooser::folder("Share")
                .multiple(true)
                .request()
                .multiple
        );
    }

    #[test]
    fn an_answer_reaches_the_applications_state_as_the_paths_a_cancel_or_why_not() {
        let dir = Dir::new("answers");
        let chosen = ChooserAnswer::Chosen(vec!["/home/me/face.png".into()]);
        let (socket, seen) = fake_odkd(&dir.0, chosen);
        let chooser = FileChooser::open("Choose a face");
        assert_eq!(
            answer_to_app(chooser.clone(), Some(socket)),
            Ok(Some(vec!["/home/me/face.png".into()]))
        );
        let sent = seen.lock().unwrap().clone();
        assert_eq!(
            ChooserRequest::from_line(sent.trim_end()),
            Ok(chooser.request().clone())
        );

        let dir = Dir::new("cancelled");
        let (socket, _) = fake_odkd(&dir.0, ChooserAnswer::Cancelled);
        assert_eq!(answer_to_app(chooser.clone(), Some(socket)), Ok(None));

        let dir = Dir::new("busy");
        let busy = ChooserAnswer::Failed("the file chooser is busy".into());
        let (socket, _) = fake_odkd(&dir.0, busy);
        assert_eq!(
            answer_to_app(chooser, Some(socket)),
            Err(FileChooserError::Failed("the file chooser is busy".into()))
        );
    }

    #[test]
    fn no_odkd_is_an_answer_and_not_a_hang() {
        let chooser = FileChooser::open("Choose");
        assert_eq!(
            answer_to_app(chooser.clone(), None),
            Err(FileChooserError::NoHost)
        );
        let dir = Dir::new("nosocket");
        assert_eq!(
            answer_to_app(chooser, Some(dir.0.join("files.sock"))),
            Err(FileChooserError::NoHost)
        );
    }

    #[test]
    fn without_a_window_there_is_nothing_to_answer_to() {
        // Nothing here starts a window, so no poster was installed for the process.
        let started = FileChooser::open("Choose").choose::<App>(|_, _| unreachable!("no window"));
        assert!(matches!(started, Err(FileChooserError::Failed(_))));
    }
}
