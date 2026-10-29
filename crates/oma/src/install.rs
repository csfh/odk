//! Installing a Bundle into `~/Applications` (ADR 0009, ADR 0018).
//!
//! Minimal for now: it writes no `.desktop` entry, refuses an Identifier that is already
//! installed (no upgrades, no previous version; only [`reinstall`], for `odk oma dev`, replaces one,
//! ADR 0051), and records consent only with `accept_permissions`: the dialog is the caller's
//! (`flow`, ADR 0033).

use std::fs;
use std::path::{Path, PathBuf};

use crate::fmt::{self, Bundle, Identifier};
use crate::grants;
use crate::layout::Layout;
use crate::trust::{Sealed, SignedManifest};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Invalid(#[from] fmt::Error),
    #[error(transparent)]
    Grants(#[from] grants::Error),
    #[error("{identifier} is already installed at {}; upgrades are not supported yet", path.display())]
    AlreadyInstalled {
        identifier: Identifier,
        path: PathBuf,
    },
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    #[error("it asks for permissions that have not been granted:\n{}\nre-run with --accept-permissions to grant exactly these", lines(.0))]
    ConsentRequired(Vec<String>),
    #[error("{} contains something that is neither a file, a directory nor a symlink", .0.display())]
    Unsupported(PathBuf),
}

fn lines(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// Writes a Bundle file's sealed bytes to `target` in `~/Applications`, which must not exist
/// (ADR 0052, ADR 0060 section 5): the bytes that verified, never the source path again. They go
/// to a hidden name first, without `.oma`, so nothing that watches the directory sees half a
/// file. They are then linked in, which never replaces anything, so neither a directory Bundle
/// nor another file is ever removed or renamed. The file is `0644`: a Bundle file is inert.
pub fn place_file(file: &Sealed, target: &Path) -> Result<(), Error> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let dir = target.parent().expect("a file in ~/Applications");
    fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    let name = target.file_name().expect("a file name").to_string_lossy();
    let partial = dir.join(format!(".{name}.{}.partial", std::process::id()));
    let written = (|| {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&partial)?;
        let mut buf = vec![0u8; 1 << 16];
        let mut at = 0;
        while at < file.len() {
            let n = (file.len() - at).min(buf.len() as u64) as usize;
            file.read_exact_at(&mut buf[..n], at)?;
            out.write_all(&buf[..n])?;
            at += n as u64;
        }
        out.sync_all()
    })();
    let linked = written.and_then(|()| fs::hard_link(&partial, target));
    let _ = fs::remove_file(&partial);
    match linked {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::Exists(target.to_owned()));
        }
        Err(e) => return Err(io(target, e)),
    }
    fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| io(dir, e))
}

/// Every regular file in `~/Applications` named `*.oma`, by name, with what its prefix says when
/// the prefix parses. Nothing here is verified: it is for finding files, and what is run or
/// installed is checked in full (`flow`).
pub fn bundle_files(layout: &Layout) -> Vec<(PathBuf, Option<SignedManifest>)> {
    use crate::trust::{Prefix, container::PREFIX_LEN};
    use std::io::Read;
    let says = |path: &Path| -> Option<SignedManifest> {
        let mut file = fs::File::open(path).ok()?;
        let len = file.metadata().ok()?.len();
        let mut prefix = [0u8; PREFIX_LEN];
        file.read_exact(&mut prefix).ok()?;
        let prefix = Prefix::parse(&prefix, len).ok()?;
        SignedManifest::parse(&prefix.manifest).ok()
    };
    let Ok(entries) = fs::read_dir(layout.applications()) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "oma"))
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_file()))
        .map(|p| {
            let signed = says(&p);
            (p, signed)
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// A Bundle file in `~/Applications` whose signed manifest names `id`, if there is one.
pub fn find_file(layout: &Layout, id: &Identifier) -> Option<PathBuf> {
    bundle_files(layout)
        .into_iter()
        .find(|(_, signed)| signed.as_ref().is_some_and(|s| s.app_id == *id))
        .map(|(path, _)| path)
}

/// Every valid Bundle in `~/Applications`. Directories that are not valid Bundles are skipped.
pub fn installed(layout: &Layout) -> Vec<Bundle> {
    let Ok(entries) = fs::read_dir(layout.applications()) else {
        return Vec::new();
    };
    let mut found: Vec<Bundle> = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.ends_with(".oma") && !n.starts_with('.'))
        })
        .filter_map(|e| Bundle::open(&e.path()).ok())
        .collect();
    found.sort_by(|a, b| a.root().cmp(b.root()));
    found
}

/// The installed Bundle with this Identifier.
pub fn find(layout: &Layout, id: &Identifier) -> Option<Bundle> {
    installed(layout)
        .into_iter()
        .find(|b| &b.manifest().identifier == id)
}

