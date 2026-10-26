//! What `odkd` tells an application (ADR 0062): the state it sends, the path of its socket, and
//! the client that keeps the latest state.
//!
//! `odkd` writes the whole state as one line of JSON when a client connects, and again whenever
//! any of it changes. A value it could not read is absent, and an absent value is what the ADRs
//! say it is: motion is not reduced, the scale is 1, the font is the application's own, the
//! palette is the emergency one (ADR 0041).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread;
use std::time::Duration;

use serde_json::{Map, Value, json};

/// Names a socket other than the user's own. `none` (or nothing) says there is no host, so a
/// test, a screenshot or a build machine never hears the user's settings.
pub const SOCKET_ENV: &str = "ODKD_SOCKET";

/// The version of the line. A line of another version says nothing.
pub const VERSION: u64 = 1;

/// How long [`Host::from_environment`] waits for the first line, so an application does not
/// draw one frame of the emergency palette before the user's.
pub const FIRST_LINE_WAIT: Duration = Duration::from_millis(250);

/// A longer line than this is not `odkd`'s: the connection is dropped.
const MAX_LINE: u64 = 64 * 1024;
const RECONNECT_FROM: Duration = Duration::from_millis(200);
const RECONNECT_UP_TO: Duration = Duration::from_secs(5);

/// What the host says, as far as it could read it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostState {
    /// The user asks for reduced motion: the compositor's animations are off (ADR 0048).
    pub reduced_motion: Option<bool>,
    /// The user's text size in pixels, as ADR 0043 reads it: the user's `[font] base-size` over
    /// the theme's, clamped.
    pub base_size: Option<f32>,
    /// The monospace family fontconfig gives. Each application finds the font itself.
    pub font_family: Option<String>,
    /// The entries of the theme's `colors.toml`, the mode included, as the file has them.
    pub colors: Option<BTreeMap<String, String>>,
}

impl HostState {
    /// The line `odkd` writes, without its newline.
    pub fn to_line(&self) -> String {
        let mut line = Map::new();
        line.insert("v".into(), json!(VERSION));
        if let Some(reduced) = self.reduced_motion {
            line.insert("motion".into(), json!({ "reduced": reduced }));
        }
        if let Some(size) = self.base_size {
            line.insert("text".into(), json!({ "base_size": size }));
        }
        if let Some(family) = &self.font_family {
            line.insert("font".into(), json!({ "family": family }));
        }
        if let Some(colors) = &self.colors {
            line.insert("colors".into(), json!(colors));
        }
        Value::Object(line).to_string()
    }

    /// Reads a line. Keys it does not know are ignored; a line that is not JSON, or is of
    /// another version, is an error, and so is a known key of the wrong type.
    pub fn from_line(line: &str) -> Result<Self, HostError> {
        let value: Value = serde_json::from_str(line).map_err(|_| HostError::NotJson)?;
        let object = value.as_object().ok_or(HostError::NotJson)?;
        let version = object.get("v").and_then(Value::as_u64);
        if version != Some(VERSION) {
            return Err(HostError::Version(version));
        }
        let field = |group: &str, key: &str| -> Result<Option<&Value>, HostError> {
            match object.get(group) {
                None => Ok(None),
                Some(Value::Object(inner)) => Ok(inner.get(key)),
                Some(_) => Err(HostError::Wrong(group.to_owned())),
            }
        };
        let wrong = |what: &str| HostError::Wrong(what.to_owned());
        let reduced_motion = match field("motion", "reduced")? {
            None => None,
            Some(v) => Some(v.as_bool().ok_or_else(|| wrong("motion.reduced"))?),
        };
        let base_size = match field("text", "base_size")? {
            None => None,
            Some(v) => Some(v.as_f64().ok_or_else(|| wrong("text.base_size"))? as f32),
        };
        let font_family = match field("font", "family")? {
            None => None,
            Some(v) => Some(v.as_str().ok_or_else(|| wrong("font.family"))?.to_owned()),
        };
        let colors = match object.get("colors") {
            None => None,
            Some(Value::Object(entries)) => {
                let mut colors = BTreeMap::new();
                for (name, value) in entries {
                    let text = value.as_str().ok_or_else(|| wrong("colors"))?;
                    colors.insert(name.clone(), text.to_owned());
                }
                Some(colors)
            }
            Some(_) => return Err(wrong("colors")),
        };
        Ok(Self {
            reduced_motion,
            base_size,
            font_family,
            colors,
        })
    }

    /// The palette as the text of a `colors.toml`, for [`crate::Palette::parse`].
    pub fn colors_toml(&self) -> Option<String> {
        let mut table = toml::Table::new();
        for (name, value) in self.colors.as_ref()? {
            table.insert(name.clone(), toml::Value::String(value.clone()));
        }
        toml::to_string(&table).ok()
    }
}

/// Why a line says nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    #[error("the line is not a JSON object")]
    NotJson,
    #[error("the line is of version {0:?}, and this client reads version 1")]
    Version(Option<u64>),
    #[error("{0} is of the wrong type")]
    Wrong(String),
}

