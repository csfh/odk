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
use crate::{desktop, dev, dev_key, fetch, grants, image, install, userdirs};

/// Shows the dialog. The real one is [`consent::ask`].
pub type Ask<'a> = &'a dyn Fn(&Request) -> Result<Button, consent::Error>;

/// Runs `systemctl` with these arguments. The real one is [`systemctl`].
pub type Systemctl<'a> = &'a dyn Fn(&[&str]) -> Result<(), String>;

/// `systemctl` itself; its standard error is the reason when it fails.
pub fn systemctl(args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new("systemctl")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

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
    #[error(
        "the key set kept for this publisher is version {found}, not the version {kept} accepted last; it is not used"
    )]
    KeptKeySet { kept: u64, found: u64 },
    #[error(
        "this is a newer {app} than the installed one; install it with `odk oma install` (opening a newer version is not built yet, issue #14)"
    )]
    UpgradeByOpening { app: Identifier },
    #[error("{app} is not installed as a Bundle file in ~/Applications")]
    NoFile { app: Identifier },
    #[error("{app} has no previous version to go back to")]
    NoPrevious { app: Identifier },
    #[error("{app} is still installed at {}; remove it first, or purging would only make it ask again", .path.display())]
    StillInstalled { app: Identifier, path: PathBuf },
    #[error("there is nothing to purge for {app}")]
    NothingToPurge { app: Identifier },
    #[error("systemctl: {0}")]
    Systemctl(String),
    #[error(transparent)]
    DevKey(#[from] dev_key::Error),
    #[error(
        "{app} is installed at {} under another publisher key, and one copy of an application is installed at a time (ADR 0085)", .path.display()
    )]
    InstalledUnderOtherRoot { app: Identifier, path: PathBuf },
    #[error(
        "the data of the directory Bundle being replaced, {}, would move to {}, which exists; nothing is merged, so move one of them away first", .from.display(), .to.display()
    )]
    DevData { from: PathBuf, to: PathBuf },
    #[error("no key set is kept for this computer's dev key; `odk oma dev` makes one")]
    NoDevKeySet,
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
    fetch: Fetch,
) -> Result<i32, Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    // A Bundle file: the installed one for an Identifier with no installed directory Bundle, or
    // a path, which is opened.
    match target.parse::<Identifier>() {
        Ok(id) if install::find(layout, &id).is_none() => {
            if let Some(file) = installed_file(layout, &id)? {
                return run_file(layout, host, &file, wait, now, ask);
            }
        }
        Ok(_) => {}
        Err(_) if Path::new(target).is_file() => {
            return open_file(layout, host, Path::new(target), wait, now, ask, fetch);
        }
        Err(_) => {}
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
    if !options.run_once
        && install::find(layout, &bundle.manifest().identifier)
            .is_some_and(|installed| installed.root() == bundle.root())
    {
        list_directory(layout, bundle.root());
    }
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
    let installed = install::install(layout, source, accept)?;
    list_directory(layout, &installed);
    Ok(installed)
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
    /// The dev key was made by this run: its root's key id (ADR 0085).
    pub made_key: Option<KeyId>,
}

/// Removes the directory a Bundle was assembled in, whichever way `dev` ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `odk oma dev` up to running (ADR 0051, ADR 0085): builds the project in `dir`, signs its
/// Bundle file with this computer's dev key, asks for what the pair (dev root, app id) lacks,
/// stops the daemon of the copy it replaces and installs over it. `now` is the time in Unix
/// seconds. A build that fails changes nothing, and a Cancel changes nothing but the dev key,
/// once made. Running the installed copy is [`run`]'s, by Identifier.
///
/// The dev root is pinned without the first-install dialog, and its key set is made here and
/// kept, never fetched. An installed directory Bundle of the Identifier, the old output, is
/// replaced: its data moves to the pair's, and its consent record is deleted. A copy installed
/// under another root is refused.
pub fn dev(
    layout: &Layout,
    dir: &Path,
    runtime_dir: Option<&Path>,
    now: i64,
    ask: Ask,
    stopper: &dyn Stopper,
) -> Result<Prepared, Error> {
    let project = dev::Project::load(dir)?;
    project.build()?;
    let app = project.manifest()?.identifier;

    let (key, made) = dev_key::DevKey::load_or_create(layout)?;
    let root = key.root.public_key();
    let root_id = root.key_id();
    let existing = grants::load_pair(layout, &root_id, &app)?;
    // The time, or one more than the mark when that is higher: two builds in one second, or a
    // clock that went back, still install.
    let marked = existing.as_ref().and_then(|p| p.mark).map(|m| m.version);
    let version = newer(now, marked);

    let scratch = Scratch(layout.dev_dir().join(std::process::id().to_string()));
    let written = project.write_file(&scratch.0, &key.signing(version, now)?)?;
    let sealed = Sealed::memfd(&written)?;
    let checked = trust::check(&sealed)?;
    let image = Image::read(&checked)?;
    let file = checked.signed();
    let manifest = image.manifest();
    let target = layout
        .applications()
        .join(format!("{}.oma", manifest.name.replace('/', "-")));

    // Everything that refuses does so before anyone is asked.
    one_copy(layout, &app, &root_id)?;
    let installed = installed_file(layout, &app)?;
    let directory = install::find(layout, &app).map(|b| b.root().to_owned());
    if target.symlink_metadata().is_ok()
        && installed.as_ref() != Some(&target)
        && directory.as_ref() != Some(&target)
    {
        return Err(install::Error::Exists(target).into());
    }
    let (old_data, new_data) = (layout.data_dir(&app), layout.pair_data_dir(&root_id, &app));
    let move_data = directory.is_some() && old_data.symlink_metadata().is_ok();
    if move_data && new_data.symlink_metadata().is_ok() {
        return Err(Error::DevData {
            from: old_data,
            to: new_data,
        });
    }

    let requested = &manifest.permissions;
    let granted = existing
        .as_ref()
        .and_then(|p| p.permissions.clone())
        .unwrap_or_default();
    let missing = grants::missing(&granted, requested);
    if !grants::is_empty(&missing) {
        let identity = Identity {
            app_id: &app,
            version,
            root: &root,
            publisher: &file.manifest().publisher,
            fetched: None,
        };
        let named_dirs = userdirs::resolve(layout.home(), &layout.user_dirs_file());
        let request = Request::dev(&identity, &missing, &named_dirs);
        if ask(&request).map_err(Error::Dialog)? != Button::Install {
            return Err(Error::Cancelled);
        }
    }

    // The dev root's key set, newer than the one kept, so that it never runs out while the
    // application is being built.
    let set_version = newer(now, grants::keys_version(layout, &root_id)?);
    let set = key.key_set(set_version, now)?;
    grants::save_key_set(layout, &root_id, &set.body, &set.signature, set_version)?;
    let unfetched: Fetch = &|publisher| {
        Err(fetch::Failure::Unreachable {
            host: publisher.as_str().to_owned(),
            reason: "the dev root's key set is never fetched".to_owned(),
        })
    };
    let authorized = authorized_keys(layout, file, Some(&root), now, unfetched)?;
    grants::save_pair(
        layout,
        &app,
        &grants::Pair {
            root,
            publisher: file.manifest().publisher.clone(),
            permissions: Some(requested.clone()),
            mark: existing.as_ref().and_then(|p| p.mark),
            installed: existing.as_ref().and_then(|p| p.installed),
            denied: false,
        },
    )?;

    let declared = manifest.permissions.daemon.as_ref();
    if declared.is_some() {
        stopper.stop(&app).map_err(Error::StopDaemon)?;
    }
    // The directory Bundle goes aside, under a name listing skips, and its data moves to the
    // pair's. Both come back if the file cannot be placed, so an error leaves what was installed.
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| install::Error::Io { path, source }
    };
    let aside = match &directory {
        Some(path) => {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            let aside = path.with_file_name(format!(
                ".{}.replaced-{}",
                name.as_deref().unwrap_or("bundle"),
                std::process::id()
            ));
            fs::rename(path, &aside).map_err(io(path))?;
            Some((path, aside))
        }
        None => None,
    };
    let put_back = |data_moved: bool| {
        if data_moved {
            let _ = fs::rename(&new_data, &old_data);
        }
        if let Some((path, aside)) = &aside {
            let _ = fs::rename(aside, path);
        }
    };
    if move_data {
        let parent = new_data.parent().expect("a pair's directory");
        let moved = fs::create_dir_all(parent)
            .map_err(io(parent))
            .and_then(|()| fs::rename(&old_data, &new_data).map_err(io(&old_data)));
        if let Err(e) = moved {
            put_back(false);
            return Err(e.into());
        }
    }
    if let Err(e) = commit(
        layout,
        file,
        installed.as_deref(),
        &sealed,
        &target,
        &authorized,
    ) {
        put_back(move_data);
        return Err(e);
    }
    // The file is in place: what is left is tidying, which does not undo it.
    if let Some((_, aside)) = &aside {
        let _ = fs::remove_file(layout.grants_file(&app));
        let _ = fs::remove_dir_all(aside);
    }
    list(layout, &app, manifest, None);

    let foreign_daemon = declared
        .zip(runtime_dir)
        .filter(|(daemon, runtime)| daemon::serving(runtime, daemon))
        .map(|(daemon, runtime)| runtime.join(&daemon.socket));
    Ok(Prepared {
        identifier: app,
        installed: target,
        replaced: installed.is_some() || directory.is_some(),
        foreign_daemon,
        made_key: made.then_some(root_id),
    })
}

