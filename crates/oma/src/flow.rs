//! The user-facing flows that need consent (ADR 0033): one consent step shared by `run`,
//! `install` and `consent`. Everything privileged lives elsewhere; this decides what to ask,
//! acts on the answer and fails closed on anything unexpected.

use std::fs;
use std::path::{Path, PathBuf};

use crate::consent::{self, Button, Identity, Request};
use crate::daemon::{self, Stopper};
use crate::fmt::{Bundle, Identifier, Permissions};
use crate::image::Image;
use crate::layout::Layout;
use crate::sandbox::launch::{self, HostEnv, Options};
use crate::trust::keys::Signer;
use crate::trust::{
    self, KeyId, Mark, Publisher, Sealed, SelfSigned, SetChange, Timestamp, Verdict, VerifiedKeys,
};
use crate::{dev, fetch, grants, image, install, userdirs};

/// Shows the dialog. The real one is [`consent::ask`].
pub type Ask<'a> = &'a dyn Fn(&Request) -> Result<Button, consent::Error>;

/// Fetches a publisher's key set. The real one is [`fetch::oma_keys`] with [`fetch::agent`].
pub type Fetch<'a> = &'a dyn Fn(&Publisher) -> Result<fetch::KeySet, fetch::Failure>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Launch(#[from] launch::Error),
    #[error(transparent)]
    Install(#[from] install::Error),
    #[error(transparent)]
    Grants(#[from] grants::Error),
    #[error(transparent)]
    Dev(#[from] dev::Error),
    #[error("cannot stop the daemon of the copy being replaced: {0}")]
    StopDaemon(String),
    #[error(
        "{0}\nno consent was recorded; grant the permissions without a dialog with `odk oma install --accept-permissions`"
    )]
    Dialog(consent::Error),
    #[error("cancelled; nothing was changed or started")]
    Cancelled,
    #[error("{app} is signed under publisher key {named}, and the key set at hand is {fetched}'s")]
    OtherRoot {
        app: Identifier,
        named: KeyId,
        fetched: KeyId,
    },
    #[error("{app}'s release key {key} is not in its publisher's key set")]
    NotAuthorized { app: Identifier, key: KeyId },
    #[error("{app} was fetched from {host}, which is not its publisher, {publisher}")]
    HostMismatch {
        app: Identifier,
        host: String,
        publisher: Publisher,
    },
    #[error(
        "{app} under publisher key {root} was declined before, so it is not asked again; `odk oma install` of the file can ask"
    )]
    Denied { app: Identifier, root: KeyId },
    #[error(
        "{app}'s publisher key {root} is not pinned, and --accept-permissions does not pin one: install it with the dialog"
    )]
    NewRootNotAccepted { app: Identifier, root: KeyId },
    #[error("{app}'s publisher key {root} is not pinned, and --register does not ask")]
    NotPinned { app: Identifier, root: KeyId },
    #[error(
        "the pinned key with id {0} is another key than this file's root; two keys with one id have no rule yet (docs/oma-trust-plan.md, question 11)"
    )]
    KeyIdCollision(KeyId),
    #[error(
        "{app} asks for more than its publisher's pin allows; asking for more under a pinned root is not built yet (issue #14)"
    )]
    MoreThanAllowed { app: Identifier },
    #[error(transparent)]
    Trust(#[from] trust::Error),
    #[error(transparent)]
    Image(#[from] image::Error),
    #[error("{0}; nothing was installed, and no publisher key was pinned")]
    Fetch(fetch::Failure),
    #[error("{app}'s signature expired at {}; it is not installed", .at.as_str())]
    Expired { app: Identifier, at: Timestamp },
    #[error("{app}'s release key {key} expired at {}; it does not authorize an install", .at.as_str())]
    ReleaseKeyExpired {
        app: Identifier,
        key: KeyId,
        at: Timestamp,
    },
    #[error(
        "this {app} is not the installed one; install it with `odk oma install` (opening a Bundle file that is not installed is not built yet, issue #14)"
    )]
    NotInstalledFile { app: Identifier },
    #[error(
        "{app} {version} is not newer than the installed {marked}, or is that version with other contents; it is not installed"
    )]
    Version {
        app: Identifier,
        version: u64,
        marked: u64,
    },
    #[error(
        "the publisher's key set is version {fetched}, older than the version {stored} already accepted; nothing was installed"
    )]
    KeySetLower { stored: u64, fetched: u64 },
}

/// What the Bundle's consent record does not cover yet.
fn missing(layout: &Layout, bundle: &Bundle) -> Result<Permissions, grants::Error> {
    let manifest = bundle.manifest();
    let granted = grants::load(layout, &manifest.identifier)?.unwrap_or_default();
    Ok(grants::missing(&granted, &manifest.permissions))
}

fn dialog(
    layout: &Layout,
    bundle: &Bundle,
    missing: &Permissions,
    buttons: &[Button],
    ask: Ask,
) -> Result<Button, Error> {
    let named_dirs = userdirs::resolve(layout.home(), &layout.user_dirs_file());
    let request = Request::new(bundle, missing, buttons, &named_dirs);
    match ask(&request).map_err(Error::Dialog)? {
        Button::Cancel => Err(Error::Cancelled),
        answer => Ok(answer),
    }
}

/// A Bundle that is neither installed nor inside `~/Applications` is being opened for the first
/// time from somewhere else (ADR 0018).
fn opened_from_outside(layout: &Layout, bundle: &Bundle) -> bool {
    let applications =
        std::fs::canonicalize(layout.applications()).unwrap_or_else(|_| layout.applications());
    !bundle.root().starts_with(&applications)
        && install::find(layout, &bundle.manifest().identifier).is_none()
}

/// `odk oma run`: asks for consent if the Bundle needs any, then runs it. Returns the status
/// `odk oma run` exits with.
pub fn run(
    layout: &Layout,
    host: &HostEnv,
    target: &str,
    wait: bool,
    ask: Ask,
) -> Result<i32, Error> {
    // A Bundle file: the path given, or the installed one for an Identifier with no installed
    // directory Bundle.
    let file = match target.parse::<Identifier>() {
        Ok(id) if install::find(layout, &id).is_none() => install::find_file(layout, &id),
        Ok(_) => None,
        Err(_) => Some(PathBuf::from(target)).filter(|p| p.is_file()),
    };
    if let Some(file) = file {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        return run_file(layout, host, &file, wait, now);
    }
    let bundle = launch::resolve(layout, target)?;
    launch::preflight(&bundle)?;
    let missing = missing(layout, &bundle)?;
    let mut options = Options {
        wait,
        run_once: false,
    };

    let bundle = if grants::is_empty(&missing) {
        bundle
    } else if opened_from_outside(layout, &bundle) {
        // A daemon keeps running after the application closes, which a run once must not leave
        // behind (ADR 0050), so such a Bundle can only be installed.
        let buttons: &[Button] = if bundle.manifest().permissions.daemon.is_some() {
            &[Button::Install, Button::Cancel]
        } else {
            &[Button::Install, Button::RunOnce, Button::Cancel]
        };
        match dialog(layout, &bundle, &missing, buttons, ask)? {
            Button::Install => {
                // Install copies it and records the consent; the installed copy is what runs.
                let installed = install::install(layout, bundle.root(), true)?;
                Bundle::open(&installed).map_err(install::Error::from)?
            }
            _ => {
                // Run once: this launch only, with nothing written to the user's home.
                options.run_once = true;
                bundle
            }
        }
    } else {
        dialog(
            layout,
            &bundle,
            &missing,
            &[Button::Allow, Button::Cancel],
            ask,
        )?;
        grants::save(
            layout,
            &bundle.manifest().identifier,
            &bundle.manifest().permissions,
        )?;
        bundle
    };
    Ok(launch::run(layout, &bundle, host, &options)?)
}

