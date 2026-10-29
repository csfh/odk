//! `odk oma bundle`: one signed Bundle file (ADR 0052, ADR 0060, ADR 0067).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use oma::trust::{Sealed, SigningKey};
use tempfile::TempDir;

fn oma(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_odk-oma"))
        .args(args)
        .output()
        .unwrap()
}

/// A project with a built program and a manifest, and the fixture keys to sign it with.
fn a_project(publisher: Option<&str>) -> (TempDir, String) {
    let project = TempDir::new().unwrap();
    let publisher = publisher.map_or(String::new(), |p| format!("publisher = \"{p}\"\n"));
    fs::write(
        project.path().join("manifest.toml"),
        format!("manifest_version = 1\nidentifier = \"com.example.t\"\nname = \"Tee\"\nversion = \"1.0.0\"\nexecutable = \"tee\"\n{publisher}\n[permissions]\ngpu = true\n"),
    )
    .unwrap();
    let mut elf = vec![0u8; 64];
    elf[..4].copy_from_slice(b"\x7fELF");
    elf[4] = 2;
    elf[5] = 1;
    elf[16] = 3;
    elf[18] = 62;
    fs::write(project.path().join("built"), elf).unwrap();
    // Fixture keys, never real ones.
    fs::write(project.path().join("release.seed"), [0x42u8; 32]).unwrap();
    let root = SigningKey::from_seed(&[1; 32])
        .unwrap()
        .public_key()
        .key_id();
    (project, root.to_string())
}

fn bundle(project: &Path, out: &Path, root: &str, extra: &[&str]) -> Output {
    let mut args: Vec<std::ffi::OsString> = vec![
        "bundle".into(),
        project.join("built").into(),
        "--manifest".into(),
        project.join("manifest.toml").into(),
        "-o".into(),
        out.into(),
        "--sign".into(),
        project.join("release.seed").into(),
        "--root".into(),
        root.into(),
        "--release".into(),
        "7".into(),
    ];
    args.extend(extra.iter().map(|a| a.into()));
    let args: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    oma(&args)
}

#[test]
fn bundle_writes_one_signed_file_that_verifies_and_never_replaces_one() {
    let (project, root) = a_project(Some("apps.example.com"));
    let out = TempDir::new().unwrap();
    let made = bundle(project.path(), out.path(), &root, &[]);
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let path = String::from_utf8(made.stdout).unwrap();
    let path = Path::new(path.trim());
    assert_eq!(path, out.path().join("Tee.oma"));
    assert!(path.is_file());
    assert_eq!(&fs::read(path).unwrap()[..4], b"OMA1");

    let sealed = Sealed::memfd(path).unwrap();
    let checked = oma::trust::check(&sealed).unwrap();
    let image = oma::image::Image::read(&checked).unwrap();
    let signed = checked.signed().manifest();
    assert_eq!(signed.version, 7);
    assert_eq!(signed.root_key_id.to_string(), root);
    assert_eq!(signed.update_url, "https://apps.example.com/Tee.oma");
    assert_eq!(image.manifest().identifier.as_str(), "com.example.t");
    // Nothing else is left in the output directory.
    assert_eq!(fs::read_dir(out.path()).unwrap().count(), 1);

    let again = bundle(project.path(), out.path(), &root, &[]);
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));
}

#[test]
fn a_bundle_without_a_publisher_or_with_a_bad_seed_is_not_written() {
    let (project, root) = a_project(None);
    let out = TempDir::new().unwrap();
    let made = bundle(project.path(), out.path(), &root, &[]);
    assert!(!made.status.success());
    assert!(String::from_utf8_lossy(&made.stderr).contains("names no publisher"));

    let (project, root) = a_project(Some("apps.example.com"));
    fs::write(project.path().join("release.seed"), [0x42u8; 31]).unwrap();
    let made = bundle(project.path(), out.path(), &root, &[]);
    assert!(!made.status.success());
    assert!(String::from_utf8_lossy(&made.stderr).contains("32-byte"));
    assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0);
}
