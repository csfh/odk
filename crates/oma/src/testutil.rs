//! Helpers for building throwaway Bundles in tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn elf() -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(b"\x7fELF");
    h[4] = 2;
    h[5] = 1;
    h[16] = 3;
    h[18] = 62;
    h
}

/// Writes a valid Bundle directory `<parent>/<name>.oma` and returns its path.
/// `permissions` is the text under `[permissions]`.
pub fn bundle(
    parent: &Path,
    name: &str,
    identifier: &str,
    version: &str,
    permissions: &str,
) -> PathBuf {
    let dir = parent.join(format!("{name}.oma"));
    fs::create_dir_all(dir.join("bin")).unwrap();
    fs::write(
        dir.join("manifest.toml"),
        format!(
            "manifest_version = 1\nidentifier = \"{identifier}\"\nname = \"{name}\"\nversion = \"{version}\"\nexecutable = \"bin/app\"\n\n[permissions]\n{permissions}\n"
        ),
    )
    .unwrap();
    let exe = dir.join("bin/app");
    fs::write(&exe, elf()).unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

/// Whether this process is the child started by [`in_child`].
pub fn is_child() -> bool {
    std::env::var_os("OMA_TEST_CHILD").is_some()
}

/// Runs the named test again in a fresh process and asserts it succeeds. Tests that restrict
/// the calling process (seccomp, Landlock) cannot undo it, so they must not run in the
/// harness. The test body returns straight away unless [`is_child`].
pub fn in_child(test_name: &str) {
    if is_child() {
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env("OMA_TEST_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