/// `odk oma install`: with `accept_permissions` it records consent without a dialog; otherwise a
/// dialog asks if the Bundle needs consent.
pub fn install(
    layout: &Layout,
    source: &Path,
    accept_permissions: bool,
    ask: Ask,
) -> Result<PathBuf, Error> {
    let bundle = Bundle::open(source).map_err(install::Error::from)?;
    let missing = missing(layout, &bundle)?;
    let accept = if accept_permissions || grants::is_empty(&missing) {
        accept_permissions
    } else {
        launch::preflight(&bundle)?;
        dialog(
            layout,
            &bundle,
            &missing,
            &[Button::Install, Button::Cancel],
            ask,
        )?;
        true
    };
    Ok(install::install(layout, source, accept)?)
}

/// `odk oma consent`: asks about whatever the Bundle needs that has not been granted, and records
/// the answer. `Ok(false)` means there was nothing to ask.
pub fn consent(layout: &Layout, target: &str, ask: Ask) -> Result<bool, Error> {
    let bundle = launch::resolve(layout, target)?;
    let missing = missing(layout, &bundle)?;
    if grants::is_empty(&missing) {
        return Ok(false);
    }
    launch::preflight(&bundle)?;
    dialog(
        layout,
        &bundle,
        &missing,
        &[Button::Allow, Button::Cancel],
        ask,
    )?;
    grants::save(
        layout,
        &bundle.manifest().identifier,
        &bundle.manifest().permissions,
    )?;
    Ok(true)
}

/// What [`dev`] did, for `odk oma dev` to say.
#[derive(Debug)]
pub struct Prepared {
    pub identifier: Identifier,
    pub installed: PathBuf,
    /// An installed copy was replaced, and not only installed.
    pub replaced: bool,
    /// Something that is not `oma`'s unit still accepts connections on the daemon's socket, so
    /// `odk oma run` will use it, not the daemon of the Bundle just installed (ADR 0051).
    pub foreign_daemon: Option<PathBuf>,
}

/// Removes the directory a Bundle was assembled in, whichever way `dev` ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `odk oma dev` up to running (ADR 0051): builds the project in `dir`, assembles its Bundle,
/// asks for what its consent record lacks, stops the daemon of the copy it replaces and installs
/// over it. The steps come in that order so that a build that fails, or a Cancel, changes nothing
/// and stops nothing. Running the installed copy is [`run`]'s, by Identifier.
pub fn dev(
    layout: &Layout,
    dir: &Path,
    runtime_dir: Option<&Path>,
    ask: Ask,
    stopper: &dyn Stopper,
) -> Result<Prepared, Error> {
    let project = dev::Project::load(dir)?;
    project.build()?;

    let scratch = Scratch(layout.dev_dir().join(std::process::id().to_string()));
    let assembled = project.assemble(&scratch.0).map_err(Error::from)?;
    let bundle = Bundle::open(&assembled).map_err(install::Error::from)?;
    let manifest = bundle.manifest();

    let missing = missing(layout, &bundle)?;
    let accept = if grants::is_empty(&missing) {
        false
    } else {
        launch::preflight(&bundle)?;
        dialog(
            layout,
            &bundle,
            &missing,
            &[Button::Install, Button::Cancel],
            ask,
        )?;
        true
    };

    let declared = manifest.permissions.daemon.as_ref();
    if declared.is_some() {
        stopper
            .stop(&manifest.identifier)
            .map_err(Error::StopDaemon)?;
    }
    let (installed, replaced) = install::reinstall(layout, &assembled, accept)?;
    let foreign_daemon = declared
        .zip(runtime_dir)
        .filter(|(daemon, runtime)| daemon::serving(runtime, daemon))
        .map(|(daemon, runtime)| runtime.join(&daemon.socket));
    Ok(Prepared {
        identifier: manifest.identifier.clone(),
        installed,
        replaced,
        foreign_daemon,
    })
}

/// How a signed file reached `odk-oma`. It decides what may be asked (ADR 0060, section 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occasion {
    /// Opening or launching the file.
    Launch,
    /// `--register`, which never opens a dialog.
    Register,
    /// `odk oma install`, which may ask again after a Cancel. With `accept_permissions` there is
    /// no dialog, and a root that is not pinned is refused.
    Install { accept_permissions: bool },
}

/// What the person allowed for a signed file's (root key id, app id) pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// The root was already pinned for this app id: nothing was asked.
    Pinned,
    /// The person pinned the root just now, and the pair's record holds the pin.
    Installed,
    /// This run only. Nothing was written: no pin, no key set, no denial.
    RunOnce,
}

/// The first-install decision for a signed file (ADR 0060, sections 2 and 7).
///
/// `file` has passed [`crate::trust::check`]. `keys` is its root's key set, verified on first
/// contact or under the pin. `fetched` is the URL the file was downloaded from, or `None` for a
/// file opened from disk, which has no download host. `requested` is what the file's verified
/// image asks for (`crate::image`), which the dialog lists (ADR 0067, which settles
/// `docs/oma-trust-plan.md`'s question 12).
///
/// A root not pinned for this app id is asked about even when nothing else would be: Install,
/// Run once and Cancel, or Install and Cancel for `odk oma install`. Install writes the pin and
/// the permissions, and Cancel writes a denial. Run once writes nothing. A host one edit away
/// from a host pinned for another root is confirmed on its own first. Matching a Unicode
/// confusable is open (question 15). A pinned root whose file asks for more than was allowed is
/// asked about by `odk oma install`, Install or Cancel, and Cancel there writes no denial; a
/// launch refuses it for now.
pub fn admit(
    layout: &Layout,
    file: &SelfSigned,
    keys: &VerifiedKeys,
    fetched: Option<&str>,
    occasion: Occasion,
    requested: &Permissions,
    ask: Ask,
) -> Result<Admission, Error> {
    let manifest = file.manifest();
    let app = &manifest.app_id;
    let root = keys.root();
    let root_id = root.key_id();
    if manifest.root_key_id != root_id {
        return Err(Error::OtherRoot {
            app: app.clone(),
            named: manifest.root_key_id,
            fetched: root_id,
        });
    }
    if keys.set().release_key(&file.header().release_key).is_none() {
        return Err(Error::NotAuthorized {
            app: app.clone(),
            key: file.header().release_key.key_id(),
        });
    }
    if let Some(url) = fetched {
        let host = url_host(url);
        if host != Some(manifest.publisher.as_str()) {
            return Err(Error::HostMismatch {
                app: app.clone(),
                host: host.unwrap_or(url).to_owned(),
                publisher: manifest.publisher.clone(),
            });
        }
    }

    let existing = grants::load_pair(layout, &root_id, app)?;
    if let Some(pair) = &existing {
        if pair.root != *root {
            return Err(Error::KeyIdCollision(root_id));
        }
        if !pair.denied {
            let granted = pair.permissions.clone().unwrap_or_default();
            let missing = grants::missing(&granted, requested);
            if grants::is_empty(&missing) {
                return Ok(Admission::Pinned);
            }
            let Occasion::Install { accept_permissions } = occasion else {
                return Err(Error::MoreThanAllowed { app: app.clone() });
            };
            if !accept_permissions {
                let identity = Identity {
                    app_id: app,
                    version: manifest.version,
                    root,
                    publisher: &manifest.publisher,
                    fetched,
                };
                let named_dirs = userdirs::resolve(layout.home(), &layout.user_dirs_file());
                let request = Request::more(&identity, &missing, &named_dirs);
                if ask(&request).map_err(Error::Dialog)? != Button::Install {
                    return Err(Error::Cancelled);
                }
            }
            let mut pair = pair.clone();
            pair.permissions = Some(requested.clone());
            grants::save_pair(layout, app, &pair)?;
            return Ok(Admission::Pinned);
        }
    }
    let denied = || Error::Denied {
        app: app.clone(),
        root: root_id,
    };
    match occasion {
        Occasion::Register if existing.is_some() => return Err(denied()),
        Occasion::Register => {
            return Err(Error::NotPinned {
                app: app.clone(),
                root: root_id,
            });
        }
        Occasion::Launch if existing.is_some() => return Err(denied()),
        Occasion::Install {
            accept_permissions: true,
        } => {
            return Err(Error::NewRootNotAccepted {
                app: app.clone(),
                root: root_id,
            });
        }
        Occasion::Launch | Occasion::Install { .. } => {}
    }

    let others = grants::pairs(layout)?;
    let identity = Identity {
        app_id: app,
        version: manifest.version,
        root,
        publisher: &manifest.publisher,
        fetched,
    };
    let refuse = |layout: &Layout| -> Result<Admission, Error> {
        let mut pair = existing.clone().unwrap_or(grants::Pair {
            root: *root,
            publisher: manifest.publisher.clone(),
            permissions: None,
            mark: None,
            denied: true,
        });
        pair.denied = true;
        grants::save_pair(layout, app, &pair)?;
        Err(Error::Cancelled)
    };

    let confusable = others.iter().find(|(_, other)| {
        other.root != *root
            && edit_distance(other.publisher.as_str(), manifest.publisher.as_str()) == 1
    });
    if let Some((_, other)) = confusable {
        match ask(&Request::confusable(&identity, &other.publisher)).map_err(Error::Dialog)? {
            Button::Confirm => {}
            _ => return refuse(layout),
        }
    }

    let also_for = others
        .iter()
        .find(|(other_app, other)| other.root == *root && other_app != app)
        .map(|(other_app, _)| other_app);
    let buttons: &[Button] = match occasion {
        Occasion::Install { .. } => &[Button::Install, Button::Cancel],
        _ => &[Button::Install, Button::RunOnce, Button::Cancel],
    };
    let named_dirs = userdirs::resolve(layout.home(), &layout.user_dirs_file());
    let request = Request::new_root(&identity, also_for, buttons, requested, &named_dirs);
    match ask(&request).map_err(Error::Dialog)? {
        Button::Install => {
            let pair = grants::Pair {
                root: *root,
                publisher: manifest.publisher.clone(),
                permissions: Some(requested.clone()),
                mark: existing.as_ref().and_then(|p| p.mark),
                denied: false,
            };
            grants::save_pair(layout, app, &pair)?;
            Ok(Admission::Installed)
        }
        Button::RunOnce => Ok(Admission::RunOnce),
        _ => refuse(layout),
    }
}

