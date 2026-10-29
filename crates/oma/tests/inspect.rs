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

/// A Bundle file, signed by `odk-oma bundle --sign` with a fixture seed, inspected in a home of
/// its own where nothing is pinned.
#[test]
fn a_bundle_file_shows_its_signature_before_its_permissions() {
    let dir = bundle(
        "manifest_version = 1\nidentifier = \"com.example.notes\"\nname = \"Notes\"\nversion = \"1.4.0\"\nexecutable = \"app\"\npublisher = \"example.com\"\n\n[permissions]\ngpu = true\n",
    );
    let home = TempDir::new().unwrap();
    // A fixture key, never a real one.
    let seed = home.path().join("release.seed");
    fs::write(&seed, [0x42; 32]).unwrap();
    let odk_oma = |args: &[&std::ffi::OsStr]| {
        Command::new(env!("CARGO_BIN_EXE_odk-oma"))
            .args(args)
            .env_clear()
            .env("HOME", home.path())
            .env("XDG_DATA_HOME", home.path().join("data"))
            .output()
            .unwrap()
    };
    let out = home.path().join("out");
    let made = odk_oma(&[
        "bundle".as_ref(),
        dir.path().join("app").as_os_str(),
        "--manifest".as_ref(),
        dir.path().join("manifest.toml").as_os_str(),
        "--sign".as_ref(),
        seed.as_os_str(),
        "--root".as_ref(),
        "0123456789abcdef".as_ref(),
        "--release".as_ref(),
        "3".as_ref(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let file = out.join("Notes.oma");

    let shown = odk_oma(&["inspect".as_ref(), file.as_os_str()]);
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    assert_eq!(
        String::from_utf8(shown.stdout).unwrap(),
        "Notes 1.4.0\ncom.example.notes\n\nSigned:\n  version 3\n  publisher example.com\n  publisher key 0123456789abcdef, not pinned\n\nPermissions:\n  Can use the graphics card directly\n"
    );

    // One byte of the image changed: it does not verify, and nothing is printed.
    let mut bytes = fs::read(&file).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&file, bytes).unwrap();
    let refused = odk_oma(&["inspect".as_ref(), file.as_os_str()]);
    assert!(!refused.status.success());
    assert!(refused.stdout.is_empty());
    assert!(String::from_utf8_lossy(&refused.stderr).starts_with("odk oma: "));
}
