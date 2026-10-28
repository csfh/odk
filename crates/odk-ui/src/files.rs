//! The desktop's file chooser, opened by `odkd` for the application (ADR 0068), so that a
//! sandboxed application, which has no session bus, can still ask the user for a file or a
//! folder.

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use gpui::App;
use odk_theme::chooser::{
    ChooserAnswer, ChooserKind, ChooserRequest, FileFilter, MAX_ANSWER, files_socket_path,
};

pub use odk_theme::chooser::StartFolder;

/// Asks the user to choose a file to open, a file to save or a folder, in the desktop's own file
/// chooser, which `odkd` opens outside the sandbox. The answer is a path, a name and not access:
/// a sandboxed application still reads only what its permissions grant, so a Bundle hands the
/// path to its daemon (ADR 0050), which runs outside the sandbox.
///
/// ```ignore
/// FileChooser::open("Choose a face")
///     .filter("Images", ["*.png", "*.jpg", "*.jpeg"])
///     .start_in(StartFolder::Pictures)
///     .choose(cx, |chosen, cx| match chosen {
///         Ok(Some(paths)) => { /* the user chose these */ }
///         Ok(None) => { /* the user cancelled */ }
///         Err(e) => { /* no dialog was shown: say why, with a toast */ }
///     });
/// ```
///
/// The dialog is the desktop's: it is a window of its own, not attached to the application's,
/// and the keyboard works in it as the desktop's chooser has it.
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

    /// Shows the dialog and calls `done` on the application once the user has chosen
    /// (`Ok(Some(paths))`) or cancelled (`Ok(None)`), or at once when no dialog can be shown
    /// (`Err`): there is no `odkd`, or it refused the request. The window stays live meanwhile.
    pub fn choose(
        self,
        cx: &mut App,
        done: impl FnOnce(Result<Option<Vec<PathBuf>>, FileChooserError>, &mut App) + 'static,
    ) {
        choose_at(files_socket_path(), self.request, cx, done);
    }
}

/// [`FileChooser::choose`] on the socket at `socket`: on a thread of its own, since the user may
/// take as long as they like, and back on the application with the answer.
fn choose_at(
    socket: Option<PathBuf>,
    request: ChooserRequest,
    cx: &mut App,
    done: impl FnOnce(Result<Option<Vec<PathBuf>>, FileChooserError>, &mut App) + 'static,
) {
    let (tx, rx) = async_channel::bounded(1);
    let asked = std::thread::Builder::new()
        .name("odk-file-chooser".into())
        .spawn(move || {
            let _ = tx.send_blocking(ask(socket, &request));
        });
    if let Err(e) = asked {
        done(Err(FileChooserError::Failed(e.to_string())), cx);
        return;
    }
    cx.spawn(async move |cx| {
        let answer = rx
            .recv()
            .await
            .unwrap_or_else(|_| Err(FileChooserError::Failed("no answer came".into())));
        let _ = cx.update(|cx| done(answer, cx));
    })
    .detach();
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

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::*;

    /// A stand-in for `odkd`'s `files.sock` that answers one request with `answer` and keeps the
    /// request it read.
    fn fake_odkd(dir: &Path, answer: ChooserAnswer) -> (PathBuf, Arc<Mutex<String>>) {
        let path = dir.join("files.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let kept = seen.clone();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            *kept.lock().unwrap() = line;
            writeln!(stream, "{}", answer.to_line()).unwrap();
        });
        (path, seen)
    }

    #[test]
    fn the_request_is_what_the_builder_says() {
        let chooser = FileChooser::save("Save diagnostic report")
            .name("omarchy-debug.txt")
            .filter("Text", ["*.txt"])
            .accept_label("Save")
            .start_in(StartFolder::Pictures)
            .multiple(true);
        let r = &chooser.request;
        assert_eq!(r.kind, ChooserKind::Save);
        assert!(!r.multiple, "a save dialog chooses one file");
        assert_eq!(r.name.as_deref(), Some("omarchy-debug.txt"));
        assert_eq!(r.filters[0].patterns, ["*.txt"]);
        assert_eq!(r.start_in, Some(StartFolder::Pictures));
        assert!(FileChooser::folder("Share").multiple(true).request.multiple);
    }

    #[test]
    fn an_answer_is_the_paths_a_cancel_or_why_not() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = ChooserAnswer::Chosen(vec!["/home/me/face.png".into()]);
        let (socket, seen) = fake_odkd(dir.path(), chosen);
        let request = FileChooser::open("Choose a face").request;
        assert_eq!(
            ask(Some(socket), &request),
            Ok(Some(vec!["/home/me/face.png".into()]))
        );
        let sent = seen.lock().unwrap().clone();
        assert_eq!(
            ChooserRequest::from_line(sent.trim_end()),
            Ok(request.clone())
        );

        let dir = tempfile::tempdir().unwrap();
        let (socket, _) = fake_odkd(dir.path(), ChooserAnswer::Cancelled);
        assert_eq!(ask(Some(socket), &request), Ok(None));

        let dir = tempfile::tempdir().unwrap();
        let busy = ChooserAnswer::Failed("the file chooser is busy".into());
        let (socket, _) = fake_odkd(dir.path(), busy);
        assert_eq!(
            ask(Some(socket), &request),
            Err(FileChooserError::Failed("the file chooser is busy".into()))
        );
    }

    #[test]
    fn without_odkd_there_is_no_dialog() {
        let request = FileChooser::open("Open").request;
        assert_eq!(ask(None, &request), Err(FileChooserError::NoHost));
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("files.sock");
        assert_eq!(ask(Some(missing), &request), Err(FileChooserError::NoHost));
    }

    #[gpui::test]
    fn the_answer_comes_back_on_the_application(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let chosen = ChooserAnswer::Chosen(vec!["/home/me/Shared".into()]);
        let (socket, _) = fake_odkd(dir.path(), chosen);
        let got = Arc::new(Mutex::new(None));
        let kept = got.clone();
        cx.update(|cx| {
            choose_at(
                Some(socket),
                FileChooser::folder("Share a folder").request,
                cx,
                move |answer, _| *kept.lock().unwrap() = Some(answer),
            )
        });
        cx.executor().allow_parking();
        let start = std::time::Instant::now();
        while got.lock().unwrap().is_none() {
            assert!(start.elapsed().as_secs() < 10, "no answer");
            cx.run_until_parked();
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            got.lock().unwrap().take(),
            Some(Ok(Some(vec!["/home/me/Shared".into()])))
        );
    }
}
