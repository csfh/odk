//! The sockets: the ones systemd hands over, or ones `odkd` makes when it is started by hand.
//! `odkd.sock` is the host's settings, which a client only hears (ADR 0062); `files.sock` is the
//! file chooser, which answers one request per connection (ADR 0068).

use std::fs;
use std::os::fd::FromRawFd;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use crate::hub::Hub;

/// The first descriptor systemd passes (`sd_listen_fds(3)`).
const LISTEN_FDS_START: i32 = 3;

/// The two sockets `odkd` serves.
pub struct Listeners {
    pub host: UnixListener,
    /// `None` when there is nowhere to make it: `ODKD_SOCKET=none` and systemd passed none.
    pub files: Option<UnixListener>,
}

/// The sockets to accept on: those systemd passed, else ones made at `host` and beside it.
pub fn listeners(host: Option<PathBuf>) -> Result<Listeners, String> {
    let passed = activated(
        std::env::var("LISTEN_PID").ok().as_deref(),
        std::env::var("LISTEN_FDS").ok().as_deref(),
        std::env::var("LISTEN_FDNAMES").ok().as_deref(),
    );
    // SAFETY: `activated` checked systemd's contract: these descriptors were passed to this
    // process, are listening sockets and are ours to own, and nothing else here uses them.
    let take = |fd: i32| unsafe { UnixListener::from_raw_fd(fd) };
    let host_listener = match passed.host {
        Some(fd) => take(fd),
        None => bind(host.as_deref().ok_or(
            "no socket: ODKD_SOCKET says there is no host, and XDG_RUNTIME_DIR is not set",
        )?)?,
    };
    let files = match passed.files {
        Some(fd) => Some(take(fd)),
        None => match host.as_deref().map(odk_theme::chooser::beside) {
            Some(path) => Some(bind(&path)?),
            None => None,
        },
    };
    Ok(Listeners {
        host: host_listener,
        files,
    })
}

/// The descriptors systemd passed to this process, by what each is for.
#[derive(Debug, Default, PartialEq)]
struct Passed {
    host: Option<i32>,
    files: Option<i32>,
}

/// Systemd's sockets, when this process was started with some: `LISTEN_PID` is ours, and each
/// descriptor's name in `LISTEN_FDNAMES` says which socket it is. The units name theirs `host`
/// and `files`; an older `odkd.socket` passes one socket under its unit's name, or no names.
fn activated(pid: Option<&str>, fds: Option<&str>, names: Option<&str>) -> Passed {
    let mut passed = Passed::default();
    let ours = pid
        .and_then(|p| p.parse::<u32>().ok())
        .is_some_and(|p| p == std::process::id());
    let Some(count) = fds.and_then(|n| n.parse::<i32>().ok()).filter(|_| ours) else {
        return passed;
    };
    let names: Vec<&str> = names.map(|n| n.split(':').collect()).unwrap_or_default();
    for i in 0..count {
        let fd = LISTEN_FDS_START + i;
        match names.get(i as usize).copied() {
            Some("host" | "odkd.socket") => passed.host = Some(fd),
            Some("files" | "odkd-files.socket") => passed.files = Some(fd),
            None if count == 1 => passed.host = Some(fd),
            _ => {}
        }
    }
    passed
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
    fn systemds_sockets_are_taken_only_when_they_were_passed_to_this_process() {
        let me = std::process::id().to_string();
        let none = Passed::default();
        assert_eq!(activated(None, None, None), none);
        assert_eq!(
            activated(Some("1"), Some("1"), None),
            none,
            "another process's"
        );
        assert_eq!(activated(Some("x"), Some("1"), None), none);
        assert_eq!(
            activated(Some(&me), Some("2"), None),
            none,
            "two sockets with no names say nothing"
        );
    }

    #[test]
    fn each_socket_is_found_by_its_name() {
        let me = std::process::id().to_string();
        let both = Passed {
            host: Some(3),
            files: Some(4),
        };
        assert_eq!(activated(Some(&me), Some("2"), Some("host:files")), both);
        let swapped = Passed {
            host: Some(4),
            files: Some(3),
        };
        assert_eq!(activated(Some(&me), Some("2"), Some("files:host")), swapped);
        let units = activated(Some(&me), Some("2"), Some("odkd.socket:odkd-files.socket"));
        assert_eq!(units, both, "named after their units");
        let old = Passed {
            host: Some(3),
            files: None,
        };
        assert_eq!(activated(Some(&me), Some("1"), None), old, "an older unit");
        assert_eq!(activated(Some(&me), Some("1"), Some("odkd.socket")), old);
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