/// `now` as a version, or one more than `kept` when that is higher.
fn newer(now: i64, kept: Option<u64>) -> u64 {
    let now = u64::try_from(now).unwrap_or(0);
    kept.map_or(now, |kept| now.max(kept + 1))
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
            installed: None,
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
                installed: existing.as_ref().and_then(|p| p.installed),
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
    one_copy(layout, app, root_id)?;
    let pinned = grants::pairs(layout)?
        .into_iter()
        .filter(|(_, pair)| pair.root.key_id() == *root_id && !pair.denied)
        .collect::<Vec<_>>();
    let pair = pinned
        .iter()
        .find(|(other, _)| other == app)
        .map(|(_, p)| p);
    let installed = match pair {
        Some(_) => installed_file(layout, app)?,
        None => None,
    };

    // The high-water rule (section 4): the same file again is a no-op, and a lower version, or
    // the same version with another payload, is refused.
    let current = pair.and_then(|p| p.installed.or(p.mark));
    match Verdict::of(
        pair.and_then(|p| p.mark.as_ref()),
        signed.version,
        &signed.payload_sha256,
    ) {
        // The marked file again: nothing to do, unless a rollback put another in its place,
        // which this one replaces again. It was authorized when it was first installed.
        Verdict::Same if current == Some(mark) && installed.is_some() => {
            return Ok(installed.expect("checked"));
        }
        Verdict::Same => {
            replace_installed(layout, root_id, app, installed.as_deref(), &sealed, &target)?;
            let mut pair = pair.expect("a mark is a pair's").clone();
            pair.installed = Some(mark);
            grants::save_pair(layout, app, &pair)?;
            return Ok(target);
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

    let pinned_root = pinned.first().map(|(_, pair)| pair.root);
    let authorized = authorized_keys(layout, file, pinned_root.as_ref(), now, fetch)?;

    let occasion = Occasion::Install { accept_permissions };
    let requested = &image.manifest().permissions;
    match admit(
        layout,
        file,
        &authorized.keys,
        None,
        occasion,
        requested,
        ask,
    )? {
        Admission::Installed | Admission::Pinned => {}
        Admission::RunOnce => unreachable!("install offers Install and Cancel only"),
    }

    commit(
        layout,
        file,
        installed.as_deref(),
        &sealed,
        &target,
        &authorized,
    )?;
    list(layout, app, image.manifest(), None);
    Ok(target)
}

/// A key set that authorizes a file, and the bytes it was fetched as, which [`commit`] keeps. A
/// set read back from what was kept has no bytes to keep again.
struct Authorized {
    keys: VerifiedKeys,
    fetched: Option<fetch::KeySet>,
}

/// The publisher's key set for `file`, fetched and checked before anyone is asked (ADR 0060,
/// section 3): verified under `pinned_root` when the root is pinned for any app id and on first
/// contact otherwise, never older than the last accepted for the root, and naming the file's
/// release key inside its `not_after`. When the fetch fails under a pinned root, the set kept
/// from the last one is used, verified again under the pin and only at the version kept: a file
/// in hand is an upgrade while the host cannot be reached (section 2, ADR 0082).
fn authorized_keys(
    layout: &Layout,
    file: &SelfSigned,
    pinned_root: Option<&trust::PublicKey>,
    now: i64,
    fetch: Fetch,
) -> Result<Authorized, Error> {
    let signed = file.manifest();
    let app = &signed.app_id;
    let root_id = &signed.root_key_id;
    // This computer's dev root: its set is the one `odk oma dev` keeps. The publisher's host
    // serves its own root's set, which would refuse every dev build (ADR 0085).
    let dev_root = dev_key::root(layout).filter(|root| root.key_id() == *root_id);
    let attempt = match &dev_root {
        Some(_) => None,
        None => Some(fetch(&signed.publisher)),
    };
    let (keys, fetched) = match (attempt, pinned_root) {
        (None, _) => {
            let root = dev_root.as_ref().expect("no fetch is the dev root's");
            let keys = kept_keys(layout, root_id, root)?.ok_or(Error::NoDevKeySet)?;
            (keys, None)
        }
        (Some(Ok(fetched)), _) => {
            let signer = match pinned_root {
                Some(root) => Signer::Pinned(root),
                None => Signer::FirstContact,
            };
            let keys = trust::keys::verify(&fetched.body, &fetched.signature, signer)?;
            (keys, Some(fetched))
        }
        (Some(Err(failure)), Some(root)) => {
            let Some(keys) = kept_keys(layout, root_id, root)? else {
                return Err(Error::Fetch(failure));
            };
            if let fetch::Failure::HostNotFound { host } = &failure {
                eprintln!(
                    "odk oma: `{host}` does not resolve; the name may have been lost. {app} keeps running. Updates that have to be fetched from this host are held."
                );
            }
            (keys, None)
        }
        (Some(Err(failure)), None) => return Err(Error::Fetch(failure)),
    };
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
    Ok(Authorized { keys, fetched })
}

/// The key set kept for `root_id`, verified again under `root` and only at the version kept
/// (ADR 0082). `None` when none is kept.
fn kept_keys(
    layout: &Layout,
    root_id: &KeyId,
    root: &trust::PublicKey,
) -> Result<Option<VerifiedKeys>, Error> {
    let Some(kept) = grants::stored_key_set(layout, root_id)? else {
        return Ok(None);
    };
    let keys = trust::keys::verify(&kept.body, &kept.signature, Signer::Pinned(root))?;
    let version = grants::keys_version(layout, root_id)?;
    if version != Some(keys.set().version) {
        return Err(Error::KeptKeySet {
            kept: version.unwrap_or_default(),
            found: keys.set().version,
        });
    }
    Ok(Some(keys))
}

/// Keeps the key set that authorized a file just installed, when it was fetched (ADR 0082).
fn keep_key_set(layout: &Layout, root: &KeyId, authorized: &Authorized) -> Result<(), Error> {
    if let Some(set) = &authorized.fetched {
        let version = authorized.keys.set().version;
        grants::save_key_set(layout, root, &set.body, &set.signature, version)?;
    }
    Ok(())
}

/// Puts an admitted file in place of `installed`, the pair's file now if it has one, and records
/// it: the mark, what is installed, and the key set that authorized it.
fn commit(
    layout: &Layout,
    file: &SelfSigned,
    installed: Option<&Path>,
    sealed: &Sealed,
    target: &Path,
    authorized: &Authorized,
) -> Result<(), Error> {
    let signed = file.manifest();
    let (root_id, app) = (&signed.root_key_id, &signed.app_id);
    let mark = Mark {
        version: signed.version,
        payload_sha256: signed.payload_sha256,
    };
    replace_installed(layout, root_id, app, installed, sealed, target)?;
    let mut pair = grants::load_pair(layout, root_id, app)?.expect("the pair is pinned");
    pair.mark = Some(mark);
    pair.installed = Some(mark);
    grants::save_pair(layout, app, &pair)?;
    keep_key_set(layout, root_id, authorized)
}

/// `odk oma run` of a Bundle file that is not the installed one: opening it (ADR 0060, section 7;
/// ADR 0018). Under a root not pinned for its app id it is checked as an install would be, and
/// then asked about with the first-install buttons, Install, Run once and Cancel. Install installs
/// it and runs the installed copy. Run once runs it with a private data directory and stores
/// nothing, no pin, no key set and no denial. Cancel stores a denial, and a later launch does not
/// ask again. The installed version itself runs as installed. A newer version of an installed
/// application is not run by opening it: the upgrade dialog is not built (issue #14).
pub fn open_file(
    layout: &Layout,
    host: &HostEnv,
    path: &Path,
    wait: bool,
    now: i64,
    ask: Ask,
    fetch: Fetch,
) -> Result<i32, Error> {
    let sealed = Sealed::memfd(path)?;
    let checked = trust::check(&sealed)?;
    let image = Image::read(&checked)?;
    let file = checked.signed();
    let signed = file.manifest();
    let (app, root_id) = (&signed.app_id, &signed.root_key_id);
    let pinned = grants::pairs(layout)?
        .into_iter()
        .filter(|(_, pair)| pair.root.key_id() == *root_id && !pair.denied)
        .collect::<Vec<_>>();
    if let Some((_, pair)) = pinned.iter().find(|(other, _)| other == app) {
        return match Verdict::of(pair.mark.as_ref(), signed.version, &signed.payload_sha256) {
            Verdict::Same => run_file(layout, host, path, wait, now, ask),
            Verdict::Higher => Err(Error::UpgradeByOpening { app: app.clone() }),
            Verdict::Lower | Verdict::SameVersionOtherPayload => Err(Error::Version {
                app: app.clone(),
                version: signed.version,
                marked: pair.mark.map_or(0, |m| m.version),
            }),
            Verdict::First => Err(Error::NotInstalledFile { app: app.clone() }),
        };
    }

    // A Cancel is remembered: a launch does not ask again, or fetch anything.
    if grants::load_pair(layout, root_id, app)?.is_some_and(|pair| pair.denied) {
        return Err(Error::Denied {
            app: app.clone(),
            root: *root_id,
        });
    }
    if signed.expires_at.unix_seconds() < now {
        return Err(Error::Expired {
            app: app.clone(),
            at: signed.expires_at.clone(),
        });
    }
    one_copy(layout, app, root_id)?;
    let target = layout
        .applications()
        .join(format!("{}.oma", image.manifest().name.replace('/', "-")));
    if target.symlink_metadata().is_ok() {
        return Err(install::Error::Exists(target).into());
    }
    let pinned_root = pinned.first().map(|(_, pair)| pair.root);
    let authorized = authorized_keys(layout, file, pinned_root.as_ref(), now, fetch)?;
    let requested = &image.manifest().permissions;
    match admit(
        layout,
        file,
        &authorized.keys,
        None,
        Occasion::Launch,
        requested,
        ask,
    )? {
        Admission::Installed => {
            commit(layout, file, None, &sealed, &target, &authorized)?;
            run_file(layout, host, &target, wait, now, ask)
        }
        Admission::RunOnce => Ok(launch::run_file(
            layout, &image, root_id, requested, host, wait, true,
        )?),
        Admission::Pinned => unreachable!("the root is not pinned for this app id"),
    }
}

/// Places `sealed` at `target`, keeping the file it replaces, `installed`, as the pair's previous
/// one (ADR 0011), and putting that back if the new one cannot be placed.
fn replace_installed(
    layout: &Layout,
    root: &KeyId,
    app: &Identifier,
    installed: Option<&Path>,
    sealed: &Sealed,
    target: &Path,
) -> Result<(), Error> {
    let previous = layout.pair_previous_file(root, app);
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| install::Error::Io { path, source }
    };
    if let Some(old) = installed {
        let dir = previous.parent().expect("a pair's directory");
        fs::create_dir_all(dir).map_err(io(dir))?;
        fs::rename(old, &previous).map_err(io(old))?;
    }
    if let Err(e) = install::place_file(sealed, target) {
        if let Some(old) = installed {
            let _ = fs::rename(&previous, old);
        }
        return Err(e.into());
    }
    Ok(())
}

/// `odk oma rollback <app id>` (ADR 0011): the installed Bundle file and the previous one change
/// places. The previous one must verify, under the same root and app id, before it is put back.
/// The high-water mark stays (ADR 0060, section 4): only the record of what is installed changes,
/// so `run` runs the file now in `~/Applications`. Returns where it is.
pub fn rollback(layout: &Layout, app: &Identifier) -> Result<PathBuf, Error> {
    let current = installed_file(layout, app)?.ok_or_else(|| Error::NoFile { app: app.clone() })?;
    let root = {
        let sealed = Sealed::memfd(&current)?;
        trust::check(&sealed)?.signed().manifest().root_key_id
    };
    let mut pair = grants::load_pair(layout, &root, app)?
        .filter(|pair| !pair.denied)
        .ok_or_else(|| Error::NotInstalledFile { app: app.clone() })?;
    let previous = layout.pair_previous_file(&root, app);
    if !previous.is_file() {
        return Err(Error::NoPrevious { app: app.clone() });
    }
    let sealed = Sealed::memfd(&previous)?;
    let checked = trust::check(&sealed)?;
    Image::read(&checked)?;
    let signed = checked.signed().manifest();
    if signed.app_id != *app || signed.root_key_id != root {
        return Err(Error::NoPrevious { app: app.clone() });
    }
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| install::Error::Io { path, source }
    };
    let swap = previous.with_extension("oma.swap");
    fs::rename(&current, &swap).map_err(io(&current))?;
    fs::rename(&previous, &current).map_err(io(&previous))?;
    fs::rename(&swap, &previous).map_err(io(&swap))?;
    pair.installed = Some(Mark {
        version: signed.version,
        payload_sha256: signed.payload_sha256,
    });
    grants::save_pair(layout, app, &pair)?;
    Ok(current)
}

