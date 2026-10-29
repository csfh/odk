//! The consent record: what the user has allowed an application.
//!
//! A directory Bundle's record is per Identifier (ADR 0023). A signed file's is per
//! (root key id, app id) pair (ADR 0060, section 2). It holds the pin, the host the root was
//! pinned from, consent, the high-water mark and a denial, and nothing else, since the rest of
//! its schema is open (`docs/oma-trust-plan.md`, question 14). The two records live at
//! different paths, so a record from before ADR 0060 is never read as a pin.

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fmt::{Access, Daemon, FsGrant, Identifier, Permissions, UserDir};
use crate::layout::Layout;
use crate::trust::{KeyId, Mark, PublicKey, Publisher};

const VERSION: u32 = 1;
/// A pair's record. Its own number, so neither kind of record is read as the other.
const PAIR_VERSION: u32 = 2;

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

/// The permissions part of a record, as a pair's record nests it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Granted {
    network: bool,
    gpu: bool,
    audio: bool,
    updates: bool,
    filesystem: Vec<Entry>,
    dbus_talk: Vec<String>,
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

impl Granted {
    fn of(permissions: &Permissions) -> Self {
        Self {
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
        }
    }

    fn permissions(self) -> Result<Permissions, String> {
        let mut filesystem = Vec::new();
        for entry in self.filesystem {
            let dir = UserDir::parse(&entry.dir)
                .ok_or_else(|| format!("unknown directory {:?}", entry.dir))?;
            let access = match entry.access.as_str() {
                "read" => Access::Read,
                "write" => Access::Write,
                other => return Err(format!("unknown access {other:?}")),
            };
            filesystem.push(FsGrant { dir, access });
        }
        Ok(Permissions {
            network: self.network,
            gpu: self.gpu,
            audio: self.audio,
            updates: self.updates,
            filesystem,
            dbus_talk: self.dbus_talk,
            daemon: self.daemon.map(|d| Daemon {
                executable: d.executable,
                args: d.args,
                socket: d.socket,
            }),
        })
    }
}

/// The text at `path`, or `None` if there is no file.
fn read(path: &Path) -> Result<Option<String>, Error> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Writes `text` to `path`, private to the user and replaced atomically.
fn write_private(path: &Path, text: &str) -> Result<(), Error> {
    let io = |path: &Path, source| Error::Io {
        path: path.to_owned(),
        source,
    };
    let dir = path.parent().expect("a record has a parent");
    fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    let name = path
        .file_name()
        .expect("a record has a name")
        .to_string_lossy();
    let staging = dir.join(format!(".{name}.partial-{}", std::process::id()));
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staging)
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| fs::rename(&staging, path));
    if let Err(source) = result {
        let _ = fs::remove_file(&staging);
        return Err(io(path, source));
    }
    Ok(())
}