/// The socket an application asks: `$ODKD_SOCKET` when it is set, else
/// `$XDG_RUNTIME_DIR/odk/odkd.sock`. `None` when there is none to ask.
pub fn socket_path() -> Option<PathBuf> {
    socket_path_from(
        std::env::var_os(SOCKET_ENV),
        std::env::var_os("XDG_RUNTIME_DIR"),
    )
}

fn socket_path_from(
    socket: Option<std::ffi::OsString>,
    runtime_dir: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    match socket {
        Some(v) if v.is_empty() || v == "none" => return None,
        Some(v) => return Some(PathBuf::from(v)),
        None => {}
    }
    let dir = PathBuf::from(runtime_dir?);
    dir.is_absolute().then(|| dir.join("odk/odkd.sock"))
}

type Callback = Box<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Shared {
    state: Mutex<Option<HostState>>,
    arrived: Condvar,
    on_change: Mutex<Option<Callback>>,
}

/// The latest state `odkd` sent, kept by a thread that waits on the socket and reconnects when
/// `odkd` goes away. Nothing runs on a clock: the thread sleeps in `read`, and waits only after
/// a connection failed.
pub struct Host {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("state", &self.state())
            .finish()
    }
}

impl Host {
    /// The host of this environment, after at most [`FIRST_LINE_WAIT`] for its first line. `None`
    /// when there is no socket to ask (see [`socket_path`]) or no such file.
    pub fn from_environment() -> Option<Self> {
        Self::connect(&socket_path()?, FIRST_LINE_WAIT)
    }

    /// Connects to the socket at `path`, and waits at most `wait` for the first line. `None`
    /// when there is no such file: no `odkd` is installed. A socket that does not answer in time
    /// is a host with no state yet; the thread keeps trying.
    pub fn connect(path: &Path, wait: Duration) -> Option<Self> {
        path.metadata().ok()?;
        let shared = Arc::new(Shared::default());
        let weak = Arc::downgrade(&shared);
        let path = path.to_owned();
        thread::Builder::new()
            .name("odkd".into())
            .spawn(move || run(&path, &weak))
            .ok()?;
        let guard = shared.state.lock().unwrap();
        drop(
            shared
                .arrived
                .wait_timeout_while(guard, wait, |state| state.is_none())
                .unwrap(),
        );
        Some(Self { shared })
    }

    /// The latest state, or `None` when `odkd` has not said anything yet.
    pub fn state(&self) -> Option<HostState> {
        self.shared.state.lock().unwrap().clone()
    }

    /// Runs `f` on the host's thread each time a line changes the state. One callback: a later
    /// call replaces it.
    pub fn on_change(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.shared.on_change.lock().unwrap() = Some(Box::new(f));
    }
}

