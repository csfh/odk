//! The Bundle files in `~/Applications` (ADR 0009, ADR 0052): placing one, and finding them.
//! What is installed, and the consent, are `flow`'s (ADR 0033). The Bundle directories from
//! before v1 are only found, to be reported and purged (ADR 0099).

use std::fs;
use std::path::{Path, PathBuf};

use crate::fmt::{self, Identifier};
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
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
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

/// The Bundle directories from before v1 in `~/Applications` (ADR 0099), by path, with the
/// Identifier their `manifest.toml` names when it can be read. They are found to be reported
/// and purged, never opened as a Bundle or run, so only the `identifier` key is read and the rest
/// of the Manifest is not validated.
pub fn directories(layout: &Layout) -> Vec<(PathBuf, Option<Identifier>)> {
    let Ok(entries) = fs::read_dir(layout.applications()) else {
        return Vec::new();
    };
    let names = |path: &Path| -> Option<Identifier> {
        let text = fs::read_to_string(path.join("manifest.toml")).ok()?;
        let table: toml::Table = text.parse().ok()?;
        table.get("identifier")?.as_str()?.parse().ok()
    };
    let mut found: Vec<_> = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.ends_with(".oma") && !n.starts_with('.'))
        })
        .map(|e| e.path())
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
        .map(|p| {
            let id = names(&p);
            (p, id)
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// The Bundle directory from before v1 whose `manifest.toml` names `id`, if there is one.
pub fn find_directory(layout: &Layout, id: &Identifier) -> Option<PathBuf> {
    directories(layout)
        .into_iter()
        .find(|(_, named)| named.as_ref() == Some(id))
        .map(|(path, _)| path)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn layout(dir: &TempDir) -> Layout {
        Layout::new(
            dir.path().join("home"),
            dir.path().join("home/.local/share"),
        )
    }

    #[test]
    fn a_directory_from_before_v1_is_found_by_its_identifier_without_being_validated() {
        let dir = TempDir::new().unwrap();
        let layout = layout(&dir);
        let apps = layout.applications();
        // Only the identifier is read: the rest would not pass today's Manifest.
        fs::create_dir_all(apps.join("Old.oma")).unwrap();
        fs::write(
            apps.join("Old.oma/manifest.toml"),
            "identifier = \"com.example.old\"\nwhatever = 1\n",
        )
        .unwrap();
        fs::create_dir_all(apps.join("Unnamed.oma")).unwrap();
        fs::create_dir_all(apps.join(".Hidden.oma")).unwrap();
        fs::write(apps.join("File.oma"), "not a directory").unwrap();

        let found = directories(&layout);
        assert_eq!(
            found,
            vec![
                (
                    apps.join("Old.oma"),
                    Some("com.example.old".parse().unwrap())
                ),
                (apps.join("Unnamed.oma"), None),
            ]
        );
        assert_eq!(
            find_directory(&layout, &"com.example.old".parse().unwrap()),
            Some(apps.join("Old.oma"))
        );
        assert_eq!(
            find_directory(&layout, &"com.example.other".parse().unwrap()),
            None
        );
    }
}
