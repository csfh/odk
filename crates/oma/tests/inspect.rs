use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use tempfile::TempDir;

fn bundle(manifest: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("manifest.toml"), manifest).unwrap();
    let exe = dir.path().join("app");
    let mut header = vec![0u8; 64];
    header[..4].copy_from_slice(b"\x7fELF");
    header[4] = 2;
    header[5] = 1;
    header[16] = 3;
    header[18] = 62;
    fs::write(&exe, header).unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn inspect(dir: &TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_odk-oma"))
        .arg("inspect")
        .arg(dir.path())
        .output()
        .unwrap()
}

#[test]
fn prints_name_identifier_version_and_permissions() {
    let dir = bundle(
        "manifest_version = 1\nidentifier = \"com.example.notes\"\nname = \"Notes\"\nversion = \"1.4.0\"\nexecutable = \"app\"\n\n[permissions]\ngpu = true\n",
    );
    let out = inspect(&dir);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Notes 1.4.0\ncom.example.notes\n\nPermissions:\n  Can use the graphics card directly\n"
    );
}

#[test]
fn an_invalid_bundle_fails_with_a_message() {
    let dir = bundle("manifest_version = 2\n");
    let out = inspect(&dir);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("odk oma: "));
}
