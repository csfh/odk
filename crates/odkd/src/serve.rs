//! The socket: the one systemd hands over, or one `odkd` makes when it is started by hand.

use std::fs;
use std::os::fd::FromRawFd;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use crate::hub::Hub;

/// The first descriptor systemd passes (`sd_listen_fds(3)`).
const LISTEN_FDS_START: i32 = 3;

/// The socket to accept on: the one `odkd.socket` passed, else one made at `path`.
pub fn listener(path: Option<std::path::PathBuf>) -> Result<UnixListener, String> {
    if let Some(listener) = activated(
        std::env::var("LISTEN_PID").ok().as_deref(),
        std::env::var("LISTEN_FDS").ok().as_deref(),
    ) {
        return Ok(listener);
    }
    let path =
        path.ok_or("no socket: ODKD_SOCKET says there is no host, and XDG_RUNTIME_DIR is not set")?;
    bind(&path)
}

/// Systemd's socket, when this process was started with one: `LISTEN_PID` is ours and
/// `LISTEN_FDS` is one.
fn activated(pid: Option<&str>, fds: Option<&str>) -> Option<UnixListener> {
    if pid?.parse::<u32>().ok()? != std::process::id() || fds?.parse::<i32>().ok()? != 1 {
        return None;
    }
    // SAFETY: the check above is systemd's own contract: descriptor 3 was passed to this
    // process, is a listening socket and is ours to own, and nothing else here uses it.
    Some(unsafe { UnixListener::from_raw_fd(LISTEN_FDS_START) })
}

/// Makes the socket at `path` for the user alone, taking over one that nobody answers on.
fn bind(path: &Path) -> Result<UnixListener, String> {
    let dir = path.parent().ok_or("the socket has no directory")?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(format!(
                "{} is already answered by another odkd",
                path.display()
            ));
        }
        fs::remove_file(path).map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).map_err(|e| format!("cannot bind {}: {e}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("cannot restrict {}: {e}", path.display()))?;
    Ok(listener)
}

/// Accepts connections on a thread of their own, handing each to the hub.
pub fn accept_in_background(listener: UnixListener, hub: Arc<Hub>) {
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if !hub.add(stream) {
                        eprintln!("odkd: too many connections, one was refused");
                    }
                }
                Err(e) => eprintln!("odkd: accept failed: {e}"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemds_socket_is_taken_only_when_it_was_passed_to_this_process() {
        let me = std::process::id().to_string();
        assert!(activated(None, None).is_none());
        assert!(
            activated(Some("1"), Some("1")).is_none(),
            "another process's"
        );
        assert!(activated(Some(&me), Some("2")).is_none(), "not one socket");
        assert!(activated(Some("x"), Some("1")).is_none());
    }

    #[test]
    fn a_socket_made_by_hand_is_the_users_alone_and_a_stale_one_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odk/odkd.sock");
        let first = bind(&path).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        assert!(bind(&path).is_err(), "another odkd answers on it");
        drop(first);
        assert!(bind(&path).is_ok(), "nobody does now");
    }
}
