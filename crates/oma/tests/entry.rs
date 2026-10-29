//! The entry the kernel uses for a `.oma` file: one path argument. The file is inert (ADR 0060).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn odk_oma() -> Command {
    Command::new(env!("CARGO_BIN_EXE_odk-oma"))
}

#[test]
fn a_single_file_is_refused_and_nothing_in_it_runs() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let file = dir.path().join("App.oma");
    // The magic of an envelope, then a program that would leave a mark if anything ran it.
    fs::write(
        &file,
        format!("OMA1\n#!/bin/sh\ntouch {}\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    let before = fs::read(&file).unwrap();

    let out = odk_oma().arg(&file).env_clear().output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("odk oma: "), "{err}");
    assert!(err.contains("App.oma"), "{err}");
    assert!(out.stdout.is_empty());
    assert!(!marker.exists(), "no byte of the file was executed");
    assert_eq!(fs::read(&file).unwrap(), before, "and it was not changed");
}

#[test]
fn a_file_named_like_a_subcommand_does_not_take_its_place() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("run"), "OMA1").unwrap();
    fs::write(dir.path().join("inspect"), "OMA1").unwrap();
    for sub in ["run", "inspect"] {
        let out = odk_oma()
            .arg(sub)
            .current_dir(dir.path())
            .env_clear()
            .output()
            .unwrap();
        // clap's own usage error for the missing argument, not the refusal of a file.
        assert_eq!(out.status.code(), Some(2), "{sub}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("Usage: odk oma"), "{err}");
        assert!(!err.contains("single-file"), "{err}");
    }
}

#[test]
fn a_path_that_is_not_a_file_is_an_unknown_command_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let out = odk_oma().arg(dir.path()).env_clear().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("single-file"));
}

#[test]
fn the_command_is_called_odk_oma_in_its_help_and_version() {
    let out = odk_oma().arg("--version").env_clear().output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("odk oma "));
    let out = odk_oma().arg("--help").env_clear().output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("Usage: odk oma"));
}

/// The image of a Bundle whose `manifest.toml` matches [`self_signed`]'s manifest, made by
/// `mkfs.erofs` (see `src/image/tests.rs`). Its first 1024 bytes are the prefix's.
const IMAGE: &[u8] = include_bytes!("../src/image/testdata/notes-mkfs-1.9.4.erofs");

/// A Bundle file whose envelope holds: signed by its own release key, with both digests right.
fn self_signed(payload: &[u8]) -> Vec<u8> {
    use oma::trust::{Publisher, SignedManifest, SigningKey, Timestamp};
    // A fixture key, never a real one.
    let key = SigningKey::from_seed(&[0x42; 32]).unwrap();
    let manifest = SignedManifest {
        app_id: "dev.csfh.notes".parse().unwrap(),
        version: 3,
        entry: "bin/notes".to_owned(),
        payload_sha256: [0; 32],
        header_sha256: [0; 32],
        key_id: key.public_key().key_id(),
        root_key_id: "fedcba9876543210".parse().unwrap(),
        issued_at: Timestamp::parse("2026-10-27T00:00:00Z").unwrap(),
        expires_at: Timestamp::parse("2027-10-27T00:00:00Z").unwrap(),
        update_url: "https://apps.csfh.dev/notes.oma".to_owned(),
        publisher: Publisher::parse("apps.csfh.dev").unwrap(),
    };
    oma::trust::stamp(payload, manifest, &key).unwrap()
}

#[test]
fn a_file_whose_envelope_and_image_hold_is_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Notes.oma");
    fs::write(&file, self_signed(&IMAGE[1024..])).unwrap();

    let out = odk_oma().arg(&file).env_clear().output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("odk oma: "), "{err}");
    assert!(
        err.contains("dev.csfh.notes 3 is signed by release key")
            && err.contains("its image matches it"),
        "{err}"
    );
    assert!(err.contains("not run yet"), "{err}");
    assert!(
        !err.contains("trusted") && !err.contains("verified"),
        "{err}"
    );
}

#[test]
fn a_signed_payload_that_is_not_an_image_is_refused_and_nothing_in_it_runs() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let file = dir.path().join("Notes.oma");
    // A program that would leave a mark if anything ran it.
    let payload = format!("#!/bin/sh\ntouch {}\n", marker.display());
    fs::write(&file, self_signed(payload.as_bytes())).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    let before = fs::read(&file).unwrap();

    let out = odk_oma().arg(&file).env_clear().output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.starts_with("odk oma: "), "{err}");
    assert!(
        err.contains("the image is not EROFS") && err.contains("never run"),
        "{err}"
    );
    assert!(
        !err.contains("trusted") && !err.contains("verified"),
        "{err}"
    );
    assert!(!marker.exists(), "no byte of the file was executed");
    assert_eq!(fs::read(&file).unwrap(), before, "and it was not changed");
}

#[test]
fn a_file_whose_envelope_does_not_hold_says_why_and_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Notes.oma");
    let mut bytes = self_signed(b"#!/bin/sh\nexit 0\n");
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&file, bytes).unwrap();

    let out = odk_oma().arg(&file).env_clear().output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("the payload digest does not match the manifest"),
        "{err}"
    );
    assert!(
        err.contains("a file that does not verify is never run"),
        "{err}"
    );
}
