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