/// `odk oma install <file>` for a Bundle file (ADR 0052), at first contact with its publisher's
/// root (ADR 0060, sections 2 to 7). `now` is the time in Unix seconds.
///
/// Everything that can refuse the file does so before the dialog: its envelope and its image,
/// its signature window, an install name that is taken, and its root's key set, fetched from
/// the publisher, verified on first contact, and naming the file's release key inside its
/// `not_after`. Then the person is asked, Install or Cancel, with the image's permissions listed.
/// Install pins the root and records the permissions; then the sealed bytes are placed in
/// `~/Applications`, and only then is the high-water mark written. A failure after the dialog
/// leaves the pin, which the person gave.
///
/// Under a root already pinned, the key set is verified under the pin and never older than the
/// last accepted for the root (section 3), and a file of the same app id must be newer than its
/// mark (section 4): the same file again is a no-op. An upgrade asks only for permissions not
/// allowed yet, and keeps the file it replaces as the previous one. A revoked release key is
/// not checked: the revocation document has no schema yet (`docs/oma-trust-plan.md`,
/// question 2).
pub fn install_file(
    layout: &Layout,
    path: &Path,
    accept_permissions: bool,
    now: i64,
    ask: Ask,
    fetch: Fetch,
) -> Result<PathBuf, Error> {
    let sealed = Sealed::memfd(path)?;
    let checked = trust::check(&sealed)?;
    let image = Image::read(&checked)?;
    let file = checked.signed();
    let signed = file.manifest();
    let app = &signed.app_id;
    if signed.expires_at.unix_seconds() < now {
        return Err(Error::Expired {
            app: app.clone(),
            at: signed.expires_at.clone(),
        });
    }
    let target = layout
        .applications()
        .join(format!("{}.oma", image.manifest().name.replace('/', "-")));
    let mark = Mark {
        version: signed.version,
        payload_sha256: signed.payload_sha256,
    };

    let root_id = &signed.root_key_id;
    let pinned = grants::pairs(layout)?
        .into_iter()
        .filter(|(_, pair)| pair.root.key_id() == *root_id && !pair.denied)
        .collect::<Vec<_>>();
    let pair = pinned
        .iter()
        .find(|(other, _)| other == app)
        .map(|(_, p)| p);
    let installed = pair.and_then(|_| install::find_file(layout, app));

    // The high-water rule (section 4): the same file again is a no-op, and a lower version, or
    // the same version with another payload, is refused.
    match Verdict::of(
        pair.and_then(|p| p.mark.as_ref()),
        signed.version,
        &signed.payload_sha256,
    ) {
        Verdict::Same => {
            return match installed {
                Some(path) => Ok(path),
                None => {
                    install::place_file(&sealed, &target)?;
                    Ok(target)
                }
            };
        }
        Verdict::Lower | Verdict::SameVersionOtherPayload => {
            return Err(Error::Version {
                app: app.clone(),
                version: signed.version,
                marked: pair.and_then(|p| p.mark).map_or(0, |m| m.version),
            });
        }
        Verdict::First | Verdict::Higher => {}
    }
    if target.symlink_metadata().is_ok() && installed.as_deref() != Some(target.as_path()) {
        return Err(install::Error::Exists(target).into());
    }

    // The key set: under the pin when the root is pinned for any app id, and never older than
    // the last one accepted for the root (section 3).
    let fetched = fetch(&signed.publisher).map_err(Error::Fetch)?;
    let signer = match pinned.first() {
        Some((_, pair)) => Signer::Pinned(&pair.root),
        None => Signer::FirstContact,
    };
    let keys = trust::keys::verify(&fetched.body, &fetched.signature, signer)?;
    if keys.root().key_id() != *root_id {
        return Err(Error::OtherRoot {
            app: app.clone(),
            named: *root_id,
            fetched: keys.root().key_id(),
        });
    }
    let stored = grants::keys_version(layout, root_id)?;
    if SetChange::of(stored, keys.set().version) == SetChange::Lower {
        return Err(Error::KeySetLower {
            stored: stored.unwrap_or_default(),
            fetched: keys.set().version,
        });
    }
    let release_key = &file.header().release_key;
    let Some(release) = keys.set().release_key(release_key) else {
        return Err(Error::NotAuthorized {
            app: app.clone(),
            key: release_key.key_id(),
        });
    };
    if release.not_after.unix_seconds() < now {
        return Err(Error::ReleaseKeyExpired {
            app: app.clone(),
            key: release.id,
            at: release.not_after.clone(),
        });
    }

    let occasion = Occasion::Install { accept_permissions };
    let requested = &image.manifest().permissions;
    match admit(layout, file, &keys, None, occasion, requested, ask)? {
        Admission::Installed | Admission::Pinned => {}
        Admission::RunOnce => unreachable!("install offers Install and Cancel only"),
    }

    // The file it replaces is kept as the previous one (ADR 0011), and put back if the new one
    // cannot be placed.
    let previous = layout.pair_previous_file(root_id, app);
    if let Some(old) = &installed {
        let dir = previous.parent().expect("a pair's directory");
        fs::create_dir_all(dir).map_err(|e| install::Error::Io {
            path: dir.to_owned(),
            source: e,
        })?;
        fs::rename(old, &previous).map_err(|e| install::Error::Io {
            path: old.clone(),
            source: e,
        })?;
    }
    if let Err(e) = install::place_file(&sealed, &target) {
        if let Some(old) = &installed {
            let _ = fs::rename(&previous, old);
        }
        return Err(e.into());
    }
    let mut pair = grants::load_pair(layout, root_id, app)?.expect("the pair is pinned");
    pair.mark = Some(mark);
    grants::save_pair(layout, app, &pair)?;
    grants::save_keys_version(layout, root_id, stored.unwrap_or(0).max(keys.set().version))?;
    Ok(target)
}

