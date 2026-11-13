//! Moving a directory to the user's Trash, as the freedesktop.org Trash specification (1.0) lays
//! it out, so a file manager shows it and can restore it (ADR 0099). Only the home trash, under
//! `$XDG_DATA_HOME`, is used: what `odk-oma` moves there is under `$XDG_DATA_HOME` too, so the
//! move is one `rename` on one filesystem and nothing is copied.

use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Moves `path` into `trash` (`$XDG_DATA_HOME/Trash`) under `name`, or `name 2`, `name 3` and so
/// on when it is taken, with a `.trashinfo` that names where it was and when, `deleted_at` in
/// Unix seconds, written as local time. Returns where it is now. On a failure nothing is moved,
/// and no `.trashinfo` is left behind.
pub fn move_to(trash: &Path, path: &Path, name: &str, deleted_at: i64) -> io::Result<PathBuf> {
    let (files, info) = (trash.join("files"), trash.join("info"));
    for dir in [trash, &files, &info] {
        match fs::DirBuilder::new().mode(0o700).create(dir) {
            Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
            _ => {}
        }
    }
    let text = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        encode(path),
        local_time(deleted_at)
    );
    for n in 1..1000u32 {
        let candidate = if n == 1 {
            name.to_owned()
        } else {
            format!("{name} {n}")
        };
        let info_file = info.join(format!("{candidate}.trashinfo"));
        // The spec's way to take a name: the `.trashinfo` is created exclusively first.
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&info_file)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let target = files.join(&candidate);
        if target.symlink_metadata().is_ok() {
            drop(file);
            let _ = fs::remove_file(&info_file);
            continue;
        }
        let moved = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(path, &target));
        if let Err(e) = moved {
            let _ = fs::remove_file(&info_file);
            return Err(e);
        }
        return Ok(target);
    }
    Err(io::Error::other(format!(
        "no free name for {name} in {}",
        files.display()
    )))
}

/// An absolute path as the spec's `Path=` key takes it: percent-encoded as in a URL path, with
/// `/` kept.
fn encode(path: &Path) -> String {
    let mut out = String::new();
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `seconds` since the epoch as local time, `YYYY-MM-DDThh:mm:ss`, which is what the spec's
/// `DeletionDate=` holds.
fn local_time(seconds: i64) -> String {
    // SAFETY: `localtime_r` fills the `tm` it is given, and `time_t` is `i64` on x86_64.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t: libc::time_t = seconds;
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return "1970-01-01T00:00:00".to_owned();
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn a_directory_goes_to_files_with_its_trashinfo_and_a_second_takes_the_next_name() {
        let dir = TempDir::new().unwrap();
        let trash = dir.path().join("Trash");
        let data = dir.path().join("my data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("notes.db"), "notes").unwrap();

        let moved = move_to(&trash, &data, "com.example.notes data", 0).unwrap();
        assert_eq!(moved, trash.join("files/com.example.notes data"));
        assert_eq!(fs::read_to_string(moved.join("notes.db")).unwrap(), "notes");
        assert!(!data.exists());
        let info = fs::read_to_string(trash.join("info/com.example.notes data.trashinfo")).unwrap();
        let lines: Vec<_> = info.lines().collect();
        assert_eq!(lines[0], "[Trash Info]");
        assert_eq!(lines[1], format!("Path={}", encode(&data)));
        assert!(lines[1].ends_with("/my%20data"), "{info}");
        assert!(lines[2].starts_with("DeletionDate=19"), "{info}");

        fs::create_dir_all(&data).unwrap();
        let again = move_to(&trash, &data, "com.example.notes data", 0).unwrap();
        assert_eq!(again, trash.join("files/com.example.notes data 2"));
        assert!(
            trash
                .join("info/com.example.notes data 2.trashinfo")
                .is_file()
        );
    }

    #[test]
    fn a_move_that_fails_leaves_the_directory_and_no_trashinfo() {
        let dir = TempDir::new().unwrap();
        let trash = dir.path().join("Trash");
        let missing = dir.path().join("not there");
        assert!(move_to(&trash, &missing, "x", 0).is_err());
        assert_eq!(fs::read_dir(trash.join("info")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(trash.join("files")).unwrap().count(), 0);
    }
}
