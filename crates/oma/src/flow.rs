//! The user-facing flows that need consent (ADR 0033): one consent step shared by `run`,
//! `install` and `consent`. Everything privileged lives elsewhere; this decides what to ask,
//! acts on the answer and fails closed on anything unexpected.

use std::path::{Path, PathBuf};

use crate::consent::{self, Button, Identity, Request};
use crate::daemon::{self, Stopper};
use crate::fmt::{Bundle, Identifier, Permissions};
use crate::layout::Layout;
use crate::sandbox::launch::{self, HostEnv, Options};
use crate::trust::{KeyId, Publisher, SelfSigned, VerifiedKeys};
use crate::{dev, grants, install, userdirs};

/// Shows the dialog. The real one is [`consent::ask`].
pub type Ask<'a> = &'a dyn Fn(&Request) -> Result<Button, consent::Error>;

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
/// file opened from disk, which has no download host. Consent for the image's permissions is not
/// asked here: how those lines are read is open (`docs/oma-trust-plan.md`, question 12).
///
/// A root not pinned for this app id is asked about even when nothing else would be. Install
/// writes the pin, and Cancel writes a denial. Run once writes nothing. A host one edit away
/// from a host pinned for another root is confirmed on its own first. Matching a Unicode
/// confusable is open (question 15).
pub fn admit(
    layout: &Layout,
    file: &SelfSigned,
    keys: &VerifiedKeys,
    fetched: Option<&str>,
    occasion: Occasion,
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
    let request = Request::new_root(
        &identity,
        also_for,
        &[Button::Install, Button::RunOnce, Button::Cancel],
    );
    match ask(&request).map_err(Error::Dialog)? {
        Button::Install => {
            let pair = grants::Pair {
                root: *root,
                publisher: manifest.publisher.clone(),
                permissions: existing.as_ref().and_then(|p| p.permissions.clone()),
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
        /// The project's files at `version`, with `permissions` and `extra` dev.toml lines.
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
        assert!(!root.join(dev::FILE).exists(), "dev.toml is not");
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
        crate::trust::check(&Sealed::memfd(&path).unwrap()).unwrap()
    }

    /// The root of `root_seed`'s key set, naming the fixture release key, verified on first
    /// contact.
    fn key_set(root_seed: u8) -> VerifiedKeys {
        let root = root(root_seed);
        let release = release_key().public_key();
        let set = OmaKeys {
            root_public_key: root.public_key(),
            root_key_id: root.public_key().key_id(),
            version: 1,
            keys: vec![ReleaseKey {
                id: release.key_id(),
                not_after: Timestamp::parse("2027-01-01T00:00:00Z").unwrap(),
                public_key: release,
            }],
        };
        let body = set.to_canonical().unwrap();
        let signature = root.sign(Context::KEYS, &body);
        keys::verify(&body, &signature.0, Signer::FirstContact).unwrap()
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
        admit(layout, file, keys, None, occasion, &|r| dialog.ask(r))
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
        assert!(!pair.denied && pair.permissions.is_none() && pair.mark.is_none());

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
        assert_eq!(shown[1].buttons, ["install", "run-once", "cancel"]);
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
}
