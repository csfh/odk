//! `odk oma mount`: a Bundle file seen as a directory, through udisks (ADR 0065). No test sets up
//! a loop device: a fake `udisksctl` on `PATH` says what it was asked and refuses, as udisks does
//! outside the active session.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A Bundle file's first bytes: `OMA1`, then the EROFS magic at 1024.
fn bundle(dir: &Path) -> PathBuf {
    let mut bytes = vec![0u8; 4096];
    bytes[..4].copy_from_slice(b"OMA1");
    bytes[1024..1028].copy_from_slice(&0xE0F5_E1E2_u32.to_le_bytes());
    let path = dir.join("App.oma");
    fs::write(&path, bytes).unwrap();
    path
}

/// A directory with a `udisksctl` that records its arguments, one a line, and its `LC_ALL`, and
/// fails as udisks does for a caller polkit does not allow.
fn fake_udisksctl(dir: &Path) -> PathBuf {
    let bin = dir.join("fakes");
    fs::create_dir(&bin).unwrap();
    let script = bin.join("udisksctl");
    fs::write(
        &script,
        "#!/bin/sh\nd=${0%/*}\nprintf '%s\\n' \"$@\" > \"$d/argv\"\nprintf '%s\\n' \"$LC_ALL\" > \"$d/lc_all\"\necho 'Error setting up loop device for x: GDBus.Error:org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain: Not authorized to perform operation' >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn odk_oma(args: &[&Path], path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_odk-oma"))
        .args(args)
        .env_clear()
        .env("PATH", path)
        .output()
        .unwrap()
}

#[test]
fn it_asks_udisks_for_a_read_only_loop_device_without_a_password_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let file = bundle(dir.path());
    let fakes = fake_udisksctl(dir.path());

    let out = odk_oma(&[Path::new("mount"), &file], &fakes);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("odk oma: "), "{err}");
    assert!(err.contains("Not authorized"), "{err}");
    assert!(
        err.contains("only from the active desktop session"),
        "{err}"
    );
    assert!(out.stdout.is_empty());

    let argv = fs::read_to_string(fakes.join("argv")).unwrap();
    let expected = [
        "loop-setup",
        "--read-only",
        "--no-partition-scan",
        "--file",
        file.to_str().unwrap(),
        "--no-user-interaction",
    ];
    assert_eq!(argv.lines().collect::<Vec<_>>(), expected);
    assert_eq!(fs::read_to_string(fakes.join("lc_all")).unwrap(), "C\n");
}

#[test]
fn a_file_that_is_not_a_bundle_file_is_refused_before_udisks_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("disk.img");
    fs::write(&file, vec![0u8; 4096]).unwrap();
    let fakes = fake_udisksctl(dir.path());

    for args in [
        vec![Path::new("mount"), &file],
        vec![Path::new("mount"), Path::new("--unmount"), &file],
    ] {
        let out = odk_oma(&args, &fakes);
        assert_eq!(out.status.code(), Some(1));
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.starts_with("odk oma: "), "{err}");
        assert!(err.contains("is not a Bundle file"), "{err}");
    }
    let out = odk_oma(&[Path::new("mount"), dir.path()], &fakes);
    assert!(String::from_utf8_lossy(&out.stderr).contains("is a directory"));
    assert!(!fakes.join("argv").exists(), "udisks was not asked");
}

#[test]
fn without_udisks_it_names_the_package() {
    let dir = tempfile::tempdir().unwrap();
    let file = bundle(dir.path());
    let out = odk_oma(&[Path::new("mount"), &file], dir.path());
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("udisksctl is not installed: it is part of udisks2"),
        "{err}"
    );
}

#[test]
fn unmount_of_a_file_that_is_not_mounted_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let file = bundle(dir.path());
    let fakes = fake_udisksctl(dir.path());
    let out = odk_oma(&[Path::new("mount"), Path::new("--unmount"), &file], &fakes);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("App.oma is not mounted"), "{err}");
    assert!(!fakes.join("argv").exists());
}

#[test]
fn open_and_unmount_do_not_go_together() {
    let dir = tempfile::tempdir().unwrap();
    let file = bundle(dir.path());
    let out = odk_oma(
        &[
            Path::new("mount"),
            Path::new("--open"),
            Path::new("--unmount"),
            &file,
        ],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(2));
}
