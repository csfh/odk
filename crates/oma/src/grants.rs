//! The consent record: what the user has allowed an application, per Identifier (ADR 0023).

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::fmt::{Access, Daemon, FsGrant, Identifier, Permissions, UserDir};
use crate::layout::Layout;

const VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the consent record {} is damaged: {reason}", path.display())]
    Damaged { path: PathBuf, reason: String },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    grant_version: u32,
    identifier: String,
    network: bool,
    gpu: bool,
    audio: bool,
    updates: bool,
    filesystem: Vec<Entry>,
    dbus_talk: Vec<String>,
    /// Absent in a record written before ADR 0050, which grants no daemon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    daemon: Option<DaemonEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    dir: String,
    access: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonEntry {
    executable: String,
    args: Vec<String>,
    socket: String,
}

/// What the user has consented to for one application.
pub fn load(layout: &Layout, id: &Identifier) -> Result<Option<Permissions>, Error> {
    let path = layout.grants_file(id);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(Error::Io { path, source }),
    };
    let damaged = |reason: String| Error::Damaged {
        path: path.clone(),
        reason,
    };
    let file: File = toml::from_str(&text).map_err(|e| damaged(e.to_string()))?;
    if file.grant_version != VERSION {
        return Err(damaged(format!(
            "unknown grant_version {}",
            file.grant_version
        )));
    }
    if file.identifier != id.as_str() {
        return Err(damaged(format!("it names {:?}", file.identifier)));
    }
    let mut filesystem = Vec::new();
    for entry in file.filesystem {
        let dir = UserDir::parse(&entry.dir)
            .ok_or_else(|| damaged(format!("unknown directory {:?}", entry.dir)))?;
        let access = match entry.access.as_str() {
            "read" => Access::Read,
            "write" => Access::Write,
            other => return Err(damaged(format!("unknown access {other:?}"))),
        };
        filesystem.push(FsGrant { dir, access });
    }
    Ok(Some(Permissions {
        network: file.network,
        gpu: file.gpu,
        audio: file.audio,
        updates: file.updates,
        filesystem,
        dbus_talk: file.dbus_talk,
        daemon: file.daemon.map(|d| Daemon {
            executable: d.executable,
            args: d.args,
            socket: d.socket,
        }),
    }))
}

/// Records consent for exactly `permissions`, replacing any earlier record. The file is
/// private to the user and replaced atomically.
pub fn save(layout: &Layout, id: &Identifier, permissions: &Permissions) -> Result<(), Error> {
    let path = layout.grants_file(id);
    let io = |path: &PathBuf, source| Error::Io {
        path: path.clone(),
        source,
    };
    let dir = path.parent().expect("grants file has a parent").to_owned();
    fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
    let file = File {
        grant_version: VERSION,
        identifier: id.as_str().to_owned(),
        network: permissions.network,
        gpu: permissions.gpu,
        audio: permissions.audio,
        updates: permissions.updates,
        filesystem: permissions
            .filesystem
            .iter()
            .map(|g| Entry {
                dir: g.dir.as_str().to_owned(),
                access: match g.access {
                    Access::Read => "read",
                    Access::Write => "write",
                }
                .to_owned(),
            })
            .collect(),
        dbus_talk: permissions.dbus_talk.clone(),
        daemon: permissions.daemon.as_ref().map(|d| DaemonEntry {
            executable: d.executable.clone(),
            args: d.args.clone(),
            socket: d.socket.clone(),
        }),
    };
    let text = toml::to_string(&file).expect("a grant serialises");
    let staging = dir.join(format!(".{id}.toml.partial-{}", std::process::id()));
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staging)
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| fs::rename(&staging, &path));
    if let Err(source) = result {
        let _ = fs::remove_file(&staging);
        return Err(io(&path, source));
    }
    Ok(())
}

/// The part of `requested` that `granted` does not cover. Empty means nothing new is asked.
pub fn missing(granted: &Permissions, requested: &Permissions) -> Permissions {
    Permissions {
        network: requested.network && !granted.network,
        gpu: requested.gpu && !granted.gpu,
        audio: requested.audio && !granted.audio,
        updates: requested.updates && !granted.updates,
        filesystem: requested
            .filesystem
            .iter()
            .filter(|want| {
                !granted.filesystem.iter().any(|have| {
                    have.dir == want.dir
                        && (have.access == Access::Write || want.access == Access::Read)
                })
            })
            .copied()
            .collect(),
        dbus_talk: requested
            .dbus_talk
            .iter()
            .filter(|n| !granted.dbus_talk.contains(n))
            .cloned()
            .collect(),
        // The three parts are one grant: a different executable, arguments or socket is a
        // different program running outside the sandbox, so it is asked for whole.
        daemon: requested
            .daemon
            .as_ref()
            .filter(|want| granted.daemon.as_ref() != Some(*want))
            .cloned(),
    }
}