/// `odk oma run` of an installed Bundle file (ADR 0060, sections 4 and 5; ADR 0067). It is
/// sealed and checked again, its image read, and it runs only as exactly the file its install
/// accepted: its pin is there, and its version and payload digest are the mark's. That ties the
/// launch to bytes whose release key was authorized then, with no key set fetched and no
/// network. Past its signature window it says so in one line and runs. A file that is not
/// installed, or another version of one that is, is not run yet: opening one is the
/// first-install or upgrade flow, which is not built (issue #14).
pub fn run_file(
    layout: &Layout,
    host: &HostEnv,
    path: &Path,
    wait: bool,
    now: i64,
) -> Result<i32, Error> {
    let sealed = Sealed::memfd(path)?;
    let checked = trust::check(&sealed)?;
    let image = Image::read(&checked)?;
    let signed = checked.signed().manifest();
    let app = &signed.app_id;
    let pair = grants::load_pair(layout, &signed.root_key_id, app)?
        .filter(|pair| !pair.denied && pair.root.key_id() == signed.root_key_id)
        .ok_or_else(|| Error::NotInstalledFile { app: app.clone() })?;
    let mark = Mark {
        version: signed.version,
        payload_sha256: signed.payload_sha256,
    };
    if pair.mark != Some(mark) {
        return Err(Error::NotInstalledFile { app: app.clone() });
    }
    if signed.expires_at.unix_seconds() < now {
        eprintln!("odk oma: {app} is past its signature window and keeps running.");
    }
    let granted = pair.permissions.unwrap_or_default();
    Ok(launch::run_file(
        layout,
        &image,
        &signed.root_key_id,
        &granted,
        host,
        wait,
    )?)
}

/// The host of an `https` URL: what comes after `https://` and before the path, query or
/// fragment. `None` for any other scheme.
fn url_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    Some(&rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())])
}

/// The number of single-character insertions, deletions and substitutions between `a` and `b`.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous + usize::from(ca != *cb);
            previous = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;
    use crate::testutil::elf;

    struct Fixture {
        _dir: TempDir,
        layout: Layout,
        project: PathBuf,
    }

    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, elf()).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A home, and a project under it with a built executable, a daemon and a manifest, at `version`.
    fn fixture() -> Fixture {
        let dir = TempDir::new().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let layout = Layout::new(root.join("home"), root.join("home/.local/share"));
        let project = root.join("work/app");
        executable(&project.join("target/app"));
        executable(&project.join("../tool"));
        let f = Fixture {
            _dir: dir,
            layout,
            project,
        };
        f.write("1.0.0", "audio = true", "");
        f
    }

    impl Fixture {
        /// The project's files at `version`, with `permissions` and `extra` oma.toml lines.
        fn write(&self, version: &str, permissions: &str, extra: &str) {
            fs::write(
                self.project.join("manifest.toml"),
                format!(
                    "manifest_version = 1\nidentifier = \"com.example.dev\"\nname = \"Dev\"\nversion = \"{version}\"\nexecutable = \"bin/app\"\n\n[daemon]\nexecutable = \"bin/d\"\nsocket = \"dev/d.sock\"\n\n[permissions]\n{permissions}\n"
                ),
            )
            .unwrap();
            fs::write(
                self.project.join(dev::FILE),
                format!("executable = \"target/app\"\ninclude = [\"../tool=bin/d\"]\n{extra}"),
            )
            .unwrap();
        }

        fn installed_version(&self) -> Option<String> {
            install::find(&self.layout, &"com.example.dev".parse().unwrap())
                .map(|b| b.manifest().version.to_string())
        }
    }

    /// What the dialog was shown and answered with.
    struct Dialog {
        shown: Cell<usize>,
        answer: Button,
    }

    impl Dialog {
        fn answering(answer: Button) -> Self {
            Self {
                shown: Cell::new(0),
                answer,
            }
        }
        fn ask(&self) -> impl Fn(&Request) -> Result<Button, consent::Error> + '_ {
            |_| {
                self.shown.set(self.shown.get() + 1);
                Ok(self.answer)
            }
        }
    }

    struct Stops(RefCell<Vec<Identifier>>);

    impl Stopper for Stops {
        fn stop(&self, id: &Identifier) -> Result<(), String> {
            self.0.borrow_mut().push(id.clone());
            Ok(())
        }
    }

    fn stops() -> Stops {
        Stops(RefCell::new(Vec::new()))
    }

    #[test]
    fn the_first_run_asks_once_and_a_later_one_replaces_the_copy_without_asking() {
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        let stopper = stops();

        let first = dev(&f.layout, &f.project, None, &dialog.ask(), &stopper).unwrap();
        assert!(!first.replaced);
        assert_eq!(dialog.shown.get(), 1, "it asks for what is not granted");
        assert_eq!(f.installed_version().as_deref(), Some("1.0.0"));

        f.write("1.1.0", "audio = true", "");
        let second = dev(&f.layout, &f.project, None, &dialog.ask(), &stopper).unwrap();
        assert!(second.replaced);
        assert_eq!(
            dialog.shown.get(),
            1,
            "the grant is unchanged, so no dialog"
        );
        assert_eq!(f.installed_version().as_deref(), Some("1.1.0"));
        assert_eq!(
            fs::read_dir(f.layout.applications()).unwrap().count(),
            1,
            "nothing is left beside the new copy"
        );

        // The daemon is stopped before each install, by the name `ensure` starts it under.
        let stopped = stopper.0.borrow();
        assert_eq!(stopped.len(), 2);
        assert_eq!(
            daemon::unit_name(&stopped[0]),
            "oma-com.example.dev".to_owned()
        );
        let root = second.installed;
        assert!(root.join("bin/d").exists(), "the include is in");
        assert!(!root.join(dev::FILE).exists(), "oma.toml is not");
    }

    #[test]
    fn a_failing_build_changes_nothing_and_asks_nothing() {
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        let stopper = stops();
        dev(&f.layout, &f.project, None, &dialog.ask(), &stopper).unwrap();
        let (shown, stopped) = (dialog.shown.get(), stopper.0.borrow().len());

        f.write(
            "2.0.0",
            "audio = true\nvideo = true",
            "build = [[\"sh\", \"-c\", \"exit 1\"]]\n",
        );
        let e = dev(&f.layout, &f.project, None, &dialog.ask(), &stopper).unwrap_err();
        assert!(matches!(e, Error::Dev(dev::Error::Build { .. })), "{e}");
        assert_eq!(f.installed_version().as_deref(), Some("1.0.0"));
        assert_eq!(dialog.shown.get(), shown);
        assert_eq!(stopper.0.borrow().len(), stopped, "nothing was stopped");
    }

    #[test]
    fn cancelling_leaves_the_installed_copy_and_its_daemon_alone() {
        let f = fixture();
        let stopper = stops();
        dev(
            &f.layout,
            &f.project,
            None,
            &Dialog::answering(Button::Install).ask(),
            &stopper,
        )
        .unwrap();
        let stopped = stopper.0.borrow().len();

        // More permissions than were granted: it asks, and the answer is no.
        f.write("2.0.0", "audio = true\ngpu = true", "");
        let no = Dialog::answering(Button::Cancel);
        let e = dev(&f.layout, &f.project, None, &no.ask(), &stopper).unwrap_err();
        assert!(matches!(e, Error::Cancelled), "{e}");
        assert_eq!(no.shown.get(), 1);
        assert_eq!(f.installed_version().as_deref(), Some("1.0.0"));
        assert_eq!(stopper.0.borrow().len(), stopped);
        assert!(
            !f.layout
                .dev_dir()
                .join(std::process::id().to_string())
                .exists(),
            "the scratch directory is gone"
        );
    }

    #[test]
    fn a_daemon_that_something_else_serves_is_reported() {
        let f = fixture();
        let runtime = f.project.parent().unwrap().join("run");
        fs::create_dir_all(runtime.join("dev")).unwrap();
        let _listener = std::os::unix::net::UnixListener::bind(runtime.join("dev/d.sock")).unwrap();
        let prepared = dev(
            &f.layout,
            &f.project,
            Some(&runtime),
            &Dialog::answering(Button::Install).ask(),
            &stops(),
        )
        .unwrap();
        assert_eq!(prepared.foreign_daemon, Some(runtime.join("dev/d.sock")));
    }

    #[test]
    fn oma_install_still_refuses_an_identifier_that_is_installed() {
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        let prepared = dev(&f.layout, &f.project, None, &dialog.ask(), &stops()).unwrap();
        let e = install(&f.layout, &prepared.installed, true, &dialog.ask()).unwrap_err();
        assert!(
            matches!(e, Error::Install(install::Error::AlreadyInstalled { .. })),
            "{e}"
        );
    }
}