/// `odk oma purge <app id>`: every pair of that app id loses its data, previous file, inbox, held
/// file and record, which holds the pin, the consent and the high-water mark. It refuses while
/// the application is in `~/Applications`, whose next launch would only ask again. Returns the
/// roots it purged.
pub fn purge(layout: &Layout, app: &Identifier) -> Result<Vec<KeyId>, Error> {
    if let Some(path) = install::find_file(layout, app)
        .or_else(|| install::find(layout, app).map(|b| b.root().to_owned()))
    {
        return Err(Error::StillInstalled {
            app: app.clone(),
            path,
        });
    }
    let roots: Vec<KeyId> = grants::pairs(layout)?
        .into_iter()
        .filter(|(other, _)| other == app)
        .map(|(_, pair)| pair.root.key_id())
        .collect();
    if roots.is_empty() {
        return Err(Error::NothingToPurge { app: app.clone() });
    }
    let gone = |result: std::io::Result<()>, path: &Path| match result {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(install::Error::Io {
            path: path.to_owned(),
            source: e,
        }),
        _ => Ok(()),
    };
    for root in &roots {
        let dir = layout.pair_dir(root, app);
        gone(fs::remove_dir_all(&dir), &dir)?;
        let held = layout.held_file(root, app);
        gone(fs::remove_file(&held), &held)?;
        let record = layout.pair_grants_file(root, app);
        gone(fs::remove_file(&record), &record)?;
    }
    Ok(roots)
}

/// The installed Bundle file of `app`: the one in `~/Applications` whose signed version and
/// payload are what its pair records as installed. Other files of the same app id may be there.
pub fn installed_file(layout: &Layout, app: &Identifier) -> Result<Option<PathBuf>, Error> {
    Ok(installed_copy(layout, app)?.map(|(path, _)| path))
}

/// [`installed_file`], with the root it is installed under.
fn installed_copy(layout: &Layout, app: &Identifier) -> Result<Option<(PathBuf, KeyId)>, Error> {
    for (path, signed) in install::bundle_files(layout) {
        let Some(signed) = signed.filter(|s| s.app_id == *app) else {
            continue;
        };
        let Some(pair) = grants::load_pair(layout, &signed.root_key_id, app)? else {
            continue;
        };
        let mark = Mark {
            version: signed.version,
            payload_sha256: signed.payload_sha256,
        };
        if !pair.denied && pair.installed.or(pair.mark) == Some(mark) {
            return Ok(Some((path, signed.root_key_id)));
        }
    }
    Ok(None)
}

/// One installed copy of an application at a time, whatever its root (ADR 0085): a file of
/// `app` under `root` is refused while a copy under another root is installed.
fn one_copy(layout: &Layout, app: &Identifier, root: &KeyId) -> Result<(), Error> {
    match installed_copy(layout, app)? {
        Some((path, other)) if other != *root => Err(Error::InstalledUnderOtherRoot {
            app: app.clone(),
            path,
        }),
        _ => Ok(()),
    }
}

/// What `odk oma inspect` says of a Bundle file (`docs/spec/oma-cli.md`).
#[derive(Debug)]
pub struct Inspected {
    /// The image's Manifest: name, semver, description and the permissions it asks for.
    pub manifest: crate::fmt::Manifest,
    /// The signed manifest: the monotonic version, the publisher and the root's key id.
    pub signed: trust::SignedManifest,
    /// The root, when it is pinned for this app id: its fingerprint is the publisher's identity.
    pub pinned: Option<trust::PublicKey>,
    /// Files of this app id held under other roots, with the root each is signed by.
    pub held: Vec<(KeyId, PathBuf)>,
}

/// `odk oma inspect` of a Bundle file: its envelope and its image are checked as a launch checks
/// them, and nothing is fetched. With no `layout` nothing is looked up, so it says nothing of a
/// pin or of held files.
pub fn inspect_file(layout: Option<&Layout>, path: &Path) -> Result<Inspected, Error> {
    let sealed = Sealed::memfd(path)?;
    let checked = trust::check(&sealed)?;
    let image = Image::read(&checked)?;
    let signed = checked.signed().manifest().clone();
    let (app, root) = (&signed.app_id, &signed.root_key_id);
    let mut pinned = None;
    let mut held = Vec::new();
    if let Some(layout) = layout {
        pinned = grants::load_pair(layout, root, app)?
            .filter(|pair| !pair.denied && pair.root.key_id() == *root)
            .map(|pair| pair.root);
        for entry in fs::read_dir(layout.held_dir())
            .into_iter()
            .flatten()
            .flatten()
        {
            let Some(other) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<KeyId>().ok())
            else {
                continue;
            };
            let file = layout.held_file(&other, app);
            if other != *root && file.is_file() {
                held.push((other, file));
            }
        }
        held.sort();
    }
    Ok(Inspected {
        manifest: image.manifest().clone(),
        signed,
        pinned,
        held,
    })
}

/// What `--register` did with one Bundle file in `~/Applications`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registered {
    /// The installed version, or another copy of it: nothing to do.
    Kept(PathBuf),
    /// A newer version under the pin, applied: where it is installed now.
    Applied(PathBuf),
    /// Signed by a root not pinned for its app id: moved to where held files are kept.
    Held {
        app: Identifier,
        from: PathBuf,
        to: PathBuf,
    },
    /// Deleted, and why.
    Deleted { path: PathBuf, why: String },
    /// Left where it is, and why.
    Waiting { path: PathBuf, why: String },
}

/// `odk oma --register` for Bundle files (ADR 0060, section 7): every `*.oma` file in
/// `~/Applications` is reconciled, and no dialog is opened. A file that does not verify, an
/// expired one, and a version lower than the mark, or the marked version with other contents,
/// are deleted. A valid file under a root not pinned for its app id is held at
/// `held/<root>/<app>.oma`, and not launched. A newer version under the pin, dropped there or
/// staged in its application's inbox, is applied when its key set authorizes it, and takes
/// effect even when it asks for more, which its next launch asks about (ADR 0015). Directory
/// Bundles, and anything that is not a regular `*.oma` file, are not touched. `.desktop` entries
/// and inbox watches are not built yet.
pub fn register(layout: &Layout, now: i64, fetch: Fetch, systemctl: Systemctl) -> Vec<Registered> {
    // The path unit watches `~/Applications`, which has to be there to be watched (ADR 0053).
    let _ = fs::create_dir_all(layout.applications());
    let or_waiting = |path: PathBuf, result: Result<Registered, Error>| {
        result.unwrap_or_else(|e| Registered::Waiting {
            why: e.to_string(),
            path,
        })
    };
    let mut done: Vec<Registered> = install::bundle_files(layout)
        .into_iter()
        .map(|(path, _)| {
            let result = register_one(layout, &path, now, fetch);
            or_waiting(path, result)
        })
        .collect();
    // Each installed application's inbox, for what it staged there (ADR 0015).
    for (app, pair) in grants::pairs(layout)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, pair)| !pair.denied)
    {
        let inbox = layout.pair_inbox_dir(&pair.root.key_id(), &app);
        let Ok(entries) = fs::read_dir(&inbox) else {
            continue;
        };
        let mut staged: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "oma"))
            .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_file()))
            .collect();
        staged.sort();
        for path in staged {
            let result = take_from_inbox(layout, &app, &pair, &path, now, fetch);
            done.push(or_waiting(path, result));
        }
    }
    match installed_apps(layout) {
        Ok(apps) => {
            if let Err(e) = desktop_entries(layout, &apps) {
                eprintln!("odk oma: the .desktop entries were not all written: {e}");
            }
            if let Err(e) = inbox_watches(layout, &apps, systemctl) {
                eprintln!("odk oma: the inbox watches were not all changed: {e}");
            }
        }
        Err(e) => eprintln!("odk oma: the installed applications could not be read: {e}"),
    }
    done
}

/// Every installed Bundle file's app id, root and Manifest, read from its verified image.
fn installed_apps(
    layout: &Layout,
) -> Result<Vec<(Identifier, KeyId, crate::fmt::Manifest)>, Error> {
    let mut apps = Vec::new();
    for (app, pair) in grants::pairs(layout)?
        .into_iter()
        .filter(|(_, pair)| !pair.denied)
    {
        let Some(path) = installed_file(layout, &app)? else {
            continue;
        };
        let sealed = Sealed::memfd(&path)?;
        let checked = trust::check(&sealed)?;
        let manifest = Image::read(&checked)?.manifest().clone();
        apps.push((app, pair.root.key_id(), manifest));
    }
    Ok(apps)
}