/// Copies the Bundle at `source` into `~/Applications` and returns its new path. The original
/// is left alone. If the Bundle asks for permissions its consent record lacks, nothing is
/// installed unless `accept_permissions` is set, which records exactly what it asks for now.
pub fn install(layout: &Layout, source: &Path, accept_permissions: bool) -> Result<PathBuf, Error> {
    let bundle = Bundle::open(source)?;
    let manifest = bundle.manifest();
    if let Some(existing) = find(layout, &manifest.identifier) {
        return Err(Error::AlreadyInstalled {
            identifier: manifest.identifier.clone(),
            path: existing.root().to_owned(),
        });
    }

    let granted = grants::load(layout, &manifest.identifier)?.unwrap_or_default();
    let missing = grants::missing(&granted, &manifest.permissions);
    if !grants::is_empty(&missing) && !accept_permissions {
        return Err(Error::ConsentRequired(fmt::permission_lines(&missing)));
    }

    let applications = layout.applications();
    fs::create_dir_all(&applications).map_err(|e| io(&applications, e))?;
    let name = format!("{}.oma", manifest.name.replace('/', "-"));
    let target = applications.join(&name);
    if target.exists() {
        return Err(Error::Exists(target));
    }
    let staging = applications.join(format!(".{name}.partial-{}", std::process::id()));
    fs::create_dir(&staging).map_err(|e| io(&staging, e))?;
    let copied = copy_tree(bundle.root(), &staging)
        .and_then(|()| Bundle::open(&staging).map(drop).map_err(Error::from))
        .and_then(|()| fs::rename(&staging, &target).map_err(|e| io(&target, e)));
    if let Err(e) = copied {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    if accept_permissions {
        grants::save(layout, &manifest.identifier, &manifest.permissions)?;
    }
    Ok(target)
}

/// [`install`] that replaces the installed Bundle with the same Identifier, for `odk oma dev`
/// (ADR 0051); `odk oma install` still refuses one. The old copy is moved aside under a name that
/// [`installed`] skips and put back if the new one cannot be installed, so a failure leaves what
/// was there. Returns the new path and whether there was a copy to replace.
pub fn reinstall(
    layout: &Layout,
    source: &Path,
    accept_permissions: bool,
) -> Result<(PathBuf, bool), Error> {
    let bundle = Bundle::open(source)?;
    let Some(old) = find(layout, &bundle.manifest().identifier) else {
        return Ok((install(layout, source, accept_permissions)?, false));
    };
    let old = old.root().to_owned();
    let name = old.file_name().map(|n| n.to_string_lossy().into_owned());
    let aside = layout.applications().join(format!(
        ".{}.replaced-{}",
        name.as_deref().unwrap_or("bundle"),
        std::process::id()
    ));
    fs::rename(&old, &aside).map_err(|e| io(&old, e))?;
    match install(layout, source, accept_permissions) {
        Ok(path) => {
            let _ = fs::remove_dir_all(&aside);
            Ok((path, true))
        }
        Err(e) => {
            let _ = fs::rename(&aside, &old);
            Err(e)
        }
    }
}

/// Copies a directory tree, keeping symlinks as symlinks and file modes as they are.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Error> {
    for entry in fs::read_dir(from).map_err(|e| io(from, e))? {
        let entry = entry.map_err(|e| io(from, e))?;
        let (source, destination) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().map_err(|e| io(&source, e))?;
        if kind.is_symlink() {
            let target = fs::read_link(&source).map_err(|e| io(&source, e))?;
            std::os::unix::fs::symlink(target, &destination).map_err(|e| io(&destination, e))?;
        } else if kind.is_dir() {
            let mode = fs::metadata(&source)
                .map_err(|e| io(&source, e))?
                .permissions();
            fs::create_dir(&destination).map_err(|e| io(&destination, e))?;
            copy_tree(&source, &destination)?;
            fs::set_permissions(&destination, mode).map_err(|e| io(&destination, e))?;
        } else if kind.is_file() {
            fs::copy(&source, &destination).map_err(|e| io(&source, e))?;
        } else {
            return Err(Error::Unsupported(source));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use tempfile::TempDir;

    use super::*;
    use crate::testutil::bundle;

    fn layout(dir: &TempDir) -> Layout {
        Layout::new(
            dir.path().join("home"),
            dir.path().join("home/.local/share"),
        )
    }

    #[test]
    fn installs_a_bundle_without_permissions_with_no_consent_needed() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let source = bundle(dir.path(), "Tee", "com.example.t", "1.0.0", "");
        let installed_path = install(&layout, &source, false).unwrap();
        assert_eq!(installed_path, layout.applications().join("Tee.oma"));
        assert!(source.exists(), "the original is left alone");
        assert!(Bundle::open(&installed_path).is_ok());
        assert_eq!(
            grants::load(&layout, &"com.example.t".parse().unwrap()).unwrap(),
            None,
            "no record when nothing was asked"
        );
        assert_eq!(
            find(&layout, &"com.example.t".parse().unwrap())
                .unwrap()
                .root(),
            fs::canonicalize(&installed_path).unwrap()
        );
    }

    #[test]
    fn a_bundle_that_asks_for_permissions_needs_consent() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let source = bundle(dir.path(), "Tee", "com.example.t", "1.0.0", "gpu = true");
        let err = install(&layout, &source, false).unwrap_err();
        assert!(
            matches!(&err, Error::ConsentRequired(l) if l == &["Can use the graphics card directly"]),
            "{err}"
        );
        assert!(
            !layout.applications().join("Tee.oma").exists(),
            "nothing is installed"
        );
        assert!(installed(&layout).is_empty());
    }

    #[test]
    fn accepting_records_exactly_what_is_asked() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let source = bundle(
            dir.path(),
            "Tee",
            "com.example.t",
            "1.0.0",
            "gpu = true\naudio = true",
        );
        install(&layout, &source, true).unwrap();
        let id: Identifier = "com.example.t".parse().unwrap();
        let granted = grants::load(&layout, &id).unwrap().unwrap();
        assert!(granted.gpu && granted.audio && !granted.network);
    }

    #[test]
    fn refuses_an_identifier_that_is_already_installed() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let first = bundle(&dir.path().join("a"), "Tee", "com.example.t", "1.0.0", "");
        install(&layout, &first, false).unwrap();
        let newer = bundle(
            &dir.path().join("b"),
            "Renamed",
            "com.example.t",
            "2.0.0",
            "",
        );
        assert!(matches!(
            install(&layout, &newer, false),
            Err(Error::AlreadyInstalled { .. })
        ));
        assert!(!layout.applications().join("Renamed.oma").exists());
    }

    #[test]
    fn refuses_a_different_application_with_the_same_name() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        install(
            &layout,
            &bundle(&dir.path().join("a"), "Tee", "com.example.one", "1.0.0", ""),
            false,
        )
        .unwrap();
        let other = bundle(&dir.path().join("b"), "Tee", "com.example.two", "1.0.0", "");
        assert!(matches!(
            install(&layout, &other, false),
            Err(Error::Exists(_))
        ));
    }

    #[test]
    fn reinstall_replaces_the_copy_and_puts_it_back_when_the_new_one_cannot_go_in() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let old = bundle(&dir.path().join("a"), "Tee", "com.example.t", "1.0.0", "");
        install(&layout, &old, false).unwrap();

        // Asks for a permission that was never granted, and consent is not given: the old
        // copy stays, under its own name, and nothing is left beside it.
        let wants = bundle(
            &dir.path().join("b"),
            "Tee",
            "com.example.t",
            "2.0.0",
            "audio = true",
        );
        assert!(matches!(
            reinstall(&layout, &wants, false),
            Err(Error::ConsentRequired(_))
        ));
        let id = "com.example.t".parse().unwrap();
        assert_eq!(
            find(&layout, &id).unwrap().manifest().version.to_string(),
            "1.0.0"
        );
        assert_eq!(fs::read_dir(layout.applications()).unwrap().count(), 1);

        let (path, replaced) = reinstall(&layout, &wants, true).unwrap();
        assert!(replaced);
        assert_eq!(path, layout.applications().join("Tee.oma"));
        assert_eq!(
            find(&layout, &id).unwrap().manifest().version.to_string(),
            "2.0.0"
        );
        assert_eq!(fs::read_dir(layout.applications()).unwrap().count(), 1);
    }

    #[test]
    fn an_invalid_bundle_installs_nothing() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let source = bundle(dir.path(), "Tee", "com.example.t", "1.0.0", "");
        fs::write(source.join("bin/app"), "#!/bin/sh\n").unwrap();
        assert!(matches!(
            install(&layout, &source, false),
            Err(Error::Invalid(_))
        ));
        assert!(
            !layout.applications().exists()
                || fs::read_dir(layout.applications()).unwrap().count() == 0
        );
    }

    #[test]
    fn copies_resources_keeping_symlinks_and_modes() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let source = bundle(dir.path(), "Tee", "com.example.t", "1.0.0", "");
        fs::create_dir_all(source.join("share/sub")).unwrap();
        fs::write(source.join("share/sub/data.txt"), "x").unwrap();
        symlink("data.txt", source.join("share/sub/link")).unwrap();
        fs::set_permissions(
            source.join("share/sub/data.txt"),
            fs::Permissions::from_mode(0o640),
        )
        .unwrap();

        let target = install(&layout, &source, false).unwrap();

        assert_eq!(
            fs::read_to_string(target.join("share/sub/data.txt")).unwrap(),
            "x"
        );
        assert_eq!(
            fs::read_link(target.join("share/sub/link")).unwrap(),
            Path::new("data.txt")
        );
        assert_eq!(
            fs::metadata(target.join("share/sub/data.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
        assert_eq!(
            fs::metadata(target.join("bin/app"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[test]
    fn leaves_no_staging_directory_behind() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        install(
            &layout,
            &bundle(dir.path(), "Tee", "com.example.t", "1.0.0", ""),
            false,
        )
        .unwrap();
        let names: Vec<_> = fs::read_dir(layout.applications())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["Tee.oma"]);
    }
}
