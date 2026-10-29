//! `odk oma self-upgrade` (ADR 0060, section 8; ADR 0086; ADR 0087): replaces this user's odk
//! with the release `odk.csfh.dev` serves, when its document verifies under the release root
//! compiled into the running binary and its version is above the one installed. The running
//! binary decides; the release's own `install.sh`, checked by digest, then puts the files down,
//! so there is one installer.

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::fetch::Failure;
use crate::layout::Layout;
use crate::trust::release::{self, Asset};
use crate::trust::{self, PublicKey};

/// Where releases are served (ADR 0086).
pub const HOST: &str = "odk.csfh.dev";

/// Fetches one file of the release by name, at most `limit` bytes. The real one is
/// [`crate::fetch::get`] from [`HOST`].
pub type Get<'a> = &'a dyn Fn(&str, u64) -> Result<Vec<u8>, Failure>;

/// Runs the checked `install.sh` in the directory that holds it and the tarball.
pub type Install<'a> = &'a dyn Fn(&Path) -> Result<(), String>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("this build has no release root, so it accepts no release (ADR 0086)")]
    NoReleaseRoot,
    #[error("odk is never upgraded as root (ADR 0087); run this as the user it is installed for")]
    AsRoot,
    #[error("{0}; nothing was replaced")]
    Fetch(Failure),
    #[error("the release does not verify: {0}; nothing was replaced")]
    Trust(#[from] trust::Error),
    #[error("{}: {reason}; nothing was replaced", path.display())]
    Receipt { path: PathBuf, reason: String },
    #[error("the {0} fetched is not the one the release names; nothing was replaced")]
    Digest(&'static str),
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the release's install.sh failed: {0}")]
    Script(String),
}

/// What [`self_upgrade`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The release served is not above the one installed.
    UpToDate { installed: u64, offered: u64 },
    /// `installed` is `None` for an install older than the receipt (ADR 0063).
    Upgraded { installed: Option<u64>, to: u64 },
}

/// The version `runtime.toml` holds, or `None` when there is none.
pub fn installed(layout: &Layout) -> Result<Option<u64>, Error> {
    let path = layout.receipt_file();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(Error::Io { path, source }),
    };
    let bad = |reason: &str| Error::Receipt {
        path: path.clone(),
        reason: reason.to_owned(),
    };
    let table: toml::Table = toml::from_str(&text).map_err(|e| bad(&e.to_string()))?;
    let version = table
        .get("version")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| bad("it names no version"))?;
    u64::try_from(version)
        .map(Some)
        .map_err(|_| bad("its version is negative"))
}

/// Upgrades this user's odk when the release served verifies under `root`, the release root
/// compiled into the running binary, and is newer than the receipt. `euid` is the effective user
/// id running it. A host that cannot be reached, another root, a release not above the installed
/// one and a file that is not the one the release names each leave everything as it was.
pub fn self_upgrade(
    layout: &Layout,
    root: Option<&PublicKey>,
    euid: u32,
    get: Get,
    install: Install,
) -> Result<Outcome, Error> {
    let root = root.ok_or(Error::NoReleaseRoot)?;
    if euid == 0 {
        return Err(Error::AsRoot);
    }
    let installed = installed(layout)?;
    let body = get("release", release::MAX_BODY as u64).map_err(Error::Fetch)?;
    let signature = get("release.sig", 64).map_err(Error::Fetch)?;
    let release = release::verify(&body, &signature, root)?;
    // The name `install.sh` downloads; `sign-release` signs no other.
    if release.tarball != crate::odk_release::TARBALL {
        return Err(Error::Digest("tarball"));
    }
    if let Some(installed) = installed.filter(|v| release.version <= *v) {
        return Ok(Outcome::UpToDate {
            installed,
            offered: release.version,
        });
    }

    let script = fetched(get, "install.sh", "install.sh", &release.script)?;
    let tarball = fetched(get, &release.tarball, "tarball", &release.tarball_asset)?;
    // Beside each other in a directory only this user can enter, for the script to install from.
    let dir = layout
        .state_dir()
        .join("self-upgrade")
        .join(std::process::id().to_string());
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| Error::Io { path, source }
    };
    let _ = fs::remove_dir_all(&dir);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(io(&dir))?;
    let result = fs::write(dir.join("install.sh"), &script)
        .map_err(io(&dir.join("install.sh")))
        .and_then(|()| {
            fs::write(dir.join(&release.tarball), &tarball).map_err(io(&dir.join(&release.tarball)))
        })
        .and_then(|()| install(&dir).map_err(Error::Script));
    let _ = fs::remove_dir_all(&dir);
    result.map(|()| Outcome::Upgraded {
        installed,
        to: release.version,
    })
}