/// Enables a watch on the inbox of each installed application that may stage upgrades, the
/// `updates` permission, and disables the ones no longer wanted (ADR 0015, ADR 0053). Only what
/// changed since the last run is asked of `systemctl`, from the record of what it enabled.
fn inbox_watches(
    layout: &Layout,
    apps: &[(Identifier, KeyId, crate::fmt::Manifest)],
    systemctl: Systemctl,
) -> Result<(), Error> {
    use std::collections::BTreeSet;
    let wanted: BTreeSet<String> = apps
        .iter()
        .filter(|(_, _, manifest)| manifest.permissions.updates)
        .map(|(app, root, _)| format!("oma-inbox@{}.path", unit_escape(&format!("{root}/{app}"))))
        .collect();
    let record = layout.inbox_watches_file();
    let had: BTreeSet<String> = fs::read_to_string(&record)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    for (verb, units) in [
        ("enable", wanted.difference(&had).collect::<Vec<_>>()),
        ("disable", had.difference(&wanted).collect::<Vec<_>>()),
    ] {
        if units.is_empty() {
            continue;
        }
        let mut args = vec!["--user", verb, "--now"];
        args.extend(units.iter().map(|u| u.as_str()));
        systemctl(&args).map_err(Error::Systemctl)?;
    }
    if wanted != had {
        let text: String = wanted.iter().map(|u| format!("{u}\n")).collect();
        let io = |source| install::Error::Io {
            path: record.clone(),
            source,
        };
        fs::create_dir_all(record.parent().expect("the state directory")).map_err(io)?;
        fs::write(&record, text).map_err(io)?;
    }
    Ok(())
}

