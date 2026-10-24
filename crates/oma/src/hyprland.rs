//! Whether Hyprland animates windows: its `animations:enabled`, which is the user's request for
//! reduced motion (ADR 0048). `oma` asks so that it can tell a sandboxed application what the
//! application cannot ask itself: the sandbox mounts nothing of Hyprland's, and must not, since
//! its socket accepts commands (`docs/spec/sandbox-baseline.md`).
//!
//! `oma` never depends on ODK (ADR 0013), so this is a second, smaller reader of the same
//! reply as `odk-theme`'s `compositor.rs`, as `oma-consent` keeps its own `colors.toml` reader.
//! It asks one question and sends no command.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How long to wait for the compositor, so a stalled one costs a launch at most this.
const PATIENCE: Duration = Duration::from_millis(250);
const MOST_OF_A_REPLY: u64 = 4096;
const QUESTION: &[u8] = b"j/getoption animations:enabled";

/// Whether the compositor of this process's environment animates windows; `None` when there is
/// none, it cannot be reached or its answer is not understood.
pub fn animations_enabled() -> Option<bool> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    animations_enabled_in(&runtime, std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE"))
}

/// The same for the runtime directory `runtime`, of instance `signature` when it is known. A
/// systemd user service does not always have the signature, so every instance with a socket is
/// tried, the newest first, after the named one.
pub fn animations_enabled_in(runtime: &Path, signature: Option<OsString>) -> Option<bool> {
    let hypr = runtime.join("hypr");
    let named = signature.filter(|s| !s.is_empty()).map(|s| hypr.join(s));
    let mut others: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(&hypr)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|dir| Some(dir) != named.as_ref())
        .filter_map(|dir| {
            let modified = std::fs::metadata(dir.join(".socket.sock"))
                .ok()?
                .modified()
                .ok()?;
            Some((modified, dir))
        })
        .collect();
    others.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    named
        .into_iter()
        .chain(others.into_iter().map(|(_, dir)| dir))
        .find_map(|dir| ask(&dir.join(".socket.sock")))
}

fn ask(socket: &Path) -> Option<bool> {
    let mut stream = UnixStream::connect(socket).ok()?;
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

/// The boolean of a `getoption` reply in JSON; `None` for anything else.
fn parse_reply(reply: &str) -> Option<bool> {
    let value = reply
        .split_once("\"bool\"")?
        .1
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
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

    use tempfile::TempDir;

    use super::*;

    /// A runtime directory with an instance that answers `reply` to its one question.
    fn serve(runtime: &Path, signature: &str, reply: &'static str) -> std::thread::JoinHandle<()> {
        let dir = runtime.join("hypr").join(signature);
        std::fs::create_dir_all(&dir).unwrap();
        let listener = UnixListener::bind(dir.join(".socket.sock")).unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut question = [0u8; 128];
            let n = stream.read(&mut question).unwrap();
            assert_eq!(&question[..n], QUESTION);
            stream.write_all(reply.as_bytes()).unwrap();
        })
    }

    const ON: &str = r#"{"option": "animations:enabled", "bool": true, "set": true }"#;
    const OFF: &str = r#"{"option": "animations:enabled", "bool": false, "set": true }"#;

    #[test]
    fn it_reads_the_answer_of_the_named_instance_or_the_one_that_is_there() {
        let dir = TempDir::new().unwrap();
        let server = serve(dir.path(), "abc", OFF);
        assert_eq!(
            animations_enabled_in(dir.path(), Some("abc".into())),
            Some(false)
        );
        server.join().unwrap();

        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("hypr/stale")).unwrap();
        let server = serve(dir.path(), "live", ON);
        assert_eq!(animations_enabled_in(dir.path(), None), Some(true));
        server.join().unwrap();
    }

    #[test]
    fn nothing_there_or_nothing_understood_says_nothing() {
        let dir = TempDir::new().unwrap();
        assert_eq!(animations_enabled_in(dir.path(), None), None);
        assert_eq!(animations_enabled_in(dir.path(), Some("abc".into())), None);
        let server = serve(dir.path(), "abc", "unknown request");
        assert_eq!(animations_enabled_in(dir.path(), Some("abc".into())), None);
        server.join().unwrap();
        for reply in ["", "{\"bool\": 1}", "{\"bool\": \"true\"}", "{\"error\":1}"] {
            assert_eq!(parse_reply(reply), None, "{reply:?}");
        }
    }

    /// After a restart the environment still names the old instance, whose directory may even
    /// still be there with a socket nothing listens on.
    #[test]
    fn a_restarted_compositor_is_found_past_the_instance_the_environment_names() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("hypr/old");
        std::fs::create_dir_all(&old).unwrap();
        drop(UnixListener::bind(old.join(".socket.sock")).unwrap());
        let server = serve(dir.path(), "new", OFF);
        assert_eq!(
            animations_enabled_in(dir.path(), Some("old".into())),
            Some(false)
        );
        server.join().unwrap();
    }
}