#[cfg(test)]
mod signed_tests {
    use std::cell::RefCell;

    use tempfile::TempDir;

    use super::*;
    use crate::trust::keys::{self, OmaKeys, ReleaseKey, Signer};
    use crate::trust::manifest::tests::sample;
    use crate::trust::{Context, Sealed, SigningKey, Timestamp};

    // Fixture keys, never real ones.
    fn release_key() -> SigningKey {
        SigningKey::from_seed(&[0x42; 32]).unwrap()
    }

    fn root(seed: u8) -> SigningKey {
        SigningKey::from_seed(&[seed; 32]).unwrap()
    }

    fn layout(dir: &TempDir) -> Layout {
        Layout::new(dir.path().join("home"), dir.path().join("data"))
    }

    /// A file for `app` from `host`, under the root of `root_seed`, that passes `check`.
    fn file(dir: &TempDir, app: &str, host: &str, root_seed: u8) -> SelfSigned {
        let mut manifest = sample();
        manifest.app_id = app.parse().unwrap();
        manifest.publisher = Publisher::parse(host).unwrap();
        manifest.update_url = format!("https://{host}/app.oma");
        manifest.root_key_id = root(root_seed).public_key().key_id();
        let bytes = crate::trust::stamp(b"an image", manifest, &release_key()).unwrap();
        let path = dir.path().join(format!("{app}-{root_seed}.oma"));
        std::fs::write(&path, bytes).unwrap();
        crate::trust::check(&Sealed::memfd(&path).unwrap())
            .unwrap()
            .into_signed()
    }

    /// The root of `root_seed`'s key set, naming the fixture release key, verified on first
    /// contact.
    fn key_set(root_seed: u8) -> VerifiedKeys {
        let set = key_set_bytes(root_seed, &release_key());
        keys::verify(&set.body, &set.signature, Signer::FirstContact).unwrap()
    }

    /// The root of `root_seed`'s key set naming `release` until 2027-01-01, as a publisher
    /// serves it: the body and the root's signature.
    fn key_set_bytes(root_seed: u8, release: &SigningKey) -> fetch::KeySet {
        key_set_version(root_seed, release, 1)
    }

    fn key_set_version(root_seed: u8, release: &SigningKey, version: u64) -> fetch::KeySet {
        let root = root(root_seed);
        let release = release.public_key();
        let set = OmaKeys {
            root_public_key: root.public_key(),
            root_key_id: root.public_key().key_id(),
            version,
            keys: vec![ReleaseKey {
                id: release.key_id(),
                not_after: Timestamp::parse("2027-01-01T00:00:00Z").unwrap(),
                public_key: release,
            }],
        };
        let body = set.to_canonical().unwrap();
        let signature = root.sign(Context::KEYS, &body).0.to_vec();
        fetch::KeySet { body, signature }
    }

    /// A dialog that answers `answers` in turn and keeps every request it was shown.
    struct Dialog {
        answers: RefCell<Vec<Button>>,
        shown: RefCell<Vec<Request>>,
    }

    impl Dialog {
        fn answering(answers: &[Button]) -> Self {
            Self {
                answers: RefCell::new(answers.iter().rev().copied().collect()),
                shown: RefCell::new(Vec::new()),
            }
        }

        fn ask(&self, request: &Request) -> Result<Button, consent::Error> {
            self.shown.borrow_mut().push(request.clone());
            Ok(self
                .answers
                .borrow_mut()
                .pop()
                .expect("an answer for every dialog"))
        }
    }

    fn admit_with(
        layout: &Layout,
        file: &SelfSigned,
        keys: &VerifiedKeys,
        occasion: Occasion,
        dialog: &Dialog,
    ) -> Result<Admission, Error> {
        admit(
            layout,
            file,
            keys,
            None,
            occasion,
            &Permissions::default(),
            &|r| dialog.ask(r),
        )
    }

    const APP: &str = "dev.csfh.notes";
    const HOST: &str = "apps.csfh.dev";