/// `text` escaped as an instance name, as `systemd-escape` does it and `%I` undoes it: `/` is
/// `-`, and any byte but ASCII letters, digits, `:`, `_` and a `.` that is not first is `\xNN`.
fn unit_escape(text: &str) -> String {
    let mut out = String::new();
    for (i, byte) in text.bytes().enumerate() {
        match byte {
            b'/' => out.push('-'),
            b'.' if i > 0 => out.push('.'),
            b if b.is_ascii_alphanumeric() || b == b':' || b == b'_' => out.push(b as char),
            b => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out
}

/// Writes the launcher entry of every installed application, Bundle files and directory Bundles,
/// and removes the entries `odk-oma` wrote for any that is gone (ADR 0014). A Bundle file's icon
/// is in its image, so its entry has none.
fn desktop_entries(
    layout: &Layout,
    apps: &[(Identifier, KeyId, crate::fmt::Manifest)],
) -> Result<(), Error> {
    let odk = desktop::facade();
    let io = |source| install::Error::Io {
        path: layout.desktop_entries(),
        source,
    };
    let mut keep = Vec::new();
    for (app, _, manifest) in apps {
        desktop::write(layout, app, &desktop::text(app, manifest, None, &odk)).map_err(io)?;
        keep.push(app.clone());
    }
    for bundle in install::installed(layout) {
        let manifest = bundle.manifest();
        let icon = manifest.icon.as_ref().map(|icon| bundle.root().join(icon));
        let id = &manifest.identifier;
        let text = desktop::text(id, manifest, icon.as_deref(), &odk);
        desktop::write(layout, id, &text).map_err(io)?;
        keep.push(id.clone());
    }
    desktop::remove_others(layout, &keep).map_err(io)?;
    Ok(())
}

/// Writes the launcher entry of the directory Bundle installed at `root`. A failure is told and
/// does not undo anything: the application is installed either way.
fn list_directory(layout: &Layout, root: &Path) {
    let Ok(bundle) = Bundle::open(root) else {
        return;
    };
    let manifest = bundle.manifest();
    let icon = manifest.icon.as_ref().map(|icon| bundle.root().join(icon));
    list(layout, &manifest.identifier, manifest, icon.as_deref());
}

fn list(layout: &Layout, id: &Identifier, manifest: &crate::fmt::Manifest, icon: Option<&Path>) {
    let text = desktop::text(id, manifest, icon, &desktop::facade());
    if let Err(e) = desktop::write(layout, id, &text) {
        eprintln!("odk oma: the launcher entry of {id} was not written: {e}");
    }
}

fn register_one(layout: &Layout, path: &Path, now: i64, fetch: Fetch) -> Result<Registered, Error> {
    let io = |source| install::Error::Io {
        path: path.to_owned(),
        source,
    };
    let delete = |why: String| -> Result<Registered, Error> {
        fs::remove_file(path).map_err(io)?;
        Ok(Registered::Deleted {
            path: path.to_owned(),
            why,
        })
    };
    let sealed = Sealed::memfd(path)?;
    let checked = match trust::check(&sealed) {
        Ok(checked) => checked,
        Err(trust::Error::Io(e)) => return Err(trust::Error::Io(e).into()),
        Err(e) => return delete(e.to_string()),
    };
    let image = match Image::read(&checked) {
        Ok(image) => image,
        Err(image::Error::Io(e)) => return Err(image::Error::Io(e).into()),
        Err(e) => return delete(e.to_string()),
    };
    let file = checked.signed();
    let signed = file.manifest();
    let (app, root_id) = (&signed.app_id, &signed.root_key_id);
    if signed.expires_at.unix_seconds() < now {
        return delete(format!(
            "its signature expired at {}",
            signed.expires_at.as_str()
        ));
    }
    let Some(pair) = grants::load_pair(layout, root_id, app)?.filter(|pair| !pair.denied) else {
        let held = layout.held_file(root_id, app);
        let _ = fs::remove_file(&held);
        install::place_file(&sealed, &held)?;
        fs::remove_file(path).map_err(io)?;
        return Ok(Registered::Held {
            app: app.clone(),
            from: path.to_owned(),
            to: held,
        });
    };
    let marked = pair.mark.map_or(0, |m| m.version);
    match Verdict::of(pair.mark.as_ref(), signed.version, &signed.payload_sha256) {
        Verdict::Same => return Ok(Registered::Kept(path.to_owned())),
        Verdict::Lower => {
            return delete(format!("version {} is lower than {marked}", signed.version));
        }
        Verdict::SameVersionOtherPayload => {
            return delete(format!(
                "version {} has other contents than the one installed",
                signed.version
            ));
        }
        Verdict::First => {
            return Ok(Registered::Waiting {
                path: path.to_owned(),
                why: "nothing of it is installed yet; install it with `odk oma install`".to_owned(),
            });
        }
        Verdict::Higher => {}
    }
    apply_newer(layout, &checked, &image, &sealed, &pair, path, now, fetch)
}

/// Applies a newer version of an installed application, under its pin, from `source`: a file
/// dropped in `~/Applications` or staged in its inbox. It takes effect even when it asks for
/// more, which its next launch asks about (ADR 0015). A key set that cannot be fetched leaves it
/// where it is, for the next pass.
#[allow(clippy::too_many_arguments)]
fn apply_newer(
    layout: &Layout,
    checked: &trust::Checked<'_>,
    image: &Image<'_>,
    sealed: &Sealed,
    pair: &grants::Pair,
    source: &Path,
    now: i64,
    fetch: Fetch,
) -> Result<Registered, Error> {
    let file = checked.signed();
    let signed = file.manifest();
    let (app, root_id) = (&signed.app_id, &signed.root_key_id);
    let waiting = |why: String| {
        Ok(Registered::Waiting {
            path: source.to_owned(),
            why,
        })
    };
    let authorized = match authorized_keys(layout, file, Some(&pair.root), now, fetch) {
        Ok(authorized) => authorized,
        Err(e) => return waiting(e.to_string()),
    };
    let installed = installed_file(layout, app)?;
    let target = layout
        .applications()
        .join(format!("{}.oma", image.manifest().name.replace('/', "-")));
    if target == source {
        // Copied over the installed file: it is in place, and only the records change.
        let mark = Mark {
            version: signed.version,
            payload_sha256: signed.payload_sha256,
        };
        let mut pair = pair.clone();
        pair.mark = Some(mark);
        pair.installed = Some(mark);
        grants::save_pair(layout, app, &pair)?;
        keep_key_set(layout, root_id, &authorized)?;
        return Ok(Registered::Applied(target));
    }
    if target.symlink_metadata().is_ok() && installed.as_deref() != Some(target.as_path()) {
        return waiting(format!("{} is taken", target.display()));
    }
    commit(
        layout,
        file,
        installed.as_deref(),
        sealed,
        &target,
        &authorized,
    )?;
    fs::remove_file(source).map_err(|e| install::Error::Io {
        path: source.to_owned(),
        source: e,
    })?;
    Ok(Registered::Applied(target))
}

/// A Bundle file an application staged in its own inbox (ADR 0015, ADR 0052): its bytes are
/// sealed at once, so the application cannot change them while they are checked. It is accepted
/// only as a newer version of that application under that root, and deleted if it is not.
fn take_from_inbox(
    layout: &Layout,
    app: &Identifier,
    pair: &grants::Pair,
    path: &Path,
    now: i64,
    fetch: Fetch,
) -> Result<Registered, Error> {
    let delete = |why: String| -> Result<Registered, Error> {
        fs::remove_file(path).map_err(|source| install::Error::Io {
            path: path.to_owned(),
            source,
        })?;
        Ok(Registered::Deleted {
            path: path.to_owned(),
            why,
        })
    };
    let sealed = Sealed::memfd(path)?;
    let checked = match trust::check(&sealed) {
        Ok(checked) => checked,
        Err(trust::Error::Io(e)) => return Err(trust::Error::Io(e).into()),
        Err(e) => return delete(e.to_string()),
    };
    let image = match Image::read(&checked) {
        Ok(image) => image,
        Err(image::Error::Io(e)) => return Err(image::Error::Io(e).into()),
        Err(e) => return delete(e.to_string()),
    };
    let signed = checked.signed().manifest();
    if signed.app_id != *app || signed.root_key_id != pair.root.key_id() {
        return delete(format!(
            "it is not {app} under the key it is installed under"
        ));
    }
    if signed.expires_at.unix_seconds() < now {
        return delete(format!(
            "its signature expired at {}",
            signed.expires_at.as_str()
        ));
    }
    if Verdict::of(pair.mark.as_ref(), signed.version, &signed.payload_sha256) != Verdict::Higher {
        return delete(format!(
            "version {} is not newer than the installed one",
            signed.version
        ));
    }
    apply_newer(layout, &checked, &image, &sealed, pair, path, now, fetch)
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
    ask: Ask,
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
    if pair.installed.or(pair.mark) != Some(mark) {
        return Err(Error::NotInstalledFile { app: app.clone() });
    }
    if signed.expires_at.unix_seconds() < now {
        eprintln!("odk oma: {app} is past its signature window and keeps running.");
    }
    // An upgrade that took effect while asking for more does not launch until the person
    // allows it, or goes back to the previous version (ADR 0015).
    let requested = &image.manifest().permissions;
    let mut granted = pair.permissions.clone().unwrap_or_default();
    let missing = grants::missing(&granted, requested);
    if !grants::is_empty(&missing) {
        let identity = Identity {
            app_id: app,
            version: signed.version,
            root: &pair.root,
            publisher: &signed.publisher,
            fetched: None,
        };
        let has_previous = layout
            .pair_previous_file(&signed.root_key_id, app)
            .is_file();
        let already = crate::fmt::permission_lines(&granted).len();
        let named_dirs = userdirs::resolve(layout.home(), &layout.user_dirs_file());
        let request = Request::upgrade(&identity, &missing, already, has_previous, &named_dirs);
        match ask(&request).map_err(Error::Dialog)? {
            Button::Allow => {
                let mut pair = pair.clone();
                pair.permissions = Some(requested.clone());
                grants::save_pair(layout, app, &pair)?;
                granted = requested.clone();
            }
            Button::KeepPrevious if has_previous => {
                let back = rollback(layout, app)?;
                return run_file(layout, host, &back, wait, now, ask);
            }
            _ => return Err(Error::Cancelled),
        }
    }
    list(layout, app, image.manifest(), None);
    Ok(launch::run_file(
        layout,
        &image,
        &signed.root_key_id,
        &granted,
        host,
        wait,
        false,
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

    /// The time the dev builds are made at, in Unix seconds.
    const NOW: i64 = 1_800_000_000;
    const DAY: i64 = 86_400;

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

    fn app() -> Identifier {
        "com.example.dev".parse().unwrap()
    }

    impl Fixture {
        /// The project's files at `version`, with `permissions` and `extra` oma.toml lines.
        fn write(&self, version: &str, permissions: &str, extra: &str) {
            fs::write(
                self.project.join("manifest.toml"),
                format!(
                    "manifest_version = 1\nidentifier = \"com.example.dev\"\nname = \"Dev\"\nversion = \"{version}\"\nexecutable = \"bin/app\"\npublisher = \"example.com\"\n\n[daemon]\nexecutable = \"bin/d\"\nsocket = \"dev/d.sock\"\n\n[permissions]\n{permissions}\n"
                ),
            )
            .unwrap();
            fs::write(
                self.project.join(dev::FILE),
                format!("executable = \"target/app\"\ninclude = [\"../tool=bin/d\"]\n{extra}"),
            )
            .unwrap();
        }

        fn dev(&self, now: i64, ask: Ask, stopper: &dyn Stopper) -> Result<Prepared, Error> {
            dev(&self.layout, &self.project, None, now, ask, stopper)
        }

        /// The dev root's key id, once it is made.
        fn root(&self) -> KeyId {
            dev_key::root(&self.layout).unwrap().key_id()
        }

        fn pair(&self) -> Option<grants::Pair> {
            grants::load_pair(&self.layout, &self.root(), &app()).unwrap()
        }

        /// The installed Bundle file's semver and the paths in its image.
        fn installed(&self) -> Option<(String, Vec<PathBuf>)> {
            let path = installed_file(&self.layout, &app()).unwrap()?;
            let sealed = Sealed::memfd(&path).unwrap();
            let checked = trust::check(&sealed).unwrap();
            let image = Image::read(&checked).unwrap();
            let paths = image.nodes().iter().map(|n| n.path.clone()).collect();
            Some((image.manifest().version.to_string(), paths))
        }

        fn installed_version(&self) -> Option<String> {
            self.installed().map(|(version, _)| version)
        }

        /// The Identifier installed as a directory Bundle, as `dev` did before ADR 0085, with
        /// consent recorded and `notes.db` in its data directory.
        fn directory_bundle(&self) -> PathBuf {
            let out = self.project.parent().unwrap().join("assembled");
            let assembled = dev::Project::load(&self.project)
                .unwrap()
                .assemble(&out)
                .unwrap();
            let installed = install::install(&self.layout, &assembled, true).unwrap();
            let data = self.layout.data_dir(&app());
            fs::create_dir_all(&data).unwrap();
            fs::write(data.join("notes.db"), "notes").unwrap();
            installed
        }

        fn applications(&self) -> Vec<String> {
            let mut names = fs::read_dir(self.layout.applications())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            names.sort();
            names
        }
    }

    /// What the dialog was shown and answered with.
    struct Dialog {
        shown: RefCell<Vec<Request>>,
        answer: Button,
    }

    impl Dialog {
        fn answering(answer: Button) -> Self {
            Self {
                shown: RefCell::new(Vec::new()),
                answer,
            }
        }
        fn ask(&self) -> impl Fn(&Request) -> Result<Button, consent::Error> + '_ {
            |request| {
                self.shown.borrow_mut().push(request.clone());
                Ok(self.answer)
            }
        }
        fn count(&self) -> usize {
            self.shown.borrow().len()
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

    /// A fetch that counts its calls and never reaches anything.
    fn counted(
        calls: &Cell<usize>,
    ) -> impl Fn(&Publisher) -> Result<fetch::KeySet, fetch::Failure> + '_ {
        |publisher| {
            calls.set(calls.get() + 1);
            Err(fetch::Failure::Unreachable {
                host: publisher.as_str().to_owned(),
                reason: "a test".to_owned(),
            })
        }
    }

    #[test]
    fn the_first_run_makes_the_key_and_asks_once_and_a_later_one_replaces_the_copy_without_asking()
    {
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        let stopper = stops();

        let first = f.dev(NOW, &dialog.ask(), &stopper).unwrap();
        assert!(!first.replaced);
        assert_eq!(first.made_key, Some(f.root()), "the key is made and said");
        assert_eq!(first.installed, f.layout.applications().join("Dev.oma"));
        assert!(first.installed.is_file(), "a Bundle file, not a directory");
        assert_eq!(dialog.count(), 1, "it asks for what is not granted");
        let request = &dialog.shown.borrow()[0];
        assert_eq!(request.buttons, ["install", "cancel"]);
        assert!(
            request.notes[0].contains("development build, signed with this computer's dev key"),
            "{:?}",
            request.notes
        );
        assert_eq!(f.installed_version().as_deref(), Some("1.0.0"));
        let pair = f.pair().unwrap();
        assert_eq!(pair.mark.unwrap().version, NOW as u64);
        assert!(pair.permissions.unwrap().audio);

        // A second build in the same second is still newer.
        f.write("1.1.0", "audio = true", "");
        let second = f.dev(NOW, &dialog.ask(), &stopper).unwrap();
        assert!(second.replaced);
        assert_eq!(second.made_key, None);
        assert_eq!(dialog.count(), 1, "the grant is unchanged, so no dialog");
        assert_eq!(f.installed_version().as_deref(), Some("1.1.0"));
        assert_eq!(f.pair().unwrap().mark.unwrap().version, NOW as u64 + 1);
        assert_eq!(f.applications(), ["Dev.oma"], "nothing is left beside it");
        assert!(
            f.layout.pair_previous_file(&f.root(), &app()).is_file(),
            "the build it replaced is the previous one, for rollback"
        );

        // The daemon is stopped before each install, by the name `ensure` starts it under.
        let stopped = stopper.0.borrow();
        assert_eq!(stopped.len(), 2);
        assert_eq!(
            daemon::unit_name(&stopped[0]),
            "oma-com.example.dev".to_owned()
        );
        let (_, paths) = f.installed().unwrap();
        assert!(paths.contains(&PathBuf::from("bin/d")), "the include is in");
        assert!(
            !paths.contains(&PathBuf::from(dev::FILE)),
            "oma.toml is not"
        );
    }

    #[test]
    fn a_failing_build_makes_no_key_and_changes_nothing() {
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        let stopper = stops();
        let failing = "build = [[\"sh\", \"-c\", \"exit 1\"]]\n";

        f.write("1.0.0", "audio = true", failing);
        let e = f.dev(NOW, &dialog.ask(), &stopper).unwrap_err();
        assert!(matches!(e, Error::Dev(dev::Error::Build { .. })), "{e}");
        assert!(!f.layout.dev_key_dir().exists(), "no key");
        assert!(grants::pairs(&f.layout).unwrap().is_empty(), "no pair");

        f.write("1.0.0", "audio = true", "");
        f.dev(NOW, &dialog.ask(), &stopper).unwrap();
        let (shown, stopped) = (dialog.count(), stopper.0.borrow().len());
        f.write("2.0.0", "audio = true\nvideo = true", failing);
        let e = f.dev(NOW + 1, &dialog.ask(), &stopper).unwrap_err();
        assert!(matches!(e, Error::Dev(dev::Error::Build { .. })), "{e}");
        assert_eq!(f.installed_version().as_deref(), Some("1.0.0"));
        assert_eq!(dialog.count(), shown);
        assert_eq!(stopper.0.borrow().len(), stopped, "nothing was stopped");
    }

    #[test]
    fn cancelling_leaves_the_installed_copy_its_daemon_and_its_pair_alone() {
        let f = fixture();
        let stopper = stops();
        f.dev(NOW, &Dialog::answering(Button::Install).ask(), &stopper)
            .unwrap();
        let (stopped, pair) = (stopper.0.borrow().len(), f.pair());

        // More permissions than were granted: it asks, and the answer is no.
        f.write("2.0.0", "audio = true\ngpu = true", "");
        let no = Dialog::answering(Button::Cancel);
        let e = f.dev(NOW + 1, &no.ask(), &stopper).unwrap_err();
        assert!(matches!(e, Error::Cancelled), "{e}");
        assert_eq!(no.count(), 1);
        assert_eq!(f.installed_version().as_deref(), Some("1.0.0"));
        assert_eq!(stopper.0.borrow().len(), stopped);
        assert_eq!(f.pair(), pair);
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
            NOW,
            &Dialog::answering(Button::Install).ask(),
            &stops(),
        )
        .unwrap();
        assert_eq!(prepared.foreign_daemon, Some(runtime.join("dev/d.sock")));
    }

    #[test]
    fn the_dev_roots_key_set_is_never_fetched_and_does_not_run_out() {
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        f.dev(NOW, &dialog.ask(), &stops()).unwrap();
        let first_set = grants::keys_version(&f.layout, &f.root()).unwrap().unwrap();

        // A year and more later the key set of the first build has run out; the next build
        // signs a newer one.
        let later = NOW + 400 * DAY;
        f.write("1.1.0", "audio = true", "");
        f.dev(later, &dialog.ask(), &stops()).unwrap();
        assert!(grants::keys_version(&f.layout, &f.root()).unwrap().unwrap() > first_set);
        assert_eq!(f.installed_version().as_deref(), Some("1.1.0"));

        // A clock that went back still builds: the build and its key set are newer than the
        // ones kept.
        let kept = grants::keys_version(&f.layout, &f.root()).unwrap().unwrap();
        f.write("1.1.1", "audio = true", "");
        f.dev(later - DAY, &dialog.ask(), &stops()).unwrap();
        assert_eq!(f.installed_version().as_deref(), Some("1.1.1"));
        assert!(
            grants::keys_version(&f.layout, &f.root()).unwrap().unwrap() > kept,
            "a kept set's version never goes down"
        );

        // A newer dev build reaching `odk oma install` is authorized by the kept set, and the
        // publisher's host is never asked for one.
        let (key, _) = dev_key::DevKey::load_or_create(&f.layout).unwrap();
        f.write("1.2.0", "audio = true", "");
        let version = f.pair().unwrap().mark.unwrap().version + 1;
        let out = f.project.parent().unwrap().join("out");
        let file = dev::Project::load(&f.project)
            .unwrap()
            .write_file(&out, &key.signing(version, later).unwrap())
            .unwrap();
        let calls = Cell::new(0);
        install_file(
            &f.layout,
            &file,
            true,
            later,
            &dialog.ask(),
            &counted(&calls),
        )
        .unwrap();
        assert_eq!(calls.get(), 0, "no fetch for the dev root");
        assert_eq!(f.installed_version().as_deref(), Some("1.2.0"));
    }

    #[test]
    fn a_directory_bundle_of_the_identifier_is_replaced_and_its_data_moves_to_the_pair() {
        let f = fixture();
        let directory = f.directory_bundle();
        assert!(f.layout.grants_file(&app()).exists());

        let prepared = f
            .dev(NOW, &Dialog::answering(Button::Install).ask(), &stops())
            .unwrap();
        assert!(prepared.replaced);
        assert_eq!(prepared.installed, directory, "the same name, now a file");
        assert!(directory.is_file());
        assert!(
            install::find(&f.layout, &app()).is_none(),
            "no directory Bundle"
        );
        assert_eq!(f.applications(), ["Dev.oma"], "nothing is left beside it");
        let data = f.layout.pair_data_dir(&f.root(), &app());
        assert_eq!(fs::read_to_string(data.join("notes.db")).unwrap(), "notes");
        assert!(!f.layout.data_dir(&app()).exists());
        assert!(
            !f.layout.grants_file(&app()).exists(),
            "the directory Bundle's consent record goes with it"
        );
    }

    #[test]
    fn a_cancel_leaves_the_directory_bundle_and_its_data_where_they_were() {
        let f = fixture();
        f.directory_bundle();
        let e = f
            .dev(NOW, &Dialog::answering(Button::Cancel).ask(), &stops())
            .unwrap_err();
        assert!(matches!(e, Error::Cancelled), "{e}");
        assert!(install::find(&f.layout, &app()).is_some());
        let old = f.layout.data_dir(&app()).join("notes.db");
        assert_eq!(fs::read_to_string(old).unwrap(), "notes");
        assert!(f.layout.grants_file(&app()).exists());
        assert!(!f.layout.pair_dir(&f.root(), &app()).exists());
    }

    #[test]
    fn a_data_move_that_fails_puts_the_directory_bundle_back() {
        let f = fixture();
        f.directory_bundle();
        // The pair's directory is a file, so its data directory cannot be made.
        dev_key::DevKey::load_or_create(&f.layout).unwrap();
        let pair_dir = f.layout.pair_dir(&f.root(), &app());
        fs::create_dir_all(pair_dir.parent().unwrap()).unwrap();
        fs::write(&pair_dir, "in the way").unwrap();

        let e = f
            .dev(NOW, &Dialog::answering(Button::Install).ask(), &stops())
            .unwrap_err();
        assert!(
            matches!(e, Error::Install(install::Error::Io { .. })),
            "{e}"
        );
        assert!(install::find(&f.layout, &app()).is_some(), "put back");
        assert_eq!(f.applications(), ["Dev.oma"], "nothing beside it");
        let old = f.layout.data_dir(&app()).join("notes.db");
        assert_eq!(fs::read_to_string(old).unwrap(), "notes");
        assert!(f.layout.grants_file(&app()).exists());
    }

    #[test]
    fn data_is_never_merged_into_the_pairs() {
        let f = fixture();
        f.directory_bundle();
        // The dev key is made, and the pair already has data of its own.
        dev_key::DevKey::load_or_create(&f.layout).unwrap();
        let data = f.layout.pair_data_dir(&f.root(), &app());
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("notes.db"), "newer").unwrap();

        let dialog = Dialog::answering(Button::Install);
        let e = f.dev(NOW, &dialog.ask(), &stops()).unwrap_err();
        assert!(matches!(e, Error::DevData { .. }), "{e}");
        assert_eq!(dialog.count(), 0, "refused before anyone is asked");
        assert!(install::find(&f.layout, &app()).is_some());
        let old = f.layout.data_dir(&app()).join("notes.db");
        assert_eq!(fs::read_to_string(old).unwrap(), "notes");
        assert_eq!(fs::read_to_string(data.join("notes.db")).unwrap(), "newer");
    }

    /// The fixture's application as `Other.oma`, signed under a fixture root that is not the dev
    /// root, and that root's key set as its publisher serves it.
    fn released(f: &Fixture) -> (PathBuf, fetch::KeySet) {
        use crate::trust::{Context, OmaKeys, ReleaseKey, SigningKey};
        let root = SigningKey::from_seed(&[0x11; 32]).unwrap();
        let release = SigningKey::from_seed(&[0x22; 32]).unwrap();
        let dir = f.project.parent().unwrap().join("released");
        fs::create_dir_all(&dir).unwrap();
        let text = fs::read_to_string(f.project.join("manifest.toml")).unwrap();
        fs::write(
            dir.join("manifest.toml"),
            text.replace("\"Dev\"", "\"Other\""),
        )
        .unwrap();
        let file = crate::bundle::write_file(
            &f.project.join("target/app"),
            &dir.join("manifest.toml"),
            &dir,
            &[crate::bundle::Include::parse(&format!(
                "{}=bin/d",
                f.project.join("../tool").display()
            ))
            .unwrap()],
            &crate::bundle::Signing {
                release: release.clone(),
                root: root.public_key().key_id(),
                version: 7,
                issued_at: Timestamp::from_unix_seconds(NOW).unwrap(),
                expires_at: Timestamp::from_unix_seconds(NOW + DAY).unwrap(),
                update_url: None,
            },
        )
        .unwrap();
        let set = OmaKeys {
            root_public_key: root.public_key(),
            root_key_id: root.public_key().key_id(),
            version: 1,
            keys: vec![ReleaseKey {
                id: release.public_key().key_id(),
                not_after: Timestamp::from_unix_seconds(NOW + DAY).unwrap(),
                public_key: release.public_key(),
            }],
        };
        let body = set.to_canonical().unwrap();
        let signature = root.sign(Context::KEYS, &body).0.to_vec();
        (file, fetch::KeySet { body, signature })
    }

    #[test]
    fn inspecting_a_file_names_its_pin_and_the_files_held_under_other_roots() {
        let f = fixture();
        let prepared = f
            .dev(NOW, &Dialog::answering(Button::Install).ask(), &stops())
            .unwrap();
        // A file of the same app id under another root, held by `--register`.
        let (file, _) = released(&f);
        let other = trust::check(&Sealed::memfd(&file).unwrap())
            .unwrap()
            .signed()
            .manifest()
            .root_key_id;
        let held = f.layout.held_file(&other, &app());
        fs::create_dir_all(held.parent().unwrap()).unwrap();
        fs::copy(&file, &held).unwrap();

        let inspected = inspect_file(Some(&f.layout), &prepared.installed).unwrap();
        assert_eq!(inspected.manifest.name, "Dev");
        assert_eq!(inspected.signed.version, NOW as u64);
        assert_eq!(inspected.pinned, dev_key::root(&f.layout));
        assert_eq!(inspected.held, [(other, held)]);

        // The held file itself: its root is not pinned, and it is not held under another.
        let inspected = inspect_file(Some(&f.layout), &file).unwrap();
        assert_eq!(inspected.pinned, None);
        assert!(inspected.held.is_empty());
        let alone = inspect_file(None, &prepared.installed).unwrap();
        assert_eq!((alone.pinned, alone.held.len()), (None, 0));
    }

    #[test]
    fn one_copy_of_an_application_is_installed_whatever_its_root() {
        // A dev copy refuses a released one, by app id and not by file name...
        let f = fixture();
        let dialog = Dialog::answering(Button::Install);
        f.dev(NOW, &dialog.ask(), &stops()).unwrap();
        let (file, _) = released(&f);
        let e = install_file(
            &f.layout,
            &file,
            true,
            NOW,
            &dialog.ask(),
            &|_| unreachable!(),
        )
        .unwrap_err();
        assert!(matches!(e, Error::InstalledUnderOtherRoot { .. }), "{e}");
        let host = HostEnv {
            uid: 1000,
            gid: 1000,
            runtime_dir: None,
            wayland_display: None,
        };
        let e = open_file(
            &f.layout,
            &host,
            &file,
            false,
            NOW,
            &dialog.ask(),
            &|_| unreachable!(),
        )
        .unwrap_err();
        assert!(matches!(e, Error::InstalledUnderOtherRoot { .. }), "{e}");

        // ...and a released copy refuses a dev build, naming it, before anyone is asked.
        let f = fixture();
        let (file, set) = released(&f);
        let installed = install_file(
            &f.layout,
            &file,
            false,
            NOW,
            &Dialog::answering(Button::Install).ask(),
            &|_| Ok(set.clone()),
        )
        .unwrap();
        let dialog = Dialog::answering(Button::Install);
        let stopper = stops();
        let e = f.dev(NOW, &dialog.ask(), &stopper).unwrap_err();
        match e {
            Error::InstalledUnderOtherRoot { path, .. } => assert_eq!(path, installed),
            e => panic!("{e}"),
        }
        assert_eq!(dialog.count(), 0);
        assert!(stopper.0.borrow().is_empty());
        assert_eq!(f.applications(), ["Other.oma"]);
    }

    #[test]
    fn installing_a_directory_bundle_lists_it_in_the_launcher_and_run_once_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path().join("home"), dir.path().join("data"));
        let source = crate::testutil::bundle(dir.path(), "Probe", "com.example.probe", "1.0.0", "");
        let manifest = source.join("manifest.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            &manifest,
            text.replace(
                "executable = \"bin/app\"",
                "executable = \"bin/app\"\nicon = \"icon.png\"",
            ),
        )
        .unwrap();
        std::fs::write(source.join("icon.png"), b"png").unwrap();
        let installed = install(&layout, &source, true, &|_| unreachable!()).unwrap();
        let entry = layout.desktop_entries().join("com.example.probe.desktop");
        let written = std::fs::read_to_string(&entry).unwrap();
        assert!(
            written.contains("Exec=odk oma run com.example.probe\n"),
            "{written}"
        );
        let icon = std::fs::canonicalize(&installed).unwrap().join("icon.png");
        assert!(
            written.contains(&format!("Icon={}\n", icon.display())),
            "{written}"
        );

        // One with no icon still gets an entry, without an Icon line.
        let plain = crate::testutil::bundle(dir.path(), "Plain", "com.example.plain", "1.0.0", "");
        install(&layout, &plain, true, &|_| unreachable!()).unwrap();
        let written =
            std::fs::read_to_string(layout.desktop_entries().join("com.example.plain.desktop"))
                .unwrap();
        assert!(!written.contains("Icon="), "{written}");

        // Run once of another Bundle changes nothing there.
        let before = std::fs::read_dir(layout.desktop_entries()).unwrap().count();
        let once = crate::testutil::bundle(
            dir.path(),
            "Once",
            "com.example.once",
            "1.0.0",
            "network = true",
        );
        let host = HostEnv {
            uid: 1000,
            gid: 1000,
            runtime_dir: None,
            wayland_display: None,
        };
        let _ = run(
            &layout,
            &host,
            once.to_str().unwrap(),
            true,
            &|_| Ok(Button::RunOnce),
            &|_| unreachable!(),
        );
        assert_eq!(
            std::fs::read_dir(layout.desktop_entries()).unwrap().count(),
            before
        );
        assert!(
            !layout
                .desktop_entries()
                .join("com.example.once.desktop")
                .exists()
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
            run_file(&layout, &host, &path, true, soon(), &|r| Dialog::answering(
                &[]
            )
            .ask(r)),
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
            run_file(&layout, &host, &newer, true, soon(), &|r| {
                Dialog::answering(&[]).ask(r)
            }),
            Err(Error::NotInstalledFile { .. })
        ));
        // The installed one gets as far as the launch, which needs a runtime directory.
        assert!(!matches!(
            run_file(&layout, &host, &installed, true, soon(), &|r| {
                Dialog::answering(&[]).ask(r)
            }),
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

    #[test]
    fn rollback_puts_the_previous_file_back_and_keeps_the_mark() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let app: Identifier = APP.parse().unwrap();
        let host = HostEnv {
            uid: 1000,
            gid: 1000,
            runtime_dir: None,
            wayland_display: None,
        };
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let v7_bytes = std::fs::read(&v7).unwrap();
        let first = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        assert!(matches!(
            rollback(&layout, &app),
            Err(Error::NoPrevious { .. })
        ));
        let v8 = bundle_file(&dir, |t| t, |m| m.version = 8);
        let v8_bytes = std::fs::read(&v8).unwrap();
        let quiet = Dialog::answering(&[]);
        let target = install_with(&layout, &v8, false, soon(), &quiet, &site).unwrap();

        assert_eq!(rollback(&layout, &app).unwrap(), target);
        assert_eq!(std::fs::read(&target).unwrap(), v7_bytes);
        let pair = pair_of(&layout).unwrap();
        assert_eq!(pair.mark.unwrap().version, 8, "the mark stays");
        assert_eq!(pair.installed.unwrap().version, 7);
        // The installed file now is the one that runs: it passes the check, and stops only at
        // the launch, which has no runtime directory here.
        assert!(!matches!(
            run_file(&layout, &host, &target, true, soon(), &|r| {
                Dialog::answering(&[]).ask(r)
            }),
            Err(Error::NotInstalledFile { .. })
        ));

        // Installing the marked version again puts it back, without a dialog or a fetch.
        let fetches = site.fetches.get();
        install_with(&layout, &v8, false, soon(), &quiet, &site).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), v8_bytes);
        assert_eq!(pair_of(&layout).unwrap().installed.unwrap().version, 8);
        assert_eq!(site.fetches.get(), fetches);
        assert!(quiet.shown.borrow().is_empty());
    }

    #[test]
    fn purge_waits_until_the_app_is_gone_then_forgets_it() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let app: Identifier = APP.parse().unwrap();
        let root_id = root(1).public_key().key_id();
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        std::fs::create_dir_all(layout.pair_data_dir(&root_id, &app)).unwrap();
        std::fs::write(layout.pair_data_dir(&root_id, &app).join("notes.db"), "x").unwrap();

        assert!(matches!(
            purge(&layout, &app),
            Err(Error::StillInstalled { .. })
        ));
        assert!(pair_of(&layout).is_some());

        std::fs::remove_file(&target).unwrap();
        assert_eq!(purge(&layout, &app).unwrap(), [root_id]);
        assert!(pair_of(&layout).is_none());
        assert!(!layout.pair_dir(&root_id, &app).exists());
        assert!(matches!(
            purge(&layout, &app),
            Err(Error::NothingToPurge { .. })
        ));

        // With its mark gone, any version installs again, as at first contact.
        let again = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v7, false, soon(), &again, &site).unwrap();
        assert_eq!(again.shown.borrow().len(), 1);
    }

    fn open_with(layout: &Layout, path: &Path, dialog: &Dialog, site: &Site) -> Result<i32, Error> {
        // No runtime directory: whatever reaches the launch stops there, after every decision.
        let host = HostEnv {
            uid: 1000,
            gid: 1000,
            runtime_dir: None,
            wayland_display: None,
        };
        open_file(
            layout,
            &host,
            path,
            true,
            soon(),
            &|r| dialog.ask(r),
            &|p| site.fetch(p),
        )
    }

    #[test]
    fn opening_a_new_publishers_file_asks_install_run_once_or_cancel() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let path = bundle_file(&dir, with_network, |_| {});
        let target = layout.applications().join("Notes.oma");

        // Run once: launched, with nothing written.
        let once = Dialog::answering(&[Button::RunOnce]);
        let e = open_with(&layout, &path, &once, &site).unwrap_err();
        assert!(matches!(e, Error::Launch(_)), "{e}");
        assert_eq!(
            once.shown.borrow()[0].buttons,
            ["install", "run-once", "cancel"]
        );
        assert_eq!(once.shown.borrow()[0].lines.len(), 1, "the network line");
        assert!(pair_of(&layout).is_none() && !target.exists());
        let root_id = root(1).public_key().key_id();
        assert_eq!(grants::keys_version(&layout, &root_id).unwrap(), None);
        assert!(!layout.pair_dir(&root_id, &APP.parse().unwrap()).exists());

        // Install: installed, then launched as the installed copy.
        let install = Dialog::answering(&[Button::Install]);
        let e = open_with(&layout, &path, &install, &site).unwrap_err();
        assert!(matches!(e, Error::Launch(_)), "{e}");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            std::fs::read(&path).unwrap()
        );
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 7);
    }

    #[test]
    fn a_cancelled_file_is_not_asked_about_or_fetched_for_again() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let path = bundle_file(&dir, |t| t, |_| {});
        let cancel = Dialog::answering(&[Button::Cancel]);
        assert!(matches!(
            open_with(&layout, &path, &cancel, &site),
            Err(Error::Cancelled)
        ));
        let fetches = site.fetches.get();
        let quiet = Dialog::answering(&[]);
        assert!(matches!(
            open_with(&layout, &path, &quiet, &site),
            Err(Error::Denied { .. })
        ));
        assert!(quiet.shown.borrow().is_empty());
        assert_eq!(site.fetches.get(), fetches);
    }

    #[test]
    fn opening_the_installed_version_runs_it_and_another_version_does_not_run() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let fetches = site.fetches.get();

        // A copy of the installed file elsewhere runs, without a dialog or a fetch.
        let copy = dir.path().join("copy.oma");
        std::fs::copy(&v7, &copy).unwrap();
        let quiet = Dialog::answering(&[]);
        let e = open_with(&layout, &copy, &quiet, &site).unwrap_err();
        assert!(matches!(e, Error::Launch(_)), "{e}");

        let v8 = bundle_file(&dir, |t| t, |m| m.version = 8);
        assert!(matches!(
            open_with(&layout, &v8, &quiet, &site),
            Err(Error::UpgradeByOpening { .. })
        ));
        let v6 = bundle_file(&dir, |t| t, |m| m.version = 6);
        assert!(matches!(
            open_with(&layout, &v6, &quiet, &site),
            Err(Error::Version { .. })
        ));
        assert!(quiet.shown.borrow().is_empty());
        assert_eq!(site.fetches.get(), fetches);
    }

    #[test]
    fn register_keeps_holds_deletes_and_applies_without_asking() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let apps = layout.applications();
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        let installed = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let put = |name: &str, from: &Path| {
            std::fs::copy(from, apps.join(name)).unwrap();
            apps.join(name)
        };

        let mut tampered = std::fs::read(&v7).unwrap();
        *tampered.last_mut().unwrap() ^= 1;
        std::fs::write(apps.join("Tampered.oma"), tampered).unwrap();
        let lower = put("Lower.oma", &bundle_file(&dir, |t| t, |m| m.version = 6));
        let expired = put(
            "Expired.oma",
            &bundle_file(
                &dir,
                |t| t,
                |m| {
                    m.version = 9;
                    m.expires_at = Timestamp::parse("2026-10-28T00:00:00Z").unwrap();
                },
            ),
        );
        let other_root = put(
            "Other.oma",
            &bundle_file(
                &dir,
                |t| t,
                |m| m.root_key_id = root(2).public_key().key_id(),
            ),
        );
        let newer = put("Newer.oma", &bundle_file(&dir, |t| t, |m| m.version = 8));
        let directory = apps.join("Atmos.oma");
        std::fs::create_dir_all(directory.join("bin")).unwrap();

        let done = register(&layout, soon(), &|p| site.fetch(p), &|_| Ok(()));
        let deleted: Vec<_> = done
            .iter()
            .filter_map(|r| match r {
                Registered::Deleted { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(deleted.len(), 3, "{done:?}");
        for path in [apps.join("Tampered.oma"), lower, expired] {
            assert!(
                deleted.contains(&path) && !path.exists(),
                "{}",
                path.display()
            );
        }
        let held = layout.held_file(&root(2).public_key().key_id(), &APP.parse().unwrap());
        assert!(done.contains(&Registered::Held {
            app: APP.parse().unwrap(),
            from: other_root.clone(),
            to: held.clone(),
        }));
        assert!(held.is_file() && !other_root.exists());

        // The newer version is applied: installed in place of v7, which is kept as previous.
        assert!(done.contains(&Registered::Applied(installed.clone())));
        assert!(!newer.exists());
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 8);
        assert_eq!(
            std::fs::read(&installed).unwrap(),
            std::fs::read(bundle_file(&dir, |t| t, |m| m.version = 8)).unwrap()
        );
        assert!(
            directory.join("bin").is_dir(),
            "a directory Bundle is not touched"
        );

        // Run again, it keeps what is there.
        let again = register(&layout, soon(), &|p| site.fetch(p), &|_| Ok(()));
        assert_eq!(again, [Registered::Kept(installed)]);
    }

    #[test]
    fn an_upgrade_that_asks_for_more_takes_effect_and_its_launch_asks() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let apps = layout.applications();
        let host = HostEnv {
            uid: 1000,
            gid: 1000,
            runtime_dir: None,
            wayland_display: None,
        };
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();

        let more = apps.join("More.oma");
        std::fs::copy(bundle_file(&dir, with_network, |m| m.version = 8), &more).unwrap();
        let done = register(&layout, soon(), &|p| site.fetch(p), &|_| Ok(()));
        assert!(
            done.contains(&Registered::Applied(target.clone())),
            "{done:?}"
        );
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 8);
        assert!(!pair_of(&layout).unwrap().permissions.unwrap().network);

        // Its launch asks; Cancel runs nothing.
        let cancel = Dialog::answering(&[Button::Cancel]);
        assert!(matches!(
            run_file(&layout, &host, &target, true, soon(), &|r| cancel.ask(r)),
            Err(Error::Cancelled)
        ));
        let shown = cancel.shown.borrow();
        assert_eq!(shown[0].situation, "upgrade");
        assert_eq!(shown[0].buttons, ["allow", "keep-previous", "cancel"]);
        assert_eq!(shown[0].lines.len(), 1, "only the new network line");

        // Keep previous version goes back to 7 and runs that.
        let keep = Dialog::answering(&[Button::KeepPrevious]);
        let e = run_file(&layout, &host, &target, true, soon(), &|r| keep.ask(r)).unwrap_err();
        assert!(matches!(e, Error::Launch(_)), "{e}");
        assert_eq!(pair_of(&layout).unwrap().installed.unwrap().version, 7);
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 8);

        // Back on 8, Allow records the network and runs it.
        rollback(&layout, &APP.parse().unwrap()).unwrap();
        let allow = Dialog::answering(&[Button::Allow]);
        let e = run_file(&layout, &host, &target, true, soon(), &|r| allow.ask(r)).unwrap_err();
        assert!(matches!(e, Error::Launch(_)), "{e}");
        assert!(pair_of(&layout).unwrap().permissions.unwrap().network);

        // Offline, the key set kept from the install still authorizes a newer file in hand.
        let newer = apps.join("Newer.oma");
        std::fs::copy(bundle_file(&dir, with_network, |m| m.version = 9), &newer).unwrap();
        let offline = Site::failing(fetch::Failure::HostNotFound { host: HOST.into() });
        let done = register(&layout, soon(), &|p| offline.fetch(p), &|_| Ok(()));
        assert!(
            done.contains(&Registered::Applied(target.clone())),
            "{done:?}"
        );
        assert!(!newer.exists());
    }

    #[test]
    fn register_takes_a_newer_version_from_the_inbox_and_deletes_anything_else_there() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let app: Identifier = APP.parse().unwrap();
        let inbox = layout.pair_inbox_dir(&root(1).public_key().key_id(), &app);
        std::fs::create_dir_all(&inbox).unwrap();
        let put = |name: &str, from: PathBuf| {
            std::fs::copy(from, inbox.join(name)).unwrap();
            inbox.join(name)
        };
        let older = put("Older.oma", bundle_file(&dir, |t| t, |m| m.version = 6));
        let other = put(
            "Other.oma",
            bundle_file(
                &dir,
                |t| t.replace("dev.csfh.notes", "dev.csfh.other"),
                |m| {
                    m.app_id = "dev.csfh.other".parse().unwrap();
                    m.version = 9;
                },
            ),
        );
        let staged = put("Up.oma", bundle_file(&dir, |t| t, |m| m.version = 8));
        let partial = put("Up.oma.stage", bundle_file(&dir, |t| t, |m| m.version = 8));

        let done = register(&layout, soon(), &|p| site.fetch(p), &|_| Ok(()));
        assert!(
            done.contains(&Registered::Applied(target.clone())),
            "{done:?}"
        );
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 8);
        assert!(!staged.exists());
        for gone in [older, other] {
            assert!(
                done.iter()
                    .any(|r| matches!(r, Registered::Deleted { path, .. } if *path == gone)),
                "{done:?}"
            );
            assert!(!gone.exists());
        }
        assert!(partial.exists(), "a file still being staged is not touched");
    }

    #[test]
    fn register_lists_each_installed_file_in_the_launcher_and_unlists_it_when_it_goes() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        assert!(register(&layout, soon(), &|_| unreachable!(), &|_| Ok(())).is_empty());
        assert!(layout.applications().is_dir(), "created for the path unit");

        let site = Site::serving(key_set_bytes(1, &release_key()));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let entries = layout.desktop_entries();
        std::fs::create_dir_all(&entries).unwrap();
        // Someone else's entry, even one named like ours, is never touched.
        let foreign = entries.join("oma-dev.csfh.foreign.desktop");
        std::fs::write(&foreign, "[Desktop Entry]\nName=Foreign\n").unwrap();

        register(&layout, soon(), &|p| site.fetch(p), &|_| Ok(()));
        let entry = entries.join(format!("{APP}.desktop"));
        assert_eq!(
            std::fs::read_to_string(&entry).unwrap(),
            format!(
                "[Desktop Entry]\nType=Application\nName=Notes\nExec=odk oma run {APP}\nX-Oma-Identifier={APP}\n"
            )
        );

        std::fs::remove_file(&target).unwrap();
        register(&layout, soon(), &|p| site.fetch(p), &|_| Ok(()));
        assert!(!entry.exists());
        assert!(foreign.exists());
    }

    #[test]
    fn register_watches_the_inbox_of_an_app_that_may_stage_upgrades_and_stops_when_it_goes() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let site = Site::serving(key_set_bytes(1, &release_key()));
        let with_updates =
            |t: String| t.replace("[permissions]\n", "[permissions]\nupdates = true\n");
        let v7 = bundle_file(&dir, with_updates, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        let target = install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let calls = RefCell::new(Vec::<Vec<String>>::new());
        let systemctl = |args: &[&str]| {
            calls
                .borrow_mut()
                .push(args.iter().map(|a| a.to_string()).collect());
            Ok(())
        };
        let unit = format!(
            "oma-inbox@{}-dev.csfh.notes.path",
            root(1).public_key().key_id()
        );

        register(&layout, soon(), &|p| site.fetch(p), &systemctl);
        assert_eq!(
            *calls.borrow(),
            [vec!["--user", "enable", "--now", unit.as_str()]]
        );
        // Again: nothing changed, nothing asked.
        register(&layout, soon(), &|p| site.fetch(p), &systemctl);
        assert_eq!(calls.borrow().len(), 1);

        std::fs::remove_file(&target).unwrap();
        register(&layout, soon(), &|p| site.fetch(p), &systemctl);
        assert_eq!(
            calls.borrow()[1],
            ["--user", "disable", "--now", unit.as_str()]
        );
    }

    #[test]
    fn an_instance_name_escapes_what_systemd_escapes() {
        assert_eq!(
            unit_escape("0123456789abcdef/dev.csfh.my-app"),
            "0123456789abcdef-dev.csfh.my\\x2dapp"
        );
        assert_eq!(unit_escape(".hidden"), "\\x2ehidden");
    }

    #[test]
    fn an_upgrade_in_hand_installs_offline_from_the_kept_key_set_checked_again() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let root_id = root(1).public_key().key_id();
        let site = Site::serving(key_set_version(1, &release_key(), 5));
        let v7 = bundle_file(&dir, |t| t, |_| {});
        let first = Dialog::answering(&[Button::Install]);
        install_with(&layout, &v7, false, soon(), &first, &site).unwrap();
        let (body_file, signature_file) = layout.key_set_files(&root_id);
        let kept = std::fs::read(&body_file).unwrap();
        assert_eq!(kept, key_set_version(1, &release_key(), 5).body);

        let offline = Site::failing(fetch::Failure::HostNotFound { host: HOST.into() });
        let quiet = Dialog::answering(&[]);
        let v8 = bundle_file(&dir, |t| t, |m| m.version = 8);
        install_with(&layout, &v8, false, soon(), &quiet, &offline).unwrap();
        assert_eq!(pair_of(&layout).unwrap().mark.unwrap().version, 8);

        // The kept set with a byte changed does not verify under the pin.
        let mut changed = kept.clone();
        let last = changed.len() - 2;
        changed[last] ^= 1;
        std::fs::write(&body_file, &changed).unwrap();
        let v9 = bundle_file(&dir, |t| t, |m| m.version = 9);
        assert!(matches!(
            install_with(&layout, &v9, false, soon(), &quiet, &offline),
            Err(Error::Trust(_))
        ));

        // An older set the root really signed, swapped in, is not the version kept.
        let older = key_set_version(1, &release_key(), 4);
        std::fs::write(&body_file, &older.body).unwrap();
        std::fs::write(&signature_file, &older.signature).unwrap();
        assert!(matches!(
            install_with(&layout, &v9, false, soon(), &quiet, &offline),
            Err(Error::KeptKeySet { kept: 5, found: 4 })
        ));

        // Its release key past its `not_after` (well past: the boundary is question 7's).
        std::fs::write(&body_file, &kept).unwrap();
        std::fs::write(
            &signature_file,
            key_set_version(1, &release_key(), 5).signature,
        )
        .unwrap();
        let later = Timestamp::parse("2027-06-01T00:00:00Z")
            .unwrap()
            .unix_seconds();
        assert!(matches!(
            install_with(&layout, &v9, false, later, &quiet, &offline),
            Err(Error::ReleaseKeyExpired { .. })
        ));
        assert!(quiet.shown.borrow().is_empty());

        // A first contact is never made from a kept set.
        let other = bundle_file(
            &dir,
            |t| {
                t.replace("dev.csfh.notes", "dev.csfh.other")
                    .replace("\"Notes\"", "\"Other\"")
            },
            |m| {
                m.app_id = "dev.csfh.other".parse().unwrap();
                m.root_key_id = root(2).public_key().key_id();
            },
        );
        assert!(matches!(
            install_with(&layout, &other, false, soon(), &quiet, &offline),
            Err(Error::Fetch(_))
        ));
    }
}