/// `name`, fetched within its length and held to its digest. `what` names it in an error.
fn fetched(get: Get, name: &str, what: &'static str, asset: &Asset) -> Result<Vec<u8>, Error> {
    let bytes = get(name, asset.length).map_err(Error::Fetch)?;
    if bytes.len() as u64 != asset.length
        || <[u8; 32]>::from(Sha256::digest(&bytes)) != asset.sha256
    {
        return Err(Error::Digest(what));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use tempfile::TempDir;

    use super::*;
    use crate::trust::release::Release;
    use crate::trust::{Context, SigningKey, Timestamp};

    // Fixture keys, never real ones.
    fn root() -> SigningKey {
        SigningKey::from_seed(&[0x71; 32]).unwrap()
    }

    const SCRIPT: &[u8] = b"#!/bin/sh\necho install\n";
    const TARBALL: &[u8] = b"a tarball";

    /// What the host serves: a release of `version`, signed by `signer`.
    fn served(version: u64, signer: &SigningKey) -> Vec<(String, Vec<u8>)> {
        let asset = |bytes: &[u8]| Asset {
            sha256: Sha256::digest(bytes).into(),
            length: bytes.len() as u64,
        };
        let release = Release {
            root_key_id: signer.public_key().key_id(),
            version,
            issued_at: Timestamp::from_unix_seconds(1_800_000_000).unwrap(),
            tarball: "odk-x86_64-linux.tar.gz".to_owned(),
            tarball_asset: asset(TARBALL),
            script: asset(SCRIPT),
        };
        let body = release.to_canonical().unwrap();
        let signature = signer.sign(Context::RELEASE, &body).0.to_vec();
        vec![
            ("release".to_owned(), body),
            ("release.sig".to_owned(), signature),
            ("install.sh".to_owned(), SCRIPT.to_vec()),
            ("odk-x86_64-linux.tar.gz".to_owned(), TARBALL.to_vec()),
        ]
    }

    struct Host {
        files: RefCell<Vec<(String, Vec<u8>)>>,
        asked: RefCell<Vec<String>>,
        down: bool,
    }

    impl Host {
        fn serving(files: Vec<(String, Vec<u8>)>) -> Self {
            Self {
                files: RefCell::new(files),
                asked: RefCell::new(Vec::new()),
                down: false,
            }
        }
        fn get(&self) -> impl Fn(&str, u64) -> Result<Vec<u8>, Failure> + '_ {
            |name, limit| {
                self.asked.borrow_mut().push(name.to_owned());
                let unreachable = |reason: &str| Failure::Unreachable {
                    host: HOST.to_owned(),
                    reason: reason.to_owned(),
                };
                if self.down {
                    return Err(unreachable("down"));
                }
                let files = self.files.borrow();
                let (_, bytes) = files
                    .iter()
                    .find(|(n, _)| n == name)
                    .ok_or_else(|| unreachable("404"))?;
                if bytes.len() as u64 > limit {
                    return Err(unreachable("over the limit"));
                }
                Ok(bytes.clone())
            }
        }
    }

    /// An `install.sh` run that keeps what it was given.
    struct Runs(RefCell<Vec<(Vec<u8>, Vec<u8>)>>, Cell<bool>);

    impl Runs {
        fn new() -> Self {
            Self(RefCell::new(Vec::new()), Cell::new(false))
        }
        fn install(&self) -> impl Fn(&Path) -> Result<(), String> + '_ {
            |dir| {
                let script = fs::read(dir.join("install.sh")).unwrap();
                let tarball = fs::read(dir.join("odk-x86_64-linux.tar.gz")).unwrap();
                self.0.borrow_mut().push((script, tarball));
                if self.1.get() {
                    Err("it failed".to_owned())
                } else {
                    Ok(())
                }
            }
        }
    }

    fn a_layout(dir: &TempDir) -> Layout {
        Layout::new(dir.path().join("home"), dir.path().join("data"))
    }

    fn receipt(layout: &Layout, version: u64) {
        let path = layout.receipt_file();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("# a receipt\nversion = {version}\n")).unwrap();
    }

    #[test]
    fn a_newer_release_under_the_root_runs_its_own_install_sh_on_its_own_tarball() {
        let dir = TempDir::new().unwrap();
        let layout = a_layout(&dir);
        receipt(&layout, 1_000);
        let host = Host::serving(served(2_000, &root()));
        let runs = Runs::new();
        let key = root().public_key();
        let done = self_upgrade(&layout, Some(&key), 1000, &host.get(), &runs.install()).unwrap();
        assert_eq!(
            done,
            Outcome::Upgraded {
                installed: Some(1_000),
                to: 2_000
            }
        );
        assert_eq!(*runs.0.borrow(), [(SCRIPT.to_vec(), TARBALL.to_vec())]);
        assert!(
            !layout
                .state_dir()
                .join("self-upgrade")
                .join(std::process::id().to_string())
                .exists(),
            "the files are gone once it ran"
        );

        // A script that fails is said to have failed.
        runs.1.set(true);
        let e = self_upgrade(&layout, Some(&key), 1000, &host.get(), &runs.install()).unwrap_err();
        assert!(matches!(e, Error::Script(_)), "{e}");
        runs.1.set(false);

        // An install from before the receipt takes any signed release.
        let dir = TempDir::new().unwrap();
        let done = self_upgrade(
            &a_layout(&dir),
            Some(&key),
            1000,
            &host.get(),
            &runs.install(),
        );
        assert_eq!(
            done.unwrap(),
            Outcome::Upgraded {
                installed: None,
                to: 2_000
            }
        );
    }

    #[test]
    fn the_same_or_an_older_release_fetches_nothing_more_and_runs_nothing() {
        for offered in [2_000, 1_999] {
            let dir = TempDir::new().unwrap();
            let layout = a_layout(&dir);
            receipt(&layout, 2_000);
            let host = Host::serving(served(offered, &root()));
            let runs = Runs::new();
            let key = root().public_key();
            let done =
                self_upgrade(&layout, Some(&key), 1000, &host.get(), &runs.install()).unwrap();
            assert_eq!(
                done,
                Outcome::UpToDate {
                    installed: 2_000,
                    offered
                }
            );
            assert_eq!(*host.asked.borrow(), ["release", "release.sig"]);
            assert!(runs.0.borrow().is_empty());
        }
    }

    #[test]
    fn another_root_a_dark_host_a_changed_file_no_root_or_root_replace_nothing() {
        let dir = TempDir::new().unwrap();
        let layout = a_layout(&dir);
        receipt(&layout, 1_000);
        let key = root().public_key();
        let runs = Runs::new();
        let upgrade = |host: &Host, key: Option<&PublicKey>, euid: u32| {
            self_upgrade(&layout, key, euid, &host.get(), &runs.install())
        };

        let other = SigningKey::from_seed(&[0x72; 32]).unwrap();
        let e = upgrade(&Host::serving(served(2_000, &other)), Some(&key), 1000).unwrap_err();
        assert!(
            matches!(e, Error::Trust(trust::Error::OtherReleaseRoot { .. })),
            "{e}"
        );

        let mut dark = Host::serving(served(2_000, &root()));
        dark.down = true;
        let e = upgrade(&dark, Some(&key), 1000).unwrap_err();
        assert!(matches!(e, Error::Fetch(_)), "{e}");

        for (changed, what) in [
            ("install.sh", "install.sh"),
            ("odk-x86_64-linux.tar.gz", "tarball"),
        ] {
            let mut files = served(2_000, &root());
            let file = files.iter_mut().find(|(n, _)| n == changed).unwrap();
            file.1[0] ^= 1;
            let e = upgrade(&Host::serving(files), Some(&key), 1000).unwrap_err();
            assert!(matches!(e, Error::Digest(name) if name == what), "{e}");
        }

        let host = Host::serving(served(2_000, &root()));
        assert!(matches!(
            upgrade(&host, None, 1000),
            Err(Error::NoReleaseRoot)
        ));
        assert!(matches!(upgrade(&host, Some(&key), 0), Err(Error::AsRoot)));
        assert!(host.asked.borrow().is_empty(), "nothing is fetched first");

        assert!(runs.0.borrow().is_empty(), "no install.sh ran");
        assert_eq!(installed(&layout).unwrap(), Some(1_000));
    }

    #[test]
    fn a_receipt_that_names_no_version_is_refused() {
        let dir = TempDir::new().unwrap();
        let layout = a_layout(&dir);
        let path = layout.receipt_file();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "version = \"soon\"\n").unwrap();
        assert!(matches!(installed(&layout), Err(Error::Receipt { .. })));
        assert_eq!(installed(&layout_of_nothing()).unwrap(), None);

        fn layout_of_nothing() -> Layout {
            Layout::new("/nonexistent/home", "/nonexistent/data")
        }
    }
}
