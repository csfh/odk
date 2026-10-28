//! One version for every crate, the workspace's, and it is what `odk --version` prints (ADR 0064).
//! A release refuses a tag that says another (`.github/workflows/release.yml`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `version` of the root manifest's `[workspace.package]`.
fn workspace_version() -> String {
    let manifest = fs::read_to_string(repo().join("Cargo.toml")).unwrap();
    let package = manifest.split("[workspace.package]").nth(1).unwrap();
    let line = package
        .lines()
        .take_while(|l| !l.starts_with('['))
        .find_map(|l| l.strip_prefix("version = "))
        .expect("[workspace.package] has a version");
    line.trim().trim_matches('"').to_owned()
}

#[test]
fn every_crate_takes_the_workspaces_version() {
    for entry in fs::read_dir(repo().join("crates")).unwrap() {
        let manifest = entry.unwrap().path().join("Cargo.toml");
        let text = fs::read_to_string(&manifest).unwrap();
        assert!(
            text.lines().any(|l| l.trim() == "version.workspace = true"),
            "{} has a version of its own",
            manifest.display()
        );
    }
}

#[test]
fn odk_prints_the_workspaces_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_odk"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("odk {}\n", workspace_version())
    );
}