    #[test]
    fn a_new_root_is_asked_about_even_when_nothing_else_would_be() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let dialog = Dialog::answering(&[Button::Install]);
        let f = file(&dir, APP, HOST, 1);
        assert_eq!(
            admit_with(&layout, &f, &key_set(1), Occasion::Launch, &dialog).unwrap(),
            Admission::Installed
        );
        let shown = dialog.shown.borrow();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].buttons, ["install", "run-once", "cancel"]);
        assert!(
            shown[0].lines.is_empty(),
            "no permission source is invented"
        );
        let fingerprint = root(1).public_key().fingerprint().to_string();
        let grouped = fingerprint
            .as_bytes()
            .chunks(8)
            .map(|g| std::str::from_utf8(g).unwrap())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            shown[0].notes,
            [format!(
                "This pins the publisher key {grouped} for {APP} 7. The publisher host is {HOST} ({HOST})."
            )]
        );
        // Named from the signed app id, never from the image.
        assert_eq!(
            (shown[0].name.as_str(), shown[0].version.as_str()),
            (APP, "7")
        );

        let pair = grants::load_pair(
            &layout,
            &root(1).public_key().key_id(),
            &APP.parse().unwrap(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(pair.root, root(1).public_key());
        // Install records what the file asked for, here nothing beyond the baseline.
        assert!(!pair.denied && pair.mark.is_none());
        assert_eq!(pair.permissions, Some(Permissions::default()));

        // Pinned now: the same file is not asked about again.
        let again = Dialog::answering(&[]);
        assert_eq!(
            admit_with(&layout, &f, &key_set(1), Occasion::Launch, &again).unwrap(),
            Admission::Pinned
        );
        assert!(again.shown.borrow().is_empty());
    }

    #[test]
    fn run_once_leaves_no_pin_and_cancel_stores_a_denial_and_nothing_else() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let f = file(&dir, APP, HOST, 1);
        let once = Dialog::answering(&[Button::RunOnce]);
        assert_eq!(
            admit_with(&layout, &f, &key_set(1), Occasion::Launch, &once).unwrap(),
            Admission::RunOnce
        );
        assert!(
            grants::pairs(&layout).unwrap().is_empty(),
            "run once wrote nothing"
        );

        let cancel = Dialog::answering(&[Button::Cancel]);
        assert!(matches!(
            admit_with(&layout, &f, &key_set(1), Occasion::Launch, &cancel),
            Err(Error::Cancelled)
        ));
        let (_, pair) = grants::pairs(&layout).unwrap().pop().unwrap();
        assert!(pair.denied && pair.permissions.is_none() && pair.mark.is_none());
    }

    #[test]
    fn a_denial_suppresses_launch_and_register_and_not_install() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let f = file(&dir, APP, HOST, 1);
        let _ = admit_with(
            &layout,
            &f,
            &key_set(1),
            Occasion::Launch,
            &Dialog::answering(&[Button::Cancel]),
        );

        for occasion in [Occasion::Launch, Occasion::Register] {
            let silent = Dialog::answering(&[]);
            assert!(matches!(
                admit_with(&layout, &f, &key_set(1), occasion, &silent),
                Err(Error::Denied { .. })
            ));
            assert!(silent.shown.borrow().is_empty(), "{occasion:?} asked again");
        }

        let install = Dialog::answering(&[Button::Install]);
        let occasion = Occasion::Install {
            accept_permissions: false,
        };
        assert_eq!(
            admit_with(&layout, &f, &key_set(1), occasion, &install).unwrap(),
            Admission::Installed
        );
        assert_eq!(install.shown.borrow().len(), 1);
        let (_, pair) = grants::pairs(&layout).unwrap().pop().unwrap();
        assert!(!pair.denied);
    }

    #[test]
    fn accept_permissions_and_register_never_pin_a_new_root() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let f = file(&dir, APP, HOST, 1);
        let silent = Dialog::answering(&[]);
        let accept = Occasion::Install {
            accept_permissions: true,
        };
        assert!(matches!(
            admit_with(&layout, &f, &key_set(1), accept, &silent),
            Err(Error::NewRootNotAccepted { .. })
        ));
        assert!(matches!(
            admit_with(&layout, &f, &key_set(1), Occasion::Register, &silent),
            Err(Error::NotPinned { .. })
        ));
        assert!(silent.shown.borrow().is_empty());
        assert!(grants::pairs(&layout).unwrap().is_empty());

        // Once the root is pinned, accepting permissions does not need the dialog.
        let _ = admit_with(
            &layout,
            &f,
            &key_set(1),
            Occasion::Launch,
            &Dialog::answering(&[Button::Install]),
        );
        assert_eq!(
            admit_with(&layout, &f, &key_set(1), accept, &silent).unwrap(),
            Admission::Pinned
        );
    }

    #[test]
    fn the_same_app_id_under_two_roots_shares_nothing() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let first = file(&dir, APP, HOST, 1);
        let second = file(&dir, APP, HOST, 2);
        let _ = admit_with(
            &layout,
            &first,
            &key_set(1),
            Occasion::Launch,
            &Dialog::answering(&[Button::Install]),
        );
        // Another root at the same host is a new publisher, asked about from the start.
        let dialog = Dialog::answering(&[Button::Install]);
        assert_eq!(
            admit_with(&layout, &second, &key_set(2), Occasion::Launch, &dialog).unwrap(),
            Admission::Installed
        );
        assert_eq!(dialog.shown.borrow().len(), 1);
        let app: Identifier = APP.parse().unwrap();
        let (a, b) = (root(1).public_key().key_id(), root(2).public_key().key_id());
        assert_ne!(
            layout.pair_grants_file(&a, &app),
            layout.pair_grants_file(&b, &app)
        );
        assert_ne!(
            layout.pair_data_dir(&a, &app),
            layout.pair_data_dir(&b, &app)
        );
        assert_ne!(layout.pair_dir(&a, &app), layout.pair_dir(&b, &app));
        assert_eq!(grants::pairs(&layout).unwrap().len(), 2);
    }

    #[test]
    fn a_second_app_under_a_pinned_root_is_its_own_consent_and_names_both() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let notes = file(&dir, APP, HOST, 1);
        let todo = file(&dir, "dev.csfh.todo", HOST, 1);
        let _ = admit_with(
            &layout,
            &notes,
            &key_set(1),
            Occasion::Launch,
            &Dialog::answering(&[Button::Install]),
        );
        let dialog = Dialog::answering(&[Button::Install]);
        admit_with(&layout, &todo, &key_set(1), Occasion::Launch, &dialog).unwrap();
        let shown = dialog.shown.borrow();
        assert_eq!(shown.len(), 1);
        assert!(
            shown[0]
                .notes
                .iter()
                .any(|n| n.contains(APP) && n.contains("dev.csfh.todo is another application")),
            "{:?}",
            shown[0].notes
        );
    }

    #[test]
    fn a_host_one_edit_from_another_roots_is_confirmed_on_its_own_first() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let real = file(&dir, APP, HOST, 1);
        let _ = admit_with(
            &layout,
            &real,
            &key_set(1),
            Occasion::Launch,
            &Dialog::answering(&[Button::Install]),
        );
        let lookalike = file(&dir, "dev.csfh.notez", "apps.csfh.dew", 2);

        // Cancel on the warning installs nothing, offers nothing more, and is a denial.
        let cancel = Dialog::answering(&[Button::Cancel]);
        assert!(matches!(
            admit_with(&layout, &lookalike, &key_set(2), Occasion::Launch, &cancel),
            Err(Error::Cancelled)
        ));
        let shown = cancel.shown.borrow();
        assert_eq!(shown.len(), 1);
        assert_eq!(
            shown[0].buttons,
            ["confirm", "cancel"],
            "no answer that installs"
        );
        assert!(
            shown[0].notes[0].contains(&format!("apps.csfh.dew looks like {HOST}")),
            "{:?}",
            shown[0].notes
        );
        drop(shown);

        // Confirm only dismisses the warning; Install is a second, separate answer.
        let install = Occasion::Install {
            accept_permissions: false,
        };
        let confirm = Dialog::answering(&[Button::Confirm, Button::Install]);
        assert_eq!(
            admit_with(&layout, &lookalike, &key_set(2), install, &confirm).unwrap(),
            Admission::Installed
        );
        let shown = confirm.shown.borrow();
        assert_eq!(shown.len(), 2);
        // `odk oma install` installs or does nothing: it offers no Run once.
        assert_eq!(shown[1].buttons, ["install", "cancel"]);
    }

    #[test]
    fn a_host_two_edits_away_or_the_same_roots_own_is_not_confusable() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let _ = admit_with(
            &layout,
            &file(&dir, APP, HOST, 1),
            &key_set(1),
            Occasion::Launch,
            &Dialog::answering(&[Button::Install]),
        );
        for (app, host, seed) in [
            ("dev.csfh.a", "apps.csfh.dxx", 2),
            ("dev.csfh.b", "apps.csfh.dew", 1),
        ] {
            let dialog = Dialog::answering(&[Button::Install]);
            admit_with(
                &layout,
                &file(&dir, app, host, seed),
                &key_set(seed),
                Occasion::Launch,
                &dialog,
            )
            .unwrap();
            assert_eq!(
                dialog.shown.borrow()[0].buttons,
                ["install", "run-once", "cancel"],
                "{host}"
            );
        }
    }

    #[test]
    fn a_file_the_key_set_does_not_authorize_or_from_another_host_is_stopped_before_any_dialog() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let f = file(&dir, APP, HOST, 1);
        let silent = Dialog::answering(&[]);
        assert!(matches!(
            admit_with(&layout, &f, &key_set(2), Occasion::Launch, &silent),
            Err(Error::OtherRoot { .. })
        ));
        for url in [
            "https://other.example/app.oma",
            "http://apps.csfh.dev/app.oma",
            "https://apps.csfh.dev.evil.example/x",
        ] {
            assert!(
                matches!(
                    admit(
                        &layout,
                        &f,
                        &key_set(1),
                        Some(url),
                        Occasion::Launch,
                        &Permissions::default(),
                        &|r| silent.ask(r)
                    ),
                    Err(Error::HostMismatch { .. })
                ),
                "{url}"
            );
        }
        assert!(silent.shown.borrow().is_empty());

        // A fetched URL on the publisher's host is named in the dialog.
        let dialog = Dialog::answering(&[Button::RunOnce]);
        let url = "https://apps.csfh.dev/notes.oma";
        admit(
            &layout,
            &f,
            &key_set(1),
            Some(url),
            Occasion::Launch,
            &Permissions::default(),
            &|r| dialog.ask(r),
        )
        .unwrap();
        assert!(
            dialog.shown.borrow()[0]
                .notes
                .iter()
                .any(|n| n == &format!("It was fetched from {url}."))
        );
    }

    #[test]
    fn a_release_key_the_roots_set_does_not_list_is_stopped_before_any_dialog() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let f = file(&dir, APP, HOST, 1);
        // Root 1's set, naming another release key than the one that signed the file.
        let root = root(1);
        let other = SigningKey::from_seed(&[0x43; 32]).unwrap().public_key();
        let set = OmaKeys {
            root_public_key: root.public_key(),
            root_key_id: root.public_key().key_id(),
            version: 1,
            keys: vec![ReleaseKey {
                id: other.key_id(),
                not_after: Timestamp::parse("2027-01-01T00:00:00Z").unwrap(),
                public_key: other,
            }],
        };
        let body = set.to_canonical().unwrap();
        let keys = keys::verify(
            &body,
            &root.sign(Context::KEYS, &body).0,
            Signer::FirstContact,
        )
        .unwrap();
        let silent = Dialog::answering(&[]);
        assert!(matches!(
            admit_with(&layout, &f, &keys, Occasion::Launch, &silent),
            Err(Error::NotAuthorized { .. })
        ));
        assert!(silent.shown.borrow().is_empty());
    }

    #[test]
    fn a_grant_from_before_adr_0060_is_not_a_pin() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let app: Identifier = APP.parse().unwrap();
        grants::save(&layout, &app, &crate::fmt::Permissions::default()).unwrap();
        let dialog = Dialog::answering(&[Button::RunOnce]);
        assert_eq!(
            admit_with(
                &layout,
                &file(&dir, APP, HOST, 1),
                &key_set(1),
                Occasion::Launch,
                &dialog
            )
            .unwrap(),
            Admission::RunOnce
        );
        assert_eq!(
            dialog.shown.borrow().len(),
            1,
            "the old record did not stand in for a pin"
        );
        assert!(grants::pairs(&layout).unwrap().is_empty());
    }

    #[test]
    fn edit_distance_counts_single_changes() {
        for (a, b, d) in [
            ("abc", "abc", 0),
            ("abc", "abd", 1),
            ("abc", "ab", 1),
            ("abc", "xabc", 1),
            ("abc", "acb", 2),
            ("", "ab", 2),
        ] {
            assert_eq!(edit_distance(a, b), d, "{a} {b}");
        }
    }

    #[test]
    fn a_pinned_root_whose_file_asks_for_more_is_refused_for_now() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let f = file(&dir, APP, HOST, 1);
        let network = Permissions {
            network: true,
            ..Permissions::default()
        };
        let admit_asking = |requested: &Permissions, dialog: &Dialog| {
            admit(
                &layout,
                &f,
                &key_set(1),
                None,
                Occasion::Launch,
                requested,
                &|r| dialog.ask(r),
            )
        };
        let first = Dialog::answering(&[Button::Install]);
        assert_eq!(
            admit_asking(&network, &first).unwrap(),
            Admission::Installed
        );
        assert_eq!(first.shown.borrow()[0].lines.len(), 1, "the network line");

        let quiet = Dialog::answering(&[]);
        assert_eq!(admit_asking(&network, &quiet).unwrap(), Admission::Pinned);
        let more = Permissions {
            gpu: true,
            ..network.clone()
        };
        assert!(matches!(
            admit_asking(&more, &quiet),
            Err(Error::MoreThanAllowed { .. })
        ));
        assert!(quiet.shown.borrow().is_empty());
    }

    // `odk oma install <file>`.

    use crate::image::fixture as image_fixture;

    /// Two days after the sample manifest was issued: inside its window and its release key's.
    fn soon() -> i64 {
        Timestamp::parse("2026-10-29T00:00:00Z")
            .unwrap()
            .unix_seconds()
    }

    /// The tests' Bundle as a file under the root of seed 1, its `manifest.toml` changed by
    /// `edit_manifest` and its signed manifest by `edit_signed`.
    fn bundle_file(
        dir: &TempDir,
        edit_manifest: impl FnOnce(String) -> String,
        edit_signed: impl FnOnce(&mut crate::trust::SignedManifest),
    ) -> PathBuf {
        let text = edit_manifest(image_fixture::MANIFEST.to_owned());
        let tree =
            image_fixture::notes_with("manifest.toml", Some(image_fixture::file(text, 0o644)));
        let image = image_fixture::build(&tree).image;
        let mut manifest = sample();
        manifest.root_key_id = root(1).public_key().key_id();
        edit_signed(&mut manifest);
        let bytes = crate::trust::stamp(&image[1024..], manifest, &release_key()).unwrap();
        let path = dir.path().join("Notes.oma");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn with_network(text: String) -> String {
        text.replace("[permissions]\n", "[permissions]\nnetwork = true\n")
    }

    /// A publisher that serves `set`, or fails with `failure`, and counts the fetches.
    struct Site {
        served: Result<fetch::KeySet, fetch::Failure>,
        fetches: std::cell::Cell<usize>,
    }

    impl Site {
        fn serving(set: fetch::KeySet) -> Self {
            Self {
                served: Ok(set),
                fetches: Default::default(),
            }
        }

        fn failing(failure: fetch::Failure) -> Self {
            Self {
                served: Err(failure),
                fetches: Default::default(),
            }
        }

        fn fetch(&self, publisher: &Publisher) -> Result<fetch::KeySet, fetch::Failure> {
            assert_eq!(
                publisher.as_str(),
                HOST,
                "only the signed publisher is asked"
            );
            self.fetches.set(self.fetches.get() + 1);
            self.served.clone()
        }
    }

    fn install_with(
        layout: &Layout,
        path: &Path,
        accept_permissions: bool,
        now: i64,
        dialog: &Dialog,
        publisher: &Site,
    ) -> Result<PathBuf, Error> {
        install_file(
            layout,
            path,
            accept_permissions,
            now,
            &|r| dialog.ask(r),
            &|p| publisher.fetch(p),
        )
    }

    fn pair_of(layout: &Layout) -> Option<grants::Pair> {
        grants::load_pair(
            layout,
            &root(1).public_key().key_id(),
            &APP.parse().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn a_file_installs_at_first_contact_with_its_permissions_asked_and_its_mark_written() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let path = bundle_file(&dir, with_network, |_| {});
        let dialog = Dialog::answering(&[Button::Install]);
        let publisher = Site::serving(key_set_bytes(1, &release_key()));

        let target = install_with(&layout, &path, false, soon(), &dialog, &publisher).unwrap();
        assert_eq!(target, layout.applications().join("Notes.oma"));
        assert_eq!(
            std::fs::read(&target).unwrap(),
            std::fs::read(&path).unwrap()
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o644,
            "a Bundle file is inert"
        );
        assert_eq!(publisher.fetches.get(), 1);

        let shown = dialog.shown.borrow();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].buttons, ["install", "cancel"]);
        assert_eq!(
            shown[0].name, APP,
            "the signed app id, not the image's name"
        );
        assert_eq!(
            shown[0].lines,
            crate::fmt::permission_lines(&Permissions {
                network: true,
                ..Permissions::default()
            })
        );
        assert!(!shown[0].lines.is_empty());

        let pair = pair_of(&layout).unwrap();
        assert!(!pair.denied);
        assert!(pair.permissions.unwrap().network);
        let mark = pair.mark.unwrap();
        assert_eq!(mark.version, 7);

        // The same file again does nothing, asks nothing and fetches nothing.
        let again = Dialog::answering(&[]);
        assert_eq!(
            install_with(&layout, &path, false, soon(), &again, &publisher).unwrap(),
            target
        );
        assert_eq!(publisher.fetches.get(), 1);
        // A file in ~/Applications is not a directory Bundle, and listing them skips it.
        assert!(crate::install::installed(&layout).is_empty());
    }

    #[test]
    fn cancel_installs_nothing_and_remembers_the_denial() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let path = bundle_file(&dir, |t| t, |_| {});
        let dialog = Dialog::answering(&[Button::Cancel]);
        let publisher = Site::serving(key_set_bytes(1, &release_key()));
        assert!(matches!(
            install_with(&layout, &path, false, soon(), &dialog, &publisher),
            Err(Error::Cancelled)
        ));
        assert!(!layout.applications().join("Notes.oma").exists());
        assert!(pair_of(&layout).unwrap().denied);
    }

    #[test]
    fn accept_permissions_never_pins_a_new_root() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let path = bundle_file(&dir, |t| t, |_| {});
        let dialog = Dialog::answering(&[]);
        let publisher = Site::serving(key_set_bytes(1, &release_key()));
        assert!(matches!(
            install_with(&layout, &path, true, soon(), &dialog, &publisher),
            Err(Error::NewRootNotAccepted { .. })
        ));
        assert!(pair_of(&layout).is_none());
        assert!(!layout.applications().join("Notes.oma").exists());
    }

    #[test]
    fn what_is_known_before_the_dialog_refuses_before_it() {
        let check = |what: &str,
                     layout: &Layout,
                     path: &Path,
                     now: i64,
                     publisher: &Site,
                     refused: &dyn Fn(&Error) -> bool| {
            let dialog = Dialog::answering(&[]);
            let e = install_with(layout, path, false, now, &dialog, publisher).unwrap_err();
            assert!(refused(&e), "{what}: {e}");
            assert!(dialog.shown.borrow().is_empty(), "{what}: nothing asked");
            assert!(pair_of(layout).is_none(), "{what}: nothing pinned");
        };
        let serving = || Site::serving(key_set_bytes(1, &release_key()));

        // A changed byte: the envelope, and nothing fetched.
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let path = bundle_file(&dir, |t| t, |_| {});
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(&path, bytes).unwrap();
        let p = serving();
        check("tampered", &layout, &path, soon(), &p, &|e| {
            matches!(e, Error::Trust(_))
        });
        assert_eq!(p.fetches.get(), 0);

        // An image whose manifest names another publisher.
        let path = bundle_file(
            &dir,
            |t| t.replace("apps.csfh.dev", "other.csfh.dev"),
            |_| {},
        );
        let p = serving();
        check("image", &layout, &path, soon(), &p, &|e| {
            matches!(e, Error::Image(_))
        });
        assert_eq!(p.fetches.get(), 0);

        // Past its signature window (well past: the boundary is question 7's).
        let path = bundle_file(&dir, |t| t, |_| {});
        let later = Timestamp::parse("2028-01-01T00:00:00Z")
            .unwrap()
            .unix_seconds();
        let p = serving();
        check("expired", &layout, &path, later, &p, &|e| {
            matches!(e, Error::Expired { .. })
        });
        assert_eq!(p.fetches.get(), 0);

        // Its name taken by a directory Bundle, which is left as it is.
        let atmos = layout.applications().join("Notes.oma");
        std::fs::create_dir_all(atmos.join("bin")).unwrap();
        let p = serving();
        check("name taken", &layout, &path, soon(), &p, &|e| {
            matches!(e, Error::Install(install::Error::Exists(_)))
        });
        assert!(atmos.join("bin").is_dir());
        assert_eq!(p.fetches.get(), 0);
        std::fs::remove_dir_all(&atmos).unwrap();

        // The release key past its `not_after`, though the manifest is inside its window.
        let key_expired = Timestamp::parse("2027-06-01T00:00:00Z")
            .unwrap()
            .unix_seconds();
        check(
            "release key expired",
            &layout,
            &path,
            key_expired,
            &serving(),
            &|e| matches!(e, Error::ReleaseKeyExpired { .. }),
        );

        // A key set that does not name the file's release key.
        let other = Site::serving(key_set_bytes(
            1,
            &SigningKey::from_seed(&[0x43; 32]).unwrap(),
        ));
        check("not authorized", &layout, &path, soon(), &other, &|e| {
            matches!(e, Error::NotAuthorized { .. })
        });

        // Another root's key set.
        let other = Site::serving(key_set_bytes(2, &release_key()));
        check("other root", &layout, &path, soon(), &other, &|e| {
            matches!(e, Error::OtherRoot { .. })
        });

        // A host that does not resolve, and one that cannot be reached: two lines.
        for (failure, said) in [
            (
                fetch::Failure::HostNotFound { host: HOST.into() },
                "does not resolve",
            ),
            (
                fetch::Failure::Unreachable {
                    host: HOST.into(),
                    reason: "timed out".into(),
                },
                "could not be reached",
            ),
        ] {
            let p = Site::failing(failure);
            check(said, &layout, &path, soon(), &p, &|e| {
                let text = e.to_string();
                matches!(e, Error::Fetch(_))
                    && text.contains(said)
                    && text.contains("nothing was installed")
            });
        }
        assert!(!layout.applications().join("Notes.oma").exists());
    }

    #[test]
    fn only_the_installed_file_runs_under_its_pin() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let path = bundle_file(&dir, |t| t, |_| {});
        let host = HostEnv {
            uid: 1000,
            gid: 1000,
            runtime_dir: None,
            wayland_display: None,
        };
        // Not installed at all.
        assert!(matches!(
            run_file(&layout, &host, &path, true, soon()),
            Err(Error::NotInstalledFile { .. })
        ));
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let installed = install_with(
            &layout,
            &path,
            false,
            soon(),
            &Dialog::answering(&[Button::Install]),
            &site,
        )
        .unwrap();
        // Another version under the same pin is not the installed one.
        let newer = bundle_file(&dir, |t| t, |m| m.version = 8);
        assert!(matches!(
            run_file(&layout, &host, &newer, true, soon()),
            Err(Error::NotInstalledFile { .. })
        ));
        // The installed one gets as far as the launch, which needs a runtime directory.
        assert!(!matches!(
            run_file(&layout, &host, &installed, true, soon()),
            Err(Error::NotInstalledFile { .. })
        ));
    }

    #[test]
    fn a_newer_file_under_the_pin_upgrades_and_keeps_the_previous_one() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let v7_bytes = std::fs::read(&v7).unwrap();
        let first = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();

        // Asking for nothing new, an upgrade asks nothing.
        let v8 = bundle_file(&dir, |t| t, |m| m.version = 8);
        let quiet = Dialog::answering(&[]);
        assert_eq!(
            install_with(&layout, &v8, false, soon(), &quiet, &site).unwrap(),
            target
        );
        assert_eq!(std::fs::read(&target).unwrap(), std::fs::read(&v8).unwrap());
        let root_id = root(1).public_key().key_id();
        let app: Identifier = APP.parse().unwrap();
        assert_eq!(
            std::fs::read(layout.pair_previous_file(&root_id, &app)).unwrap(),
            v7_bytes
        );
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 8);
        assert_eq!(grants::keys_version(&layout, &root_id).unwrap(), Some(1));
        assert_eq!(site.fetches.get(), 2);

        // Lower, or the same version with other contents: refused, and nothing fetched.
        let lower = bundle_file(&dir, |t| t, |m| m.version = 7);
        assert!(matches!(
            install_with(&layout, &lower, false, soon(), &quiet, &site),
            Err(Error::Version { .. })
        ));
        let other = bundle_file(
            &dir,
            |t| t.replace("version = \"1.0.0\"", "version = \"1.0.1\""),
            |m| m.version = 8,
        );
        assert!(matches!(
            install_with(&layout, &other, false, soon(), &quiet, &site),
            Err(Error::Version { .. })
        ));
        assert_eq!(site.fetches.get(), 2);
        assert!(quiet.shown.borrow().is_empty());
    }

    #[test]
    fn an_upgrade_that_asks_for_more_asks_install_or_cancel_and_cancel_denies_nothing() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let v8 = bundle_file(&dir, with_network, |m| m.version = 8);

        let cancel = Dialog::answering(&[Button::Cancel]);
        assert!(matches!(
            install_with(&layout, &v8, false, soon(), &cancel, &site),
            Err(Error::Cancelled)
        ));
        let shown = cancel.shown.borrow();
        assert_eq!(shown[0].buttons, ["install", "cancel"]);
        assert_eq!(
            shown[0].lines.len(),
            1,
            "only the network line, which is new"
        );
        let pair = pair_of(&layout).unwrap();
        assert!(!pair.denied, "cancelling more is not a denial of the pin");
        assert_eq!(pair.mark.unwrap().version, 7);

        let install = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v8, false, soon(), &install, &site).unwrap();
        let pair = pair_of(&layout).unwrap();
        assert!(pair.permissions.unwrap().network);
        assert_eq!(pair.mark.unwrap().version, 8);
    }

    #[test]
    fn an_older_key_set_than_the_last_accepted_is_refused() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let site = Site::serving(key_set_version(1, &release_key(), 5));
        let first = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let v8 = bundle_file(&dir, |t| t, |m| m.version = 8);
        let older = Site::serving(key_set_version(1, &release_key(), 4));
        assert!(matches!(
            install_with(&layout, &v8, false, soon(), &Dialog::answering(&[]), &older),
            Err(Error::KeySetLower {
                stored: 5,
                fetched: 4
            })
        ));
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 7);
    }

    #[test]
    fn a_second_app_under_a_pinned_key_is_asked_about_and_named_beside_the_first() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let notes = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        install_with(&layout, &notes, false, soon(), &first, &site).unwrap();
        let other = bundle_file(
            &dir,
            |t| {
                t.replace("dev.csfh.notes", "dev.csfh.other")
                    .replace("\"Notes\"", "\"Other\"")
            },
            |m| m.app_id = "dev.csfh.other".parse().unwrap(),
        );
        let dialog = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &other, false, soon(), &dialog, &site).unwrap();
        assert_eq!(target, layout.applications().join("Other.oma"));
        let shown = dialog.shown.borrow();
        assert!(
            shown[0]
                .notes
                .iter()
                .any(|n| n.contains("already pinned for dev.csfh.notes")),
            "{:?}",
            shown[0].notes
        );
    }
}