/// Whether `permissions` asks for nothing at all.
pub fn is_empty(permissions: &Permissions) -> bool {
    *permissions == Permissions::default()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn id() -> Identifier {
        "com.example.t".parse().unwrap()
    }

    fn everything() -> Permissions {
        Permissions {
            network: true,
            gpu: true,
            audio: true,
            updates: true,
            filesystem: vec![
                FsGrant {
                    dir: UserDir::Documents,
                    access: Access::Write,
                },
                FsGrant {
                    dir: UserDir::Music,
                    access: Access::Read,
                },
            ],
            dbus_talk: vec!["org.a".into()],
            daemon: Some(daemon("bin/d", &["daemon"], "app/d.sock")),
        }
    }

    fn daemon(executable: &str, args: &[&str], socket: &str) -> Daemon {
        Daemon {
            executable: executable.into(),
            args: args.iter().map(|a| (*a).into()).collect(),
            socket: socket.into(),
        }
    }

    #[test]
    fn a_record_from_before_daemons_grants_none() {
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path(), dir.path().join("share"));
        let path = layout.grants_file(&id());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "grant_version = 1\nidentifier = \"com.example.t\"\nnetwork = true\ngpu = false\naudio = false\nupdates = false\nfilesystem = []\ndbus_talk = []\n",
        )
        .unwrap();
        let granted = load(&layout, &id()).unwrap().unwrap();
        assert!(granted.network && granted.daemon.is_none());
        // And a bundle that now declares one is asked, whole.
        let asked = missing(&granted, &everything());
        assert_eq!(asked.daemon, everything().daemon);
    }

    #[test]
    fn a_daemon_is_granted_whole_so_any_change_asks_again() {
        let with = |d: Option<Daemon>| Permissions {
            daemon: d,
            ..Default::default()
        };
        let granted = with(Some(daemon("bin/d", &["daemon"], "app/d.sock")));
        for same in [Some(daemon("bin/d", &["daemon"], "app/d.sock")), None] {
            assert!(is_empty(&missing(&granted, &with(same))));
        }
        for changed in [
            daemon("bin/other", &["daemon"], "app/d.sock"),
            daemon("bin/d", &["daemon", "--more"], "app/d.sock"),
            daemon("bin/d", &["daemon"], "app/other.sock"),
        ] {
            assert_eq!(
                missing(&granted, &with(Some(changed.clone()))).daemon,
                Some(changed)
            );
        }
        assert_eq!(
            missing(&Permissions::default(), &granted).daemon,
            granted.daemon
        );
    }

    #[test]
    fn a_saved_record_loads_back_and_is_private() {
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path(), dir.path().join("share"));
        assert_eq!(load(&layout, &id()).unwrap(), None);
        save(&layout, &id(), &everything()).unwrap();
        assert_eq!(load(&layout, &id()).unwrap(), Some(everything()));
        let mode = std::os::unix::fs::PermissionsExt::mode(
            &fs::metadata(layout.grants_file(&id()))
                .unwrap()
                .permissions(),
        );
        assert_eq!(mode & 0o777, 0o600);
        save(&layout, &id(), &Permissions::default()).unwrap();
        assert_eq!(
            load(&layout, &id()).unwrap(),
            Some(Permissions::default()),
            "saving replaces"
        );
    }

    #[test]
    fn a_damaged_record_is_an_error_not_an_empty_grant() {
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path(), dir.path().join("share"));
        let path = layout.grants_file(&id());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for text in [
            "not toml [",
            "grant_version = 2\nidentifier = \"com.example.t\"\nnetwork = false\ngpu = false\naudio = false\nupdates = false\nfilesystem = []\ndbus_talk = []\n",
            "grant_version = 1\nidentifier = \"com.example.other\"\nnetwork = false\ngpu = false\naudio = false\nupdates = false\nfilesystem = []\ndbus_talk = []\n",
            "grant_version = 1\nidentifier = \"com.example.t\"\nnetwork = false\ngpu = false\naudio = false\nupdates = false\nfilesystem = [{dir = \"home\", access = \"read\"}]\ndbus_talk = []\n",
            "grant_version = 1\nidentifier = \"com.example.t\"\nnetwork = false\n",
        ] {
            fs::write(&path, text).unwrap();
            assert!(
                matches!(load(&layout, &id()), Err(Error::Damaged { .. })),
                "{text}"
            );
        }
    }

    #[test]
    fn missing_is_what_was_asked_and_not_granted() {
        assert!(is_empty(&missing(&everything(), &everything())));
        assert!(is_empty(&missing(
            &Permissions::default(),
            &Permissions::default()
        )));

        let want = missing(&Permissions::default(), &everything());
        assert_eq!(want, everything());

        let mut granted = everything();
        granted.gpu = false;
        granted.dbus_talk.clear();
        let want = missing(&granted, &everything());
        assert!(want.gpu && !want.network && want.dbus_talk == ["org.a"]);
        assert!(want.filesystem.is_empty());
    }

    #[test]
    fn filesystem_write_covers_read_but_not_the_other_way() {
        let read = |d| FsGrant {
            dir: d,
            access: Access::Read,
        };
        let write = |d| FsGrant {
            dir: d,
            access: Access::Write,
        };
        let perms = |fs| Permissions {
            filesystem: fs,
            ..Default::default()
        };

        assert!(is_empty(&missing(
            &perms(vec![write(UserDir::Documents)]),
            &perms(vec![read(UserDir::Documents)])
        )));
        assert_eq!(
            missing(
                &perms(vec![read(UserDir::Documents)]),
                &perms(vec![write(UserDir::Documents)])
            )
            .filesystem,
            [write(UserDir::Documents)]
        );
        assert_eq!(
            missing(
                &perms(vec![write(UserDir::Music)]),
                &perms(vec![read(UserDir::Documents)])
            )
            .filesystem,
            [read(UserDir::Documents)]
        );
    }

    #[test]
    fn asking_for_less_than_granted_prompts_for_nothing() {
        let narrower = Permissions {
            gpu: true,
            ..Default::default()
        };
        assert!(is_empty(&missing(&everything(), &narrower)));
    }
}
