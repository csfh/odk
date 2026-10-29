//! Shared by the end-to-end tests: a temporary home, a probe Bundle, and the real `oma`.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct World {
    pub _dir: TempDir,
    pub home: PathBuf,
    pub runtime: PathBuf,
    pub data: PathBuf,
    pub _wayland: UnixListener,
    /// A private copy of `oma` with no `oma-consent` next to it, so no test can open the real
    /// consent dialog and wait for a click that never comes.
    pub oma_bin: PathBuf,
}

pub const ID: &str = "com.example.probe";

pub fn probe_binary() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/probe")
}

impl World {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let (home, runtime) = (root.join("home"), root.join("runtime"));
        fs::create_dir_all(home.join(".config")).unwrap();
        fs::create_dir_all(home.join("Documents")).unwrap();
        fs::create_dir_all(home.join("Music")).unwrap();
        fs::write(home.join("secret"), "the real home's secret").unwrap();
        fs::write(home.join("Documents/one.txt"), "1").unwrap();
        fs::create_dir_all(&runtime).unwrap();
        let mut perms = fs::metadata(&runtime).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        fs::set_permissions(&runtime, perms).unwrap();
        let wayland = UnixListener::bind(runtime.join("wayland-test")).unwrap();
        fs::write(
            home.join(".config/user-dirs.dirs"),
            "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\n",
        )
        .unwrap();
        let data = home.join(".local/share/oma/data").join(ID);
        let bin = root.join("oma-bin");
        fs::create_dir_all(&bin).unwrap();
        let oma_bin = bin.join("odk-oma");
        fs::copy(env!("CARGO_BIN_EXE_odk-oma"), &oma_bin).unwrap();
        assert!(!bin.join("oma-consent").exists());
        World {
            _dir: dir,
            home,
            runtime,
            data,
            _wayland: wayland,
            oma_bin,
        }
    }

    pub fn oma(&self, args: &[&str]) -> Output {
        output_retrying(
            Command::new(&self.oma_bin)
                .args(args)
                .env_clear()
                .env("HOME", &self.home)
                .env("XDG_RUNTIME_DIR", &self.runtime)
                .env("WAYLAND_DISPLAY", "wayland-test")
                .env("LANG", "en_US.UTF-8")
                .env("SECRET_HOST_VARIABLE", "leaked")
                .env("PATH", "/usr/bin"),
        )
    }

    /// Bundles the probe with the given `[permissions]` text and installs it with consent.
    pub fn install(&self, permissions: &str) {
        let manifest = self.home.join("manifest.toml");
        fs::write(
            &manifest,
            format!("manifest_version = 1\nidentifier = \"{ID}\"\nname = \"Probe\"\nversion = \"1.0.0\"\nexecutable = \"probe\"\n\n[permissions]\n{permissions}\n"),
        )
        .unwrap();
        let out = self.home.join("out");
        let made = self.oma(&[
            "bundle",
            probe_binary().to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        let installed = self.oma(&[
            "install",
            "--accept-permissions",
            out.join("Probe.oma").to_str().unwrap(),
        ]);
        assert!(
            installed.status.success(),
            "{}",
            String::from_utf8_lossy(&installed.stderr)
        );
    }

    /// Installs the probe as a Bundle file, as `odk oma install` leaves it at first contact: the
    /// image from `oma::image::write`, signed with fixture keys, in `~/Applications`, and its
    /// pair's record with the permissions and the mark. Nothing is fetched. Returns the pair's
    /// data directory.
    pub fn install_file(&self, permissions: &str) -> PathBuf {
        use oma::trust::{Mark, Publisher, Sealed, SignedManifest, SigningKey, Timestamp};
        let tree = self.home.join("tree");
        fs::create_dir_all(&tree).unwrap();
        let text = format!(
            "manifest_version = 1\nidentifier = \"{ID}\"\nname = \"Probe\"\nversion = \"1.0.0\"\nexecutable = \"probe\"\npublisher = \"apps.example.com\"\n\n[permissions]\n{permissions}\n"
        );
        fs::write(tree.join("manifest.toml"), &text).unwrap();
        fs::copy(probe_binary(), tree.join("probe")).unwrap();
        let mut image = Vec::new();
        oma::image::write(&tree, "Probe", &mut image).unwrap();

        // Fixture keys, never real ones.
        let root = SigningKey::from_seed(&[1; 32]).unwrap();
        let release = SigningKey::from_seed(&[0x42; 32]).unwrap();
        let publisher = Publisher::parse("apps.example.com").unwrap();
        let signed = SignedManifest {
            app_id: ID.parse().unwrap(),
            version: 1,
            entry: "probe".to_owned(),
            payload_sha256: [0; 32],
            header_sha256: [0; 32],
            key_id: release.public_key().key_id(),
            root_key_id: root.public_key().key_id(),
            issued_at: Timestamp::parse("2026-10-27T00:00:00Z").unwrap(),
            expires_at: Timestamp::parse("2099-01-01T00:00:00Z").unwrap(),
            update_url: "https://apps.example.com/probe.oma".to_owned(),
            publisher: publisher.clone(),
        };
        let bytes = oma::trust::stamp(&image[1024..], signed, &release).unwrap();
        let apps = self.home.join("Applications");
        fs::create_dir_all(&apps).unwrap();
        let file = apps.join("Probe.oma");
        fs::write(&file, &bytes).unwrap();

        let sealed = Sealed::memfd(&file).unwrap();
        let checked = oma::trust::check(&sealed).unwrap();
        let layout = oma::layout::Layout::new(&self.home, self.home.join(".local/share"));
        let id = ID.parse().unwrap();
        let pair = oma::grants::Pair {
            root: root.public_key(),
            publisher,
            permissions: Some(oma::fmt::Manifest::parse(&text).unwrap().permissions),
            mark: Some(Mark {
                version: 1,
                payload_sha256: checked.signed().manifest().payload_sha256,
            }),
            denied: false,
        };
        oma::grants::save_pair(&layout, &id, &pair).unwrap();
        layout.pair_data_dir(&root.public_key().key_id(), &id)
    }

    /// [`World::install`] for a Bundle that declares a daemon: `daemon` is the text of its
    /// `[daemon]` table, and the probe is also put in the Bundle as `bin/d`, the program it names.
    pub fn install_with_daemon(&self, permissions: &str, daemon: &str) -> PathBuf {
        let manifest = self.home.join("manifest.toml");
        fs::write(
            &manifest,
            format!("manifest_version = 1\nidentifier = \"{ID}\"\nname = \"Probe\"\nversion = \"1.0.0\"\nexecutable = \"probe\"\n\n[permissions]\n{permissions}\n\n[daemon]\n{daemon}\n"),
        )
        .unwrap();
        let out = self.home.join("out");
        let probe = probe_binary();
        let helper = format!("{}=bin/d", probe.display());
        let made = self.oma(&[
            "bundle",
            probe.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
            "--include",
            &helper,
            "-o",
            out.to_str().unwrap(),
        ]);
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        let installed = self.oma(&[
            "install",
            "--accept-permissions",
            out.join("Probe.oma").to_str().unwrap(),
        ]);
        assert!(
            installed.status.success(),
            "{}",
            String::from_utf8_lossy(&installed.stderr)
        );
        out.join("Probe.oma")
    }

    /// `odk oma run --wait` with `PATH` set to `path`, so a fake `systemd-run` can stand in for the
    /// user manager, which no test may start a unit in.
    pub fn run_wait_with_path(&self, path: &str) -> (Output, HashMap<String, String>) {
        self.run_target_with_path(ID, path)
    }

    /// [`World::run_wait_with_path`] for any target: an Identifier, or the path of a Bundle.
    pub fn run_target_with_path(
        &self,
        target: &str,
        path: &str,
    ) -> (Output, HashMap<String, String>) {
        let mut command = Command::new(&self.oma_bin);
        command
            .args(["run", "--wait", target])
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env("LANG", "en_US.UTF-8")
            .env("SECRET_HOST_VARIABLE", "leaked")
            .env("PATH", path);
        let out = output_retrying(&mut command);
        let report = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
            .collect();
        (out, report)
    }

    /// A directory with a `systemd-run` in it that records its arguments, one a line, in `argv`.
    pub fn fake_systemd_run(&self) -> PathBuf {
        let dir = self.home.join("fakes");
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join("systemd-run");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/argv.tmp\" && mv \"$(dirname \"$0\")/argv.tmp\" \"$(dirname \"$0\")/argv\"\n",
        )
        .unwrap();
        fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        dir
    }

    /// A copy of `oma` with a fake `oma-consent` next to it, which saves the request it is sent
    /// and answers with `answer`. Returns the directory holding both.
    pub fn oma_with_helper(&self, answer: &str) -> PathBuf {
        let dir = self.home.join("bin");
        fs::create_dir_all(&dir).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_odk-oma"), dir.join("odk-oma")).unwrap();
        let helper = dir.join("oma-consent");
        fs::write(&helper, "#!/bin/sh\ncat > \"$(dirname \"$0\")/request.toml\"\ncat \"$(dirname \"$0\")/answer\"\n").unwrap();
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut fs::metadata(&helper).unwrap().permissions(),
            0o755,
        );
        fs::set_permissions(&helper, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        fs::write(dir.join("answer"), answer).unwrap();
        dir
    }

    /// Runs the copy of `oma` in `bin` (see [`World::oma_with_helper`]).
    pub fn oma_in(&self, bin: &Path, args: &[&str]) -> Output {
        let mut command = Command::new(bin.join("odk-oma"));
        command
            .args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env("LANG", "en_US.UTF-8")
            .env("PATH", "/usr/bin");
        output_retrying(&mut command)
    }

    /// Writes a Bundle with the given `[permissions]` text and returns its directory, without
    /// installing it.
    pub fn bundle(&self, permissions: &str) -> PathBuf {
        let manifest = self.home.join("manifest.toml");
        fs::write(
            &manifest,
            format!("manifest_version = 1\nidentifier = \"{ID}\"\nname = \"Probe\"\nversion = \"1.0.0\"\nexecutable = \"probe\"\n\n[permissions]\n{permissions}\n"),
        )
        .unwrap();
        let out = self.home.join("out");
        let made = self.oma(&[
            "bundle",
            probe_binary().to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        out.join("Probe.oma")
    }

    pub fn grant_file(&self) -> PathBuf {
        self.home
            .join(".local/share/oma/grants")
            .join(format!("{ID}.toml"))
    }

    pub fn control(&self, name: &str, content: &str) {
        fs::create_dir_all(&self.data).unwrap();
        fs::write(self.data.join(name), content).unwrap();
    }

    pub fn run_wait(&self) -> (Output, HashMap<String, String>) {
        let out = self.oma(&["run", "--wait", ID]);
        let report = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
            .collect();
        (out, report)
    }
}

/// These tests need unprivileged user namespaces and Landlock ABI 6; elsewhere they are skipped.
pub fn supported() -> bool {
    let userns = Command::new("unshare")
        .args(["--user", "true"])
        .status()
        .is_ok_and(|s| s.success());
    let landlock = oma::sandbox::landlock::abi_version().is_ok_and(|abi| abi >= 6);
    if !(userns && landlock) {
        eprintln!("skipped: needs unprivileged user namespaces and Landlock ABI 6");
    }
    userns && landlock
}

pub fn wait_for(path: &Path) {
    for _ in 0..200 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{} never appeared", path.display());
}

/// Runs a command, retrying while the program is "busy". A binary that was just copied can
/// still be open for writing in another test's forked child until that child execs, which
/// makes the kernel refuse to run it for a moment.
pub fn output_retrying(command: &mut Command) -> Output {
    retrying(|| command.output())
}

/// Retries an operation that starts a program while the program is "busy" (see
/// [`output_retrying`]).
pub fn retrying<T>(mut start: impl FnMut() -> std::io::Result<T>) -> T {
    for _ in 0..100 {
        match start() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other.unwrap(),
        }
    }
    panic!("the program stayed busy");
}
