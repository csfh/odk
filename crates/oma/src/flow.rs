//! The user-facing flows that need consent (ADR 0033): one consent step shared by `run`,
//! `install` and `consent`. Everything privileged lives elsewhere; this decides what to ask,
//! acts on the answer and fails closed on anything unexpected.

use std::path::{Path, PathBuf};

use crate::consent::{self, Button, Request};
use crate::daemon::{self, Stopper};
use crate::fmt::{Bundle, Identifier, Permissions};
use crate::layout::Layout;
use crate::sandbox::launch::{self, HostEnv, Options};
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