fn run(path: &Path, shared: &Weak<Shared>) {
    let mut wait = RECONNECT_FROM;
    loop {
        match UnixStream::connect(path) {
            Ok(stream) => {
                if listen(stream, shared) {
                    wait = RECONNECT_FROM;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => {}
        }
        if shared.strong_count() == 0 {
            return;
        }
        thread::sleep(wait);
        wait = (wait * 2).min(RECONNECT_UP_TO);
    }
}

/// Reads lines until the connection ends. Whether it gave one that held.
fn listen(stream: UnixStream, shared: &Weak<Shared>) -> bool {
    let mut reader = BufReader::new(stream);
    let mut gave = false;
    loop {
        let mut line = String::new();
        match reader.by_ref().take(MAX_LINE).read_line(&mut line) {
            Ok(0) | Err(_) => return gave,
            Ok(_) if !line.ends_with('\n') => return gave,
            Ok(_) => {}
        }
        let Some(shared) = shared.upgrade() else {
            return gave;
        };
        match HostState::from_line(line.trim_end()) {
            Ok(state) => {
                gave = true;
                let changed = {
                    let mut current = shared.state.lock().unwrap();
                    let changed = current.as_ref() != Some(&state);
                    *current = Some(state);
                    changed
                };
                shared.arrived.notify_all();
                if changed && let Some(f) = shared.on_change.lock().unwrap().as_ref() {
                    f();
                }
            }
            Err(error) => log::debug!("odkd sent a line that says nothing: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn full() -> HostState {
        HostState {
            reduced_motion: Some(true),
            base_size: Some(14.0),
            font_family: Some("JetBrainsMono Nerd Font".into()),
            colors: Some(BTreeMap::from([
                ("mode".into(), "dark".into()),
                ("background".into(), "#0D0C0B".into()),
            ])),
        }
    }

    #[test]
    fn a_state_survives_the_line() {
        let line = full().to_line();
        assert_eq!(HostState::from_line(&line), Ok(full()));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn what_odkd_could_not_read_is_absent_and_stays_absent() {
        let line = HostState::default().to_line();
        assert_eq!(line, r#"{"v":1}"#);
        assert_eq!(HostState::from_line(&line), Ok(HostState::default()));
    }

    #[test]
    fn keys_it_does_not_know_are_ignored_and_the_wrong_kind_of_line_is_refused() {
        let state = HostState::from_line(
            r#"{"v":1,"motion":{"reduced":false,"later":1},"future":[1],"text":{"base_size":12}}"#,
        )
        .unwrap();
        assert_eq!(state.reduced_motion, Some(false));
        assert_eq!(state.base_size, Some(12.0));
        for bad in [
            "",
            "[]",
            "not json",
            r#"{"motion":{"reduced":true}}"#,
            r#"{"v":2}"#,
            r#"{"v":1,"motion":true}"#,
            r#"{"v":1,"motion":{"reduced":"yes"}}"#,
            r#"{"v":1,"text":{"base_size":"big"}}"#,
            r#"{"v":1,"colors":{"background":1}}"#,
        ] {
            assert!(HostState::from_line(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_palette_comes_back_as_a_colors_toml() {
        let text = full().colors_toml().unwrap();
        let table: toml::Table = text.parse().unwrap();
        assert_eq!(table["mode"].as_str(), Some("dark"));
        assert_eq!(table["background"].as_str(), Some("#0D0C0B"));
        assert_eq!(HostState::default().colors_toml(), None);
    }

    #[test]
    fn the_socket_is_the_users_unless_the_environment_says_otherwise() {
        let os = |s: &str| Some(std::ffi::OsString::from(s));
        assert_eq!(
            socket_path_from(None, os("/run/user/1000")),
            Some("/run/user/1000/odk/odkd.sock".into())
        );
        assert_eq!(
            socket_path_from(os("/tmp/x.sock"), os("/run/user/1000")),
            Some("/tmp/x.sock".into())
        );
        assert_eq!(socket_path_from(os("none"), os("/run/user/1000")), None);
        assert_eq!(socket_path_from(os(""), os("/run/user/1000")), None);
        assert_eq!(socket_path_from(None, None), None);
        assert_eq!(socket_path_from(None, os("relative")), None);
    }

    fn until(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(5), "{what}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn serve_one(listener: &UnixListener, lines: &[String]) -> UnixStream {
        let (mut stream, _) = listener.accept().unwrap();
        for line in lines {
            writeln!(stream, "{line}").unwrap();
        }
        stream
    }

    #[test]
    fn the_first_line_is_waited_for_and_each_later_one_replaces_it_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odkd.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let mut stream = serve_one(&listener, &[full().to_line()]);
            thread::sleep(Duration::from_millis(100));
            let next = HostState {
                reduced_motion: Some(false),
                ..full()
            };
            writeln!(stream, "{}", next.to_line()).unwrap();
            stream
        });
        let host = Host::connect(&path, Duration::from_secs(5)).unwrap();
        assert_eq!(
            host.state(),
            Some(full()),
            "the first line is there at once"
        );
        let changes = Arc::new(AtomicUsize::new(0));
        let seen = changes.clone();
        host.on_change(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        until("the second line", || {
            host.state().and_then(|s| s.reduced_motion) == Some(false)
        });
        assert_eq!(changes.load(Ordering::SeqCst), 1);
        drop(server.join().unwrap());
    }

    #[test]
    fn no_socket_is_no_host_and_a_silent_one_is_a_host_with_nothing_to_say() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odkd.sock");
        assert!(Host::connect(&path, Duration::from_millis(10)).is_none());
        let _listener = UnixListener::bind(&path).unwrap();
        let start = Instant::now();
        let host = Host::connect(&path, Duration::from_millis(50)).unwrap();
        assert!(start.elapsed() >= Duration::from_millis(50));
        assert_eq!(host.state(), None);
    }

    #[test]
    fn a_host_that_goes_away_is_found_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odkd.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            drop(serve_one(&listener, &[full().to_line()]));
            // The connection ended; the socket is still there, as systemd keeps it.
            let next = HostState {
                base_size: Some(16.0),
                ..full()
            };
            serve_one(&listener, &[next.to_line()])
        });
        let host = Host::connect(&path, Duration::from_secs(5)).unwrap();
        assert_eq!(host.state(), Some(full()));
        until("the line after the reconnect", || {
            host.state().and_then(|s| s.base_size) == Some(16.0)
        });
        drop(server.join().unwrap());
    }

    #[test]
    fn a_line_that_is_too_long_is_not_odkds_and_ends_the_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odkd.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let mut stream = serve_one(&listener, &[]);
            // Ignore a failed write: the client may have hung up already.
            let _ = stream.write_all(&vec![b'a'; MAX_LINE as usize + 10]);
        });
        let host = Host::connect(&path, Duration::from_millis(200)).unwrap();
        server.join().unwrap();
        assert_eq!(host.state(), None);
    }

    #[test]
    fn a_line_that_says_nothing_keeps_what_was_said() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odkd.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            serve_one(
                &listener,
                &[full().to_line(), "garbage".into(), r#"{"v":9}"#.into()],
            )
        });
        let host = Host::connect(&path, Duration::from_secs(5)).unwrap();
        let stream = server.join().unwrap();
        thread::sleep(Duration::from_millis(50));
        assert_eq!(host.state(), Some(full()));
        drop(stream);
    }
}
