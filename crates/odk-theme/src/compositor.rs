//! The compositor's animation setting, which is the user's request for reduced motion
//! (ADR 0048): Hyprland's `animations:enabled`, asked over its IPC socket and followed on its
//! event socket. Nothing here writes to the compositor or sends it a command but the one
//! question, and a compositor that is missing, slow or not understood says nothing.
//!
//! Where the sockets are is a value ([`Hyprland::at`]), not a reach into the environment, so a
//! test points it at a socket of its own and never at the machine's compositor.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long to wait for the compositor to take a question or answer it. The first reload runs
/// before any window exists, so a compositor that stalls costs a start at most this long.
const PATIENCE: Duration = Duration::from_millis(250);
/// How long to wait before looking for a compositor that went away, and how long at most between
/// looks: a restart takes a moment, and a compositor that does not come back is looked for only
/// now and then.
const RECONNECT_FROM: Duration = Duration::from_millis(200);
const RECONNECT_UP_TO: Duration = Duration::from_secs(5);
/// The most of a reply that is read; `getoption` answers in under a hundred bytes.
const MOST_OF_A_REPLY: u64 = 4096;
const QUESTION: &[u8] = b"j/getoption animations:enabled";
/// What the event socket says when the compositor has read its configuration again.
const RECONFIGURED: &str = "configreloaded>>";

/// A running Hyprland, found by where its sockets would be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyprland {
    /// `$XDG_RUNTIME_DIR`; the sockets are under `hypr/<instance>/` in it.
    runtime_dir: PathBuf,
    /// `$HYPRLAND_INSTANCE_SIGNATURE`: which instance, when the environment says.
    signature: Option<OsString>,
}

impl Hyprland {
    /// The Hyprland of this process's environment, or `None` outside a session that has a
    /// runtime directory.
    pub fn from_environment() -> Option<Self> {
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
        Some(Self::at(
            runtime_dir,
            std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE"),
        ))
    }

    /// The Hyprland under `runtime_dir`, of instance `signature` when that is known. When it is
    /// not (a systemd user service has no `HYPRLAND_INSTANCE_SIGNATURE` unless it is imported),
    /// every instance with a socket is tried, the newest first.
    pub fn at(runtime_dir: impl Into<PathBuf>, signature: Option<OsString>) -> Self {
        Self {
            runtime_dir: runtime_dir.into(),
            signature: signature.filter(|s| !s.is_empty()),
        }
    }

    /// The instance directories to try, in order: the one the environment names, then the rest
    /// that have a socket, newest first.
    fn instances(&self) -> Vec<PathBuf> {
        let hypr = self.runtime_dir.join("hypr");
        let named = self.signature.as_ref().map(|s| hypr.join(s));
        let mut found: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&hypr)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|dir| Some(dir) != named.as_ref())
            .filter_map(|dir| {
                // Either socket makes it an instance: the questions and the events have one each.
                let modified = [".socket.sock", ".socket2.sock"]
                    .iter()
                    .find_map(|name| std::fs::metadata(dir.join(name)).ok())?
                    .modified()
                    .ok()?;
                Some((modified, dir))
            })
            .collect();
        found.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        named
            .into_iter()
            .chain(found.into_iter().map(|(_, dir)| dir))
            .collect()
    }

    /// Whether the compositor animates windows: its `animations:enabled`. `None` when it cannot
    /// be asked or does not answer in a way that is understood.
    pub fn animations_enabled(&self) -> Option<bool> {
        self.instances()
            .iter()
            .find_map(|dir| ask(&dir.join(".socket.sock")))
    }

    /// Calls `on_reload` each time the compositor says it has read its configuration again, from
    /// a thread of its own, until the returned watcher is dropped. `None` when no Hyprland is
    /// expected at all: no instance's event socket can be reached, the environment names none and
    /// there is no `hypr` directory, which is a session of another compositor. When one is
    /// expected and does not answer yet, as while it restarts, the watcher looks for it as below.
    ///
    /// A compositor that goes away and comes back, as Hyprland does when it is restarted, is
    /// followed: when the connection ends `on_reload` is called, since the setting may be
    /// different now or unknowable, and the watcher then looks for an instance again, with a wait
    /// that doubles from [`RECONNECT_FROM`] to [`RECONNECT_UP_TO`], and calls `on_reload` once
    /// more when it has found one. The instance is looked for afresh each time, because the
    /// environment's `HYPRLAND_INSTANCE_SIGNATURE` names the one that has gone.
    pub fn watch_reloads(&self, on_reload: impl Fn() + Send + 'static) -> Option<ReloadWatcher> {
        let first = self.connect_events();
        if first.is_none() && !self.expected() {
            return None;
        }
        let open = Arc::new(Mutex::new(
            first.as_ref().and_then(|stream| stream.try_clone().ok()),
        ));
        let closing = Arc::new(AtomicBool::new(false));
        let (stop, stopped) = mpsc::channel::<()>();
        let (hyprland, slot, closing_too) = (self.clone(), open.clone(), closing.clone());
        std::thread::spawn(move || {
            let mut stream = first;
            let mut wait = RECONNECT_FROM;
            loop {
                if let Some(connected) = stream.take() {
                    follow(connected, &on_reload);
                    *slot.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    if closing_too.load(Ordering::SeqCst) {
                        return;
                    }
                    on_reload();
                    wait = RECONNECT_FROM;
                }
                if !matches!(stopped.recv_timeout(wait), Err(RecvTimeoutError::Timeout)) {
                    return;
                }
                match hyprland.connect_events() {
                    Some(connected) => {
                        *slot.lock().unwrap_or_else(|e| e.into_inner()) =
                            connected.try_clone().ok();
                        // Dropped while connecting: the close that dropping did missed this one.
                        if closing_too.load(Ordering::SeqCst) {
                            let _ = connected.shutdown(Shutdown::Both);
                            return;
                        }
                        on_reload();
                        stream = Some(connected);
                    }
                    None => wait = (wait * 2).min(RECONNECT_UP_TO),
                }
            }
        });
        Some(ReloadWatcher {
            open,
            closing,
            _stop: stop,
        })
    }

    /// Whether a Hyprland is meant to be there: the environment names an instance, or there is a
    /// `hypr` directory under the runtime directory.
    fn expected(&self) -> bool {
        self.signature.is_some() || self.runtime_dir.join("hypr").is_dir()
    }

    /// A connection to the event socket of the first instance that has one.
    fn connect_events(&self) -> Option<UnixStream> {
        self.instances()
            .iter()
            .find_map(|dir| UnixStream::connect(dir.join(".socket2.sock")).ok())
    }
}

