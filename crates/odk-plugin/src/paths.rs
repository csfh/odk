//! The one rule for a path a Plugin names (`docs/spec/plugin-manifest.md`, `entry`; ADR 0092,
//! `require` and `storage`): relative, with no `..`, and no symlink leaving the directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `rel` as a path of plain components, or `None` if it is empty, absolute, holds `..`, `.` or an
/// empty segment, or a character no file name may hold.
pub(crate) fn clean_relative(rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.contains('\0') || rel.contains('\\') {
        return None;
    }
    // Judged on the text, since `Path` quietly drops a `.` or a doubled `/` inside a path, and a
    // person reading a manifest would call each of those a different path.
    let mut clean = PathBuf::new();
    for segment in rel.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        clean.push(segment);
    }
    Some(clean)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PathError {
    #[error("{0:?} is not a path inside the Plugin's directory")]
    Outside(String),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// The file `rel` names under `root`, which must exist and, once symlinks are followed, still
/// be inside `root`.
pub(crate) fn resolve_existing(root: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let clean = clean_relative(rel).ok_or_else(|| PathError::Outside(rel.to_owned()))?;
    let root = fs::canonicalize(root)?;
    let found = fs::canonicalize(root.join(clean))?;
    if found.starts_with(&root) {
        Ok(found)
    } else {
        Err(PathError::Outside(rel.to_owned()))
    }
}

/// Where `rel` would be under `root`, for a file about to be written: the deepest part that
/// exists must be inside `root` once symlinks are followed, and the file itself must not be a
/// symlink. Nothing is created.
pub(crate) fn resolve_new(root: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let clean = clean_relative(rel).ok_or_else(|| PathError::Outside(rel.to_owned()))?;
    let root = fs::canonicalize(root)?;
    let target = root.join(&clean);
    let mut ancestor = target.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| PathError::Outside(rel.to_owned()))?;
    }
    if !fs::canonicalize(ancestor)?.starts_with(&root) {
        return Err(PathError::Outside(rel.to_owned()));
    }
    if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(PathError::Outside(rel.to_owned()));
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_relative_paths_are_clean() {
        assert_eq!(clean_relative("a.luau"), Some(PathBuf::from("a.luau")));
        assert_eq!(
            clean_relative("a/b/c.luau"),
            Some(PathBuf::from("a/b/c.luau"))
        );
    }

    #[test]
    fn everything_else_is_refused() {
        for bad in [
            "", "/a", "../a", "a/../b", "./a", "a/./b", "a//b", "a/", "a\\b", "a\0b", ".",
        ] {
            assert_eq!(clean_relative(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_symlink_out_of_the_directory_is_refused_for_reading_and_writing() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("file-link"))
            .unwrap();
        assert!(matches!(
            resolve_existing(dir.path(), "link/secret"),
            Err(PathError::Outside(_))
        ));
        assert!(matches!(
            resolve_existing(dir.path(), "file-link"),
            Err(PathError::Outside(_))
        ));
        assert!(matches!(
            resolve_new(dir.path(), "link/new"),
            Err(PathError::Outside(_))
        ));
        assert!(matches!(
            resolve_new(dir.path(), "file-link"),
            Err(PathError::Outside(_))
        ));
    }

    #[test]
    fn a_dangling_symlink_is_not_a_way_to_create_a_file_outside() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("not-there-yet");
        std::os::unix::fs::symlink(&target, dir.path().join("dangling")).unwrap();
        assert!(!target.exists());
        assert!(matches!(
            resolve_new(dir.path(), "dangling"),
            Err(PathError::Outside(_))
        ));
    }

    #[test]
    fn a_new_file_under_a_new_directory_is_inside() {
        let dir = tempfile::tempdir().unwrap();
        let got = resolve_new(dir.path(), "a/b/c.txt").unwrap();
        assert!(got.starts_with(fs::canonicalize(dir.path()).unwrap()));
        assert!(!got.exists(), "nothing is created");
    }
}
