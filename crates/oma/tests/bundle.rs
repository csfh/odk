use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn oma(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_odk-oma"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn bundles_and_then_inspects() {
    let project = TempDir::new().unwrap();
    fs::write(
        project.path().join("manifest.toml"),
        "manifest_version = 1\nidentifier = \"com.example.t\"\nname = \"Tee\"\nversion = \"1.0.0\"\nexecutable = \"tee\"\n\n[permissions]\ngpu = true\n",
    )
    .unwrap();
    let mut elf = vec![0u8; 64];
    elf[..4].copy_from_slice(b"\x7fELF");
    elf[4] = 2;
    elf[5] = 1;
    elf[16] = 3;
    elf[18] = 62;
    fs::write(project.path().join("built"), elf).unwrap();
    let out = TempDir::new().unwrap();

    let made = oma(&[
        "bundle".as_ref(),
        project.path().join("built").as_os_str(),
        "--manifest".as_ref(),
        project.path().join("manifest.toml").as_os_str(),
        "-o".as_ref(),
        out.path().as_os_str(),
    ]);
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let path = String::from_utf8(made.stdout).unwrap();
    assert_eq!(
        path.trim(),
        out.path().join("Tee.oma").display().to_string()
    );

    let inspected = oma(&["inspect".as_ref(), path.trim().as_ref()]);
    assert!(inspected.status.success());
    assert!(String::from_utf8_lossy(&inspected.stdout).starts_with("Tee 1.0.0\ncom.example.t\n"));

    // A second run refuses to replace it.
    let again = oma(&[
        "bundle".as_ref(),
        project.path().join("built").as_os_str(),
        "--manifest".as_ref(),
        project.path().join("manifest.toml").as_os_str(),
        "-o".as_ref(),
        out.path().as_os_str(),
    ]);
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));
}