/// Reads events from `stream` until it ends, calling `on_reload` for each reload.
fn follow(stream: UnixStream, on_reload: &impl Fn()) {
    for line in BufReader::new(stream).lines() {
        match line {
            Ok(line) if line.starts_with(RECONFIGURED) => on_reload(),
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

/// Follows a compositor's reloads, across its restarts, until dropped.
#[derive(Debug)]
pub struct ReloadWatcher {
    /// The connection being read, so that dropping can end the read.
    open: Arc<Mutex<Option<UnixStream>>>,
    closing: Arc<AtomicBool>,
    /// Dropped with the watcher, which wakes the thread if it is waiting to reconnect.
    _stop: mpsc::Sender<()>,
}

impl Drop for ReloadWatcher {
    /// Closes the connection, which ends the thread's read, and tells the thread not to go on.
    fn drop(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        if let Some(stream) = self.open.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

/// Asks the socket at `path` whether animations are enabled.
fn ask(path: &Path) -> Option<bool> {
    let mut stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(PATIENCE)).ok()?;
    stream.set_write_timeout(Some(PATIENCE)).ok()?;
    stream.write_all(QUESTION).ok()?;
    let mut reply = String::new();
    stream
        .take(MOST_OF_A_REPLY)
        .read_to_string(&mut reply)
        .ok()?;
    parse_reply(&reply)
}

/// The boolean of a `getoption` reply in JSON, such as `{"option": "animations:enabled",
/// "bool": true, "set": true }`. `None` for anything else, an error text included.
pub(crate) fn parse_reply(reply: &str) -> Option<bool> {
    let after = reply.split_once("\"bool\"")?.1.trim_start();
    let value = after.strip_prefix(':')?.trim_start();
    if value.starts_with("true") {
        Some(true)
    } else if value.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;

    use tempfile::TempDir;

    use super::*;

    const ENABLED: &str = r#"{"option": "animations:enabled", "bool": true, "set": true }"#;
    const DISABLED: &str = r#"{"option": "animations:enabled", "bool": false, "set": true }"#;

    #[test]
    fn a_reply_says_whether_animations_are_on() {
        assert_eq!(parse_reply(ENABLED), Some(true));
        assert_eq!(parse_reply(DISABLED), Some(false));
        assert_eq!(parse_reply("{\"bool\":false}"), Some(false));
        assert_eq!(parse_reply("{ \"bool\" :  true }"), Some(true));
        for not_an_answer in [
            "",
            "no such option",
            "{\"error\": \"invalid\"}",
            "{\"bool\": 1}",
            "{\"bool\": \"true\"}",
            "\"bool\"",
        ] {
            assert_eq!(parse_reply(not_an_answer), None, "{not_an_answer:?}");
        }
    }

    /// A runtime directory with `hypr/<signature>/.socket.sock` listening, answering `reply` to
    /// the one question it takes and sending what it was asked down `asked`.
    fn serve(
        runtime: &Path,
        signature: &str,
        reply: &'static str,
    ) -> (mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let dir = runtime.join("hypr").join(signature);
        std::fs::create_dir_all(&dir).unwrap();
        let listener = UnixListener::bind(dir.join(".socket.sock")).unwrap();
        let (tell, asked) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 256];
            let n = stream.read(&mut buffer).unwrap();
            tell.send(String::from_utf8_lossy(&buffer[..n]).into_owned())
                .unwrap();
            stream.write_all(reply.as_bytes()).unwrap();
        });
        (asked, thread)
    }

    #[test]
    fn it_asks_the_one_question_and_reads_the_answer() {
        let dir = TempDir::new().unwrap();
        let (asked, server) = serve(dir.path(), "abc", DISABLED);
        let hyprland = Hyprland::at(dir.path(), Some("abc".into()));
        assert_eq!(hyprland.animations_enabled(), Some(false));
        assert_eq!(asked.recv().unwrap(), "j/getoption animations:enabled");
        server.join().unwrap();
    }

    #[test]
    fn without_a_signature_the_instance_with_a_socket_is_found() {
        let dir = TempDir::new().unwrap();
        // A stale instance (a directory, no socket) and the live one.
        std::fs::create_dir_all(dir.path().join("hypr/stale")).unwrap();
        let (_asked, server) = serve(dir.path(), "live", ENABLED);
        let hyprland = Hyprland::at(dir.path(), None);
        assert_eq!(hyprland.animations_enabled(), Some(true));
        server.join().unwrap();
        // The same when the environment names an instance that is gone.
        let (_asked_too, server) = serve(dir.path(), "live2", DISABLED);
        let hyprland = Hyprland::at(dir.path(), Some("gone".into()));
        assert_eq!(hyprland.animations_enabled(), Some(false));
        server.join().unwrap();
    }

    #[test]
    fn a_compositor_that_is_not_there_says_nothing() {
        let dir = TempDir::new().unwrap();
        assert_eq!(Hyprland::at(dir.path(), None).animations_enabled(), None);
        assert_eq!(
            Hyprland::at(dir.path(), Some("abc".into())).animations_enabled(),
            None
        );
        // A socket file nothing listens on.
        let instance = dir.path().join("hypr/abc");
        std::fs::create_dir_all(&instance).unwrap();
        drop(UnixListener::bind(instance.join(".socket.sock")).unwrap());
        assert_eq!(
            Hyprland::at(dir.path(), Some("abc".into())).animations_enabled(),
            None
        );
        // A compositor that is not there at all is not waited for. One that is meant to be, an
        // instance directory with a socket nothing listens on, is: it may be starting.
        let nowhere = TempDir::new().unwrap();
        assert!(
            Hyprland::at(nowhere.path(), None)
                .watch_reloads(|| {})
                .is_none()
        );
        assert!(
            Hyprland::at(dir.path(), None)
                .watch_reloads(|| {})
                .is_some()
        );
    }

    #[test]
    fn a_compositor_that_does_not_answer_is_waited_for_a_moment_and_no_more() {
        let dir = TempDir::new().unwrap();
        let instance = dir.path().join("hypr/abc");
        std::fs::create_dir_all(&instance).unwrap();
        let listener = UnixListener::bind(instance.join(".socket.sock")).unwrap();
        let _silent = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(3));
            drop(stream);
        });
        let started = std::time::Instant::now();
        assert_eq!(
            Hyprland::at(dir.path(), Some("abc".into())).animations_enabled(),
            None
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_reload_is_heard_and_nothing_else_is() {
        let dir = TempDir::new().unwrap();
        let instance = dir.path().join("hypr/abc");
        std::fs::create_dir_all(&instance).unwrap();
        let events = UnixListener::bind(instance.join(".socket2.sock")).unwrap();
        let (heard, reloads) = mpsc::channel();
        let hyprland = Hyprland::at(dir.path(), Some("abc".into()));
        let watcher = hyprland
            .watch_reloads(move || heard.send(()).unwrap())
            .unwrap();
        let (mut stream, _) = events.accept().unwrap();
        stream
            .write_all(b"workspace>>2\nconfigreloaded>>\nactivewindow>>x,y\nconfigreloaded>>\n")
            .unwrap();
        for _ in 0..2 {
            reloads.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        assert!(
            reloads.recv_timeout(Duration::from_millis(100)).is_err(),
            "only the reloads count"
        );
        // Dropping the watcher closes the connection, so the compositor sees the end of it.
        drop(watcher);
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut rest = Vec::new();
        assert_eq!(stream.read_to_end(&mut rest).unwrap(), 0);
    }

    /// The connection `listener` gets within `seconds`, or a failed test: an accept that waits for
    /// ever would hang the run instead.
    fn accept_within(listener: &UnixListener, seconds: u64) -> UnixStream {
        listener.set_nonblocking(true).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(seconds);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    return stream;
                }
                Err(_) if std::time::Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(e) => panic!("nothing connected within {seconds} s: {e}"),
            }
        }
    }

    /// A restarted compositor is a new instance in a new directory, while this process's
    /// environment still names the old one (ADR 0048).
    #[test]
    fn a_restarted_compositor_is_found_and_followed_again() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("hypr/old");
        std::fs::create_dir_all(&old).unwrap();
        let events = UnixListener::bind(old.join(".socket2.sock")).unwrap();
        let (heard, reloads) = mpsc::channel();
        let hyprland = Hyprland::at(dir.path(), Some("old".into()));
        let watcher = hyprland
            .watch_reloads(move || heard.send(()).unwrap())
            .unwrap();
        let (mut stream, _) = events.accept().unwrap();
        stream.write_all(b"configreloaded>>\n").unwrap();
        reloads.recv_timeout(Duration::from_secs(2)).unwrap();

        // The compositor goes: the watcher says so, since what the setting is has become
        // unknown, and starts looking.
        drop(stream);
        drop(events);
        reloads.recv_timeout(Duration::from_secs(2)).unwrap();
        // The old instance's socket file may stay where it was, with nothing listening.
        assert_eq!(hyprland.animations_enabled(), None);

        // It comes back after a while, as a different instance.
        std::thread::sleep(Duration::from_millis(700));
        let new = dir.path().join("hypr/new");
        std::fs::create_dir_all(&new).unwrap();
        let (_asked, server) = serve(dir.path(), "new", DISABLED);
        let events = UnixListener::bind(new.join(".socket2.sock")).unwrap();
        let mut stream = accept_within(&events, 10);
        reloads
            .recv_timeout(Duration::from_secs(10))
            .expect("told that it is back");
        assert_eq!(hyprland.animations_enabled(), Some(false));
        server.join().unwrap();
        stream.write_all(b"configreloaded>>\n").unwrap();
        reloads
            .recv_timeout(Duration::from_secs(2))
            .expect("and followed");

        drop(watcher);
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut rest = Vec::new();
        assert_eq!(stream.read_to_end(&mut rest).unwrap(), 0);
    }

    /// A watcher dropped while it waits for the compositor to come back goes quietly.
    #[test]
    fn a_watcher_dropped_while_waiting_never_calls_again() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("hypr/old");
        std::fs::create_dir_all(&old).unwrap();
        let events = UnixListener::bind(old.join(".socket2.sock")).unwrap();
        let (heard, reloads) = mpsc::channel();
        let watcher = Hyprland::at(dir.path(), None)
            .watch_reloads(move || {
                let _ = heard.send(());
            })
            .unwrap();
        let (stream, _) = events.accept().unwrap();
        drop(stream);
        drop(events);
        reloads.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(watcher);

        let new = dir.path().join("hypr/new");
        std::fs::create_dir_all(&new).unwrap();
        let events = UnixListener::bind(new.join(".socket2.sock")).unwrap();
        events.set_nonblocking(true).unwrap();
        std::thread::sleep(Duration::from_millis(800));
        assert!(events.accept().is_err(), "it did not reconnect");
        assert!(reloads.try_recv().is_err(), "and did not call");
    }

    /// An application that starts while the compositor is restarting is told when it is there.
    #[test]
    fn a_compositor_that_is_not_up_yet_is_waited_for() {
        let dir = TempDir::new().unwrap();
        let (heard, reloads) = mpsc::channel();
        let watcher = Hyprland::at(dir.path(), Some("later".into()))
            .watch_reloads(move || {
                let _ = heard.send(());
            })
            .expect("the environment names an instance, so one is expected");
        assert!(reloads.recv_timeout(Duration::from_millis(300)).is_err());
        let instance = dir.path().join("hypr/later");
        std::fs::create_dir_all(&instance).unwrap();
        let events = UnixListener::bind(instance.join(".socket2.sock")).unwrap();
        let mut stream = accept_within(&events, 10);
        reloads
            .recv_timeout(Duration::from_secs(5))
            .expect("told that it is up");
        stream.write_all(b"configreloaded>>\n").unwrap();
        reloads
            .recv_timeout(Duration::from_secs(2))
            .expect("and followed");
        drop(watcher);
    }
}