/// What the user has consented to for one application.
pub fn load(layout: &Layout, id: &Identifier) -> Result<Option<Permissions>, Error> {
    let path = layout.grants_file(id);
    let Some(text) = read(&path)? else {
        return Ok(None);
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
    let granted = Granted {
        network: file.network,
        gpu: file.gpu,
        audio: file.audio,
        updates: file.updates,
        filesystem: file.filesystem,
        dbus_talk: file.dbus_talk,
        daemon: file.daemon,
    };
    granted.permissions().map(Some).map_err(damaged)
}

/// Records consent for exactly `permissions`, replacing any earlier record. The file is
/// private to the user and replaced atomically.
pub fn save(layout: &Layout, id: &Identifier, permissions: &Permissions) -> Result<(), Error> {
    let granted = Granted::of(permissions);
    let file = File {
        grant_version: VERSION,
        identifier: id.as_str().to_owned(),
        network: granted.network,
        gpu: granted.gpu,
        audio: granted.audio,
        updates: granted.updates,
        filesystem: granted.filesystem,
        dbus_talk: granted.dbus_talk,
        daemon: granted.daemon,
    };
    write_private(
        &layout.grants_file(id),
        &toml::to_string(&file).expect("a grant serialises"),
    )
}

/// What `odk-oma` keeps for one (root key id, app id) pair (ADR 0060, sections 2 and 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The pin: the publisher root this application belongs to.
    pub root: PublicKey,
    /// The host the root was first discovered at. Display text and the first-contact host, not
    /// the root of trust.
    pub publisher: Publisher,
    /// What the user has consented to. `None` until something is granted.
    pub permissions: Option<Permissions>,
    pub mark: Option<Mark>,
    /// The version and payload of the file installed now: the mark's, unless `odk oma rollback`
    /// put the previous file back (ADR 0011), which does not lower the mark. `None` is the mark's.
    pub installed: Option<Mark>,
    /// The user cancelled the first-install dialog for this pair. A launch, `--register` and
    /// another pass over the same file do not ask again; `odk oma install` may.
    pub denied: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PairFile {
    grant_version: u32,
    app_id: String,
    /// The root's 32 bytes, as 64 lowercase hex digits.
    root_public_key: String,
    publisher: String,
    denied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    high_water: Option<HighWater>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installed: Option<HighWater>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permissions: Option<Granted>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HighWater {
    /// Decimal, since a TOML integer cannot hold every `u64`.
    version: String,
    payload_sha256: String,
}

/// The record of the pair (`root`, `app`), if there is one.
pub fn load_pair(layout: &Layout, root: &KeyId, app: &Identifier) -> Result<Option<Pair>, Error> {
    let path = layout.pair_grants_file(root, app);
    let Some(text) = read(&path)? else {
        return Ok(None);
    };
    let damaged = |reason: String| Error::Damaged {
        path: path.clone(),
        reason,
    };
    let file: PairFile = toml::from_str(&text).map_err(|e| damaged(e.to_string()))?;
    if file.grant_version != PAIR_VERSION {
        return Err(damaged(format!(
            "unknown grant_version {}",
            file.grant_version
        )));
    }
    if file.app_id != app.as_str() {
        return Err(damaged(format!("it names {:?}", file.app_id)));
    }
    let key = crate::trust::unhex::<32>(&file.root_public_key)
        .map(PublicKey::from_bytes)
        .ok_or_else(|| damaged("its root_public_key is not 64 lowercase hex digits".into()))?;
    // The directory is named for the root, so a record under another root's name is not this one.
    if key.key_id() != *root {
        return Err(damaged(format!("its root is {}, not {root}", key.key_id())));
    }
    let publisher = Publisher::parse(&file.publisher).map_err(|e| damaged(e.to_string()))?;
    let read_mark = |h: HighWater, field: &str| -> Result<Mark, Error> {
        Ok(Mark {
            version: h
                .version
                .parse()
                .map_err(|_| damaged(format!("{field}.version {:?} is not a number", h.version)))?,
            payload_sha256: crate::trust::unhex::<32>(&h.payload_sha256).ok_or_else(|| {
                damaged(format!(
                    "{field}.payload_sha256 is not 64 lowercase hex digits"
                ))
            })?,
        })
    };
    let mark = file
        .high_water
        .map(|h| read_mark(h, "high_water"))
        .transpose()?;
    let installed = file
        .installed
        .map(|h| read_mark(h, "installed"))
        .transpose()?;
    let permissions = file
        .permissions
        .map(Granted::permissions)
        .transpose()
        .map_err(damaged)?;
    Ok(Some(Pair {
        root: key,
        publisher,
        permissions,
        mark,
        installed,
        denied: file.denied,
    }))
}

/// Writes the record of the pair (`pair.root`, `app`), replacing any earlier one.
pub fn save_pair(layout: &Layout, app: &Identifier, pair: &Pair) -> Result<(), Error> {
    let file = PairFile {
        grant_version: PAIR_VERSION,
        app_id: app.as_str().to_owned(),
        root_public_key: crate::trust::hex(pair.root.as_bytes()),
        publisher: pair.publisher.to_string(),
        denied: pair.denied,
        high_water: pair.mark.map(|m| HighWater {
            version: m.version.to_string(),
            payload_sha256: crate::trust::hex(&m.payload_sha256),
        }),
        installed: pair.installed.map(|m| HighWater {
            version: m.version.to_string(),
            payload_sha256: crate::trust::hex(&m.payload_sha256),
        }),
        permissions: pair.permissions.as_ref().map(Granted::of),
    };
    write_private(
        &layout.pair_grants_file(&pair.root.key_id(), app),
        &toml::to_string(&file).expect("a pair's record serialises"),
    )
}

/// The version of `root`'s key set last accepted (ADR 0060, section 3), or `None` before one is
/// stored: a pin made at first contact before this was kept takes the next set that verifies
/// under it as the first.
pub fn keys_version(layout: &Layout, root: &KeyId) -> Result<Option<u64>, Error> {
    let path = layout.keys_version_file(root);
    let Some(text) = read(&path)? else {
        return Ok(None);
    };
    text.trim().parse().map(Some).map_err(|_| Error::Damaged {
        path,
        reason: format!("{:?} is not a version", text.trim()),
    })
}

pub fn save_keys_version(layout: &Layout, root: &KeyId, version: u64) -> Result<(), Error> {
    write_private(&layout.keys_version_file(root), &format!("{version}\n"))
}

/// Every pair that has a record, for asking which roots are pinned and from which hosts. A
/// directory or file that is not a pair's is skipped; a pair's record that is damaged is an error.
pub fn pairs(layout: &Layout) -> Result<Vec<(Identifier, Pair)>, Error> {
    let top = layout.pairs_dir();
    let roots = match fs::read_dir(&top) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path: top, source }),
    };
    let mut found = Vec::new();
    for root in roots.flatten() {
        let Some(root_id) = root
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<KeyId>().ok())
        else {
            continue;
        };
        let Ok(apps) = fs::read_dir(root.path()) else {
            continue;
        };
        for app in apps.flatten() {
            let name = app.file_name();
            let Some(app_id) = name
                .to_str()
                .and_then(|n| n.strip_suffix(".toml"))
                .and_then(|n| n.parse::<Identifier>().ok())
            else {
                continue;
            };
            if let Some(pair) = load_pair(layout, &root_id, &app_id)? {
                found.push((app_id, pair));
            }
        }
    }
    found.sort_by(|a, b| (a.1.root.key_id(), a.0.as_str()).cmp(&(b.1.root.key_id(), b.0.as_str())));
    Ok(found)
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

    fn pair(seed: u8) -> Pair {
        use crate::trust::SigningKey;
        Pair {
            root: SigningKey::from_seed(&[seed; 32]).unwrap().public_key(),
            publisher: Publisher::parse("apps.csfh.dev").unwrap(),
            permissions: Some(everything()),
            mark: Some(Mark {
                version: u64::MAX,
                payload_sha256: [0xab; 32],
            }),
            installed: Some(Mark {
                version: 3,
                payload_sha256: [0xcd; 32],
            }),
            denied: false,
        }
    }

    #[test]
    fn a_pairs_record_loads_back_is_private_and_is_found() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path(), dir.path());
        let record = pair(1);
        let root = record.root.key_id();
        save_pair(&layout, &id(), &record).unwrap();
        assert_eq!(
            load_pair(&layout, &root, &id()).unwrap(),
            Some(record.clone())
        );
        let path = layout.pair_grants_file(&root, &id());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(pairs(&layout).unwrap(), vec![(id(), record.clone())]);

        let bare = Pair {
            permissions: None,
            mark: None,
            installed: None,
            denied: true,
            ..record
        };
        save_pair(&layout, &id(), &bare).unwrap();
        assert_eq!(load_pair(&layout, &root, &id()).unwrap(), Some(bare));
    }

    #[test]
    fn a_pairs_record_under_another_roots_name_or_damaged_is_an_error() {
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path(), dir.path());
        save_pair(&layout, &id(), &pair(1)).unwrap();
        let one = pair(1).root.key_id();
        let two = pair(2).root.key_id();
        let path = layout.pair_grants_file(&one, &id());
        let text = fs::read_to_string(&path).unwrap();
        // The record of root 1, copied to where root 2's would be.
        let moved = layout.pair_grants_file(&two, &id());
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::write(&moved, &text).unwrap();
        assert!(matches!(
            load_pair(&layout, &two, &id()),
            Err(Error::Damaged { .. })
        ));
        for (from, to) in [
            ("grant_version = 2", "grant_version = 1"),
            ("denied = false", "denied = false\nextra = 1"),
            ("app_id = \"com.example.t\"", "app_id = \"com.example.u\""),
            (
                "publisher = \"apps.csfh.dev\"",
                "publisher = \"Apps.csfh.dev\"",
            ),
        ] {
            assert!(text.contains(from), "{from}");
            fs::write(&path, text.replacen(from, to, 1)).unwrap();
            assert!(
                matches!(load_pair(&layout, &one, &id()), Err(Error::Damaged { .. })),
                "{to}"
            );
        }
    }

    #[test]
    fn a_record_from_before_adr_0060_is_not_a_pairs() {
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path(), dir.path());
        save(&layout, &id(), &everything()).unwrap();
        assert!(pairs(&layout).unwrap().is_empty());
        assert_eq!(
            load_pair(&layout, &pair(1).root.key_id(), &id()).unwrap(),
            None
        );
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
