//! Signing an odk release (ADR 0086): a release `tools/package-release` makes from stand-in
//! programs is signed with a fixture root, its `release` verifies under that root, and its
//! `install.sh.sig` verifies with the OpenSSL commands `packaging/README.md` gives.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use oma::odk_release::{self, Error, TARBALL};
use oma::trust::SigningKey;
use oma::trust::release;
use tempfile::TempDir;

/// The release's version, as `SOURCE_DATE_EPOCH`.
const VERSION: u64 = 1_800_000_000;

// Fixture keys, never real ones.
fn root() -> SigningKey {
    SigningKey::from_seed(&[0x61; 32]).unwrap()
}

struct Made {
    _dir: TempDir,
    out: PathBuf,
    /// The stand-in for the `odk-oma` being released.
    odk_oma: PathBuf,
}

/// A release of stand-in programs, as `tools/package-release` makes it.
fn made() -> Made {
    let dir = TempDir::new().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    for name in ["odk", "odk-oma", "oma-consent", "odkd"] {
        let path = bin.join(name);
        fs::write(&path, format!("#!/bin/sh\necho {name}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(bin.join("THIRD-PARTY-LICENSES"), "licences\n").unwrap();
    let out = dir.path().join("out");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let made = Command::new(repo.join("tools/package-release"))
        .arg(&bin)
        .arg(&out)
        .arg("test-1")
        .env("SOURCE_DATE_EPOCH", VERSION.to_string())
        .output()
        .unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    Made {
        odk_oma: bin.join("odk-oma"),
        out,
        _dir: dir,
    }
}

fn sign(
    made: &Made,
    key: &SigningKey,
    compiled: Option<&SigningKey>,
) -> Result<release::Release, Error> {
    let compiled = compiled.map(SigningKey::public_key);
    odk_release::sign(
        &made.out,
        key,
        compiled.as_ref(),
        &made.odk_oma,
        VERSION as i64 + 60,
    )
}

fn signed_files(out: &Path) -> Vec<bool> {
    ["release", "release.sig", "install.sh.sig"]
        .iter()
        .map(|name| out.join(name).exists())
        .collect()
}

/// `sh -c` of a command from `packaging/README.md`, in `dir`.
fn shell(dir: &Path, command: &str) -> std::process::Output {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .output()
        .unwrap()
}

#[test]
fn a_signed_release_verifies_under_its_root_and_its_script_under_openssl() {
    let made = made();
    let signed = sign(&made, &root(), Some(&root())).unwrap();
    assert_eq!(signed.version, VERSION, "the version the script carries");
    assert_eq!(signed.tarball, TARBALL);

    let body = fs::read(made.out.join("release")).unwrap();
    let signature = fs::read(made.out.join("release.sig")).unwrap();
    let verified = release::verify(&body, &signature, &root().public_key()).unwrap();
    assert_eq!(verified, signed);
    let script = fs::read(made.out.join("install.sh")).unwrap();
    assert_eq!(verified.script.length, script.len() as u64);

    // As a person checks it, with OpenSSL 3 and the root's PEM (ADR 0060, section 8).
    if Command::new("openssl").arg("version").output().is_err() {
        eprintln!("skipped the OpenSSL half: no openssl");
        return;
    }
    fs::write(
        made.out.join("odk-release-root.pem"),
        odk_release::pem(&root().public_key()),
    )
    .unwrap();
    let fingerprint = shell(
        &made.out,
        "openssl pkey -pubin -in odk-release-root.pem -outform DER | tail -c 32 | sha256sum",
    );
    assert!(fingerprint.status.success());
    assert_eq!(
        String::from_utf8_lossy(&fingerprint.stdout)
            .split(' ')
            .next(),
        Some(root().public_key().fingerprint().to_string().as_str())
    );
    let verify = "openssl pkeyutl -verify -pubin -inkey odk-release-root.pem -rawin -in install.sh -sigfile install.sh.sig";
    let checked = shell(&made.out, verify);
    assert!(
        checked.status.success(),
        "{}{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    // One byte of the script changed: OpenSSL refuses it.
    let mut changed = script.clone();
    let at = changed.len() - 2;
    changed[at] ^= 1;
    fs::write(made.out.join("install.sh"), changed).unwrap();
    assert!(!shell(&made.out, verify).status.success());
}

#[test]
fn nothing_is_signed_for_another_root_a_build_with_none_or_another_binary() {
    let made = made();
    let other = SigningKey::from_seed(&[0x62; 32]).unwrap();
    assert!(matches!(
        sign(&made, &other, Some(&root())),
        Err(Error::NotThisRoot { .. })
    ));
    assert!(matches!(
        sign(&made, &root(), None),
        Err(Error::NoReleaseRoot)
    ));
    // The tarball's odk-oma is not the one signing.
    fs::write(&made.odk_oma, "#!/bin/sh\necho another\n").unwrap();
    assert!(matches!(
        sign(&made, &root(), Some(&root())),
        Err(Error::NotThisBinary)
    ));
    assert_eq!(signed_files(&made.out), [false; 3]);
}

#[test]
fn a_tarball_that_is_not_the_one_the_script_names_is_not_signed() {
    let made = made();
    let tarball = made.out.join(TARBALL);
    let mut bytes = fs::read(&tarball).unwrap();
    bytes.push(0);
    fs::write(&tarball, bytes).unwrap();
    assert!(matches!(
        sign(&made, &root(), Some(&root())),
        Err(Error::Tarball(_))
    ));
    // The repository's script names no release at all.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh");
    fs::copy(repo, made.out.join("install.sh")).unwrap();
    assert!(matches!(
        sign(&made, &root(), Some(&root())),
        Err(Error::NotPackaged { .. })
    ));
    assert_eq!(signed_files(&made.out), [false; 3]);
}
