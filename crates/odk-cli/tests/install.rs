//! `install.sh` (ADR 0063), against a release built by `tools/package-release`, the same script CI
//! uses, in a throwaway home. The programs are shell scripts, except `odk`, which is the real
//! dispatcher so that the layout is shown to work. `systemctl` and `update-mime-database` are stubs
//! that record their arguments; nothing here reaches the real user manager, the network or the
//! real home.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn executable(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        for d in ["home", "tmp", "stubs", "release"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        executable(
            &root.join("stubs/systemctl"),
            "#!/bin/sh\necho \"$*\" >>\"$SYSTEMCTL_LOG\"\n\
             if [ \"$1 $2\" = \"--user show-environment\" ] && [ -e \"$NO_MANAGER\" ]; then exit 1; fi\nexit 0\n",
        );
        executable(
            &root.join("stubs/update-mime-database"),
            "#!/bin/sh\necho \"$*\" >>\"$MIME_LOG\"\n",
        );
        Self { _dir: dir, root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn log(&self) -> Vec<String> {
        self.lines("systemctl.log")
    }

    /// The directories `update-mime-database` was asked to rebuild.
    fn mime_log(&self) -> Vec<String> {
        self.lines("mime.log")
    }

    fn lines(&self, log: &str) -> Vec<String> {
        fs::read_to_string(self.root.join(log))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Builds a release of `version` into `release/`, with `tools/package-release`.
    fn publish(&self, version: &str) {
        let bin = self.root.join(format!("built-{version}"));
        fs::create_dir_all(&bin).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_odk"), bin.join("odk")).unwrap();
        for (name, says) in [
            ("odk-oma", "odk oma"),
            ("oma-consent", "consent"),
            ("odkd", "odkd"),
        ] {
            executable(
                &bin.join(name),
                &format!("#!/bin/sh\necho '{says} {version}'\n"),
            );
        }
        let out = Command::new(repo().join("tools/package-release"))
            .arg(&bin)
            .arg(self.root.join("release"))
            .arg(version)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        // The script as published; the copy stays when a test removes the release.
        fs::copy(
            self.root.join("release/install.sh"),
            self.root.join("install.sh"),
        )
        .unwrap();
    }

    fn script(&self) -> PathBuf {
        self.root.join("install.sh")
    }

    fn install_with(&self, shell: &[&str], args: &[&str], extra: &[(&str, String)]) -> Output {
        let mut command = Command::new(shell[0]);
        command
            .args(&shell[1..])
            .arg(self.script())
            .args(args)
            .env_clear()
            .env("HOME", self.home())
            .env("TMPDIR", self.root.join("tmp"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.join("stubs").display()),
            )
            .env(
                "ODK_BASE_URL",
                format!("file://{}", self.root.join("release").display()),
            )
            .env("SYSTEMCTL_LOG", self.root.join("systemctl.log"))
            .env("MIME_LOG", self.root.join("mime.log"))
            .env("NO_MANAGER", self.root.join("no-manager"));
        for (k, v) in extra {
            command.env(k, v);
        }
        for _ in 0..50 {
            match command.output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => return other.unwrap(),
            }
        }
        panic!("busy");
    }

    fn install(&self, args: &[&str]) -> Output {
        self.install_with(&["sh"], args, &[])
    }

    /// Every file and directory under the home, relative to it, sorted.
    fn tree(&self) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                out.push(path.strip_prefix(base).unwrap().display().to_string());
                if path.is_dir() {
                    walk(&path, base, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.home(), &self.home(), &mut out);
        out.sort();
        out
    }
}

fn out(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// The tests need to be someone who is not root, as the script is.
fn skip_as_root() -> bool {
    let id = Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&id.stdout).trim() == "0"
}

const FILES: [&str; 7] = [
    ".local/bin/odk",
    ".local/lib/odk/odk-oma",
    ".local/lib/odk/oma-consent",
    ".local/lib/odk/odkd",
    ".config/systemd/user/odkd.socket",
    ".config/systemd/user/odkd.service",
    ".local/share/mime/packages/oma.xml",
];

#[test]
fn it_installs_the_command_the_private_programs_and_the_units_for_this_user_only() {
    if skip_as_root() {
        return;
    }
    for shell in [&["sh"][..], &["bash", "--posix"]] {
        let w = World::new();
        w.publish("1.2.3");
        let o = w.install_with(shell, &[], &[]);
        assert!(o.status.success(), "{shell:?}: {}", out(&o));
        let text = out(&o);
        assert!(text.contains("installed odk 1.2.3"), "{text}");
        assert!(text.contains("checksum ok"), "{text}");

        // Exactly these paths, and nothing else, under the home.
        let tree = w.tree();
        for file in FILES {
            assert!(tree.contains(&file.to_owned()), "{file} in {tree:?}");
        }
        let files: Vec<&String> = tree.iter().filter(|p| w.home().join(p).is_file()).collect();
        assert_eq!(files.len(), FILES.len(), "{files:?}");
        for file in &files[..] {
            let mode = fs::metadata(w.home().join(file))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            let want = if file.contains("systemd") || file.ends_with(".xml") {
                0o644
            } else {
                0o755
            };
            assert_eq!(mode, want, "{file}");
        }
        assert!(
            tree.iter().all(|p| !p.contains(".new.")),
            "no temporary file is left: {tree:?}"
        );
        assert!(
            fs::read_dir(w.root.join("tmp")).unwrap().next().is_none(),
            "the working directory is gone"
        );

        // The units name the program with %h, so they hold no user name.
        let service =
            fs::read_to_string(w.home().join(".config/systemd/user/odkd.service")).unwrap();
        assert!(
            service.contains("ExecStart=%h/.local/lib/odk/odkd"),
            "{service}"
        );

        // The installed command finds the installed private program: `odk oma` ran odk-oma.
        let ran = Command::new(w.home().join(".local/bin/odk"))
            .args(["oma", "--version"])
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "odk oma 1.2.3");

        // Everything told to systemd is for this user's manager, in this order.
        let log = w.log();
        assert!(log.iter().all(|l| l.starts_with("--user ")), "{log:?}");
        assert_eq!(
            &log[1..],
            [
                "--user daemon-reload",
                "--user enable --now odkd.socket",
                "--user try-restart odkd.service"
            ],
            "{log:?}"
        );

        // The file type, as packaged, and the user's MIME database rebuilt with it (ADR 0065).
        assert_eq!(
            fs::read(w.home().join(".local/share/mime/packages/oma.xml")).unwrap(),
            fs::read(repo().join("packaging/mime/oma.xml")).unwrap()
        );
        assert_eq!(
            w.mime_log(),
            [w.home().join(".local/share/mime").display().to_string()]
        );
    }
}

#[test]
fn running_it_again_upgrades_and_restarts_odkd() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    assert!(w.install(&[]).status.success());
    w.publish("1.1.0");
    let o = w.install(&[]);
    assert!(o.status.success(), "{}", out(&o));
    assert!(out(&o).contains("installed odk 1.1.0"));
    let odkd = Command::new(w.home().join(".local/lib/odk/odkd"))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&odkd.stdout).trim(), "odkd 1.1.0");
    assert_eq!(
        w.log()
            .iter()
            .filter(|l| l.contains("try-restart odkd.service"))
            .count(),
        2
    );
}

#[test]
fn a_download_that_does_not_match_its_checksum_installs_nothing() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    let tarball = w.root.join("release/odk-x86_64-linux.tar.gz");
    let mut bytes = fs::read(&tarball).unwrap();
    let last = bytes.len() / 2;
    bytes[last] ^= 0xff;
    fs::write(&tarball, bytes).unwrap();
    let o = w.install(&[]);
    assert!(!o.status.success());
    assert!(
        out(&o).contains("does not match its checksum"),
        "{}",
        out(&o)
    );
    assert!(w.tree().is_empty(), "{:?}", w.tree());
    assert!(w.log().is_empty(), "systemd was not touched: {:?}", w.log());
}

#[test]
fn a_checksum_line_for_another_file_is_not_a_checksum_of_the_download() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    let sums = w.root.join("release/odk-x86_64-linux.tar.gz.sha256");
    // The sum of an empty file, for a file that is not the tarball: it verifies as itself.
    for line in [
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  /dev/null\n",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  other.tar.gz\n",
        "",
    ] {
        fs::write(&sums, line).unwrap();
        let o = w.install(&[]);
        assert!(!o.status.success(), "{line:?}");
        let text = out(&o);
        assert!(
            text.contains("is for '") || text.contains("is empty"),
            "{text}"
        );
        assert!(w.tree().is_empty());
    }
}

#[test]
fn a_missing_checksum_or_tarball_installs_nothing() {
    if skip_as_root() {
        return;
    }
    for missing in ["odk-x86_64-linux.tar.gz.sha256", "odk-x86_64-linux.tar.gz"] {
        let w = World::new();
        w.publish("1.0.0");
        fs::remove_file(w.root.join("release").join(missing)).unwrap();
        let o = w.install(&[]);
        assert!(!o.status.success(), "{missing}");
        assert!(out(&o).contains("cannot download"), "{}", out(&o));
        assert!(w.tree().is_empty());
    }
}

#[test]
fn a_tarball_with_a_path_outside_odk_is_refused_even_with_a_matching_checksum() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    // A tarball with the right checksum that reaches out of its directory.
    let evil = w.root.join("evil");
    fs::create_dir_all(evil.join("odk")).unwrap();
    fs::write(evil.join("odk/VERSION"), "9\n").unwrap();
    fs::write(w.root.join("outside"), "x").unwrap();
    let tarball = w.root.join("release/odk-x86_64-linux.tar.gz");
    let status = Command::new("tar")
        .args(["-P", "-czf"])
        .arg(&tarball)
        .arg("-C")
        .arg(&evil)
        .arg("odk")
        .arg("odk/../../outside")
        .status()
        .unwrap();
    assert!(status.success());
    let sums = Command::new("sha256sum")
        .arg("odk-x86_64-linux.tar.gz")
        .current_dir(w.root.join("release"))
        .output()
        .unwrap();
    fs::write(
        w.root.join("release/odk-x86_64-linux.tar.gz.sha256"),
        sums.stdout,
    )
    .unwrap();
    let o = w.install(&[]);
    assert!(!o.status.success(), "{}", out(&o));
    assert!(out(&o).contains("nothing was installed"), "{}", out(&o));
    assert!(w.tree().is_empty());
}

#[test]
fn a_tarball_missing_a_file_installs_nothing() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    let stage = w.root.join("stage");
    fs::create_dir_all(&stage).unwrap();
    let tarball = w.root.join("release/odk-x86_64-linux.tar.gz");
    assert!(
        Command::new("tar")
            .arg("-xzf")
            .arg(&tarball)
            .arg("-C")
            .arg(&stage)
            .status()
            .unwrap()
            .success()
    );
    fs::remove_file(stage.join("odk/lib/odk/odkd")).unwrap();
    assert!(
        Command::new("tar")
            .arg("-czf")
            .arg(&tarball)
            .arg("-C")
            .arg(&stage)
            .arg("odk")
            .status()
            .unwrap()
            .success()
    );
    let sums = Command::new("sha256sum")
        .arg("odk-x86_64-linux.tar.gz")
        .current_dir(w.root.join("release"))
        .output()
        .unwrap();
    fs::write(
        w.root.join("release/odk-x86_64-linux.tar.gz.sha256"),
        sums.stdout,
    )
    .unwrap();
    let o = w.install(&[]);
    assert!(!o.status.success());
    assert!(out(&o).contains("has no lib/odk/odkd"), "{}", out(&o));
    assert!(w.tree().is_empty());
}

#[test]
fn without_a_user_session_it_installs_the_files_and_says_how_to_start_odkd() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    fs::write(w.root.join("no-manager"), "").unwrap();
    let o = w.install(&[]);
    assert!(o.status.success(), "{}", out(&o));
    let text = out(&o);
    assert!(text.contains("no systemd user session"), "{text}");
    assert!(
        text.contains("systemctl --user enable --now odkd.socket"),
        "{text}"
    );
    for file in FILES {
        assert!(w.home().join(file).is_file(), "{file}");
    }
    let log = w.log();
    assert!(
        log.iter().all(|l| l.ends_with("show-environment")),
        "only asked: {log:?}"
    );
}

#[test]
fn only_https_or_a_local_file_is_a_place_to_download_from() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    for base in [
        "http://example.invalid/odk",
        "ftp://example.invalid",
        "/tmp/odk",
        "",
    ] {
        let o = w.install_with(&["sh"], &[], &[("ODK_BASE_URL", base.to_owned())]);
        if base.is_empty() {
            // An empty override is no override; the real default would be the network.
            continue;
        }
        assert!(!o.status.success(), "{base}");
        assert!(out(&o).contains("must be https://"), "{}", out(&o));
        assert!(w.tree().is_empty());
    }
}

#[test]
fn it_says_when_the_bin_directory_is_not_on_path() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    let o = w.install(&[]);
    assert!(out(&o).contains("is not on your PATH"), "{}", out(&o));
    let on_path = format!(
        "{}:{}:/usr/bin:/bin",
        w.root.join("stubs").display(),
        w.home().join(".local/bin").display()
    );
    let o = w.install_with(&["sh"], &[], &[("PATH", on_path)]);
    assert!(o.status.success(), "{}", out(&o));
    assert!(!out(&o).contains("is not on your PATH"), "{}", out(&o));
}

#[test]
fn the_unit_directory_follows_xdg_config_home_and_the_programs_stay_under_local() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    let config = w.root.join("home/cfg");
    let o = w.install_with(
        &["sh"],
        &[],
        &[("XDG_CONFIG_HOME", config.display().to_string())],
    );
    assert!(o.status.success(), "{}", out(&o));
    assert!(config.join("systemd/user/odkd.socket").is_file());
    assert!(w.home().join(".local/lib/odk/odk-oma").is_file());
    assert!(w.home().join(".local/bin/odk").is_file());
}

#[test]
fn the_file_type_follows_xdg_data_home() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    let data = w.root.join("home/data");
    let env = [("XDG_DATA_HOME", data.display().to_string())];
    let o = w.install_with(&["sh"], &[], &env);
    assert!(o.status.success(), "{}", out(&o));
    assert!(data.join("mime/packages/oma.xml").is_file());
    assert!(!w.home().join(".local/share/mime").exists());
    assert_eq!(w.mime_log(), [data.join("mime").display().to_string()]);

    let o = w.install_with(&["sh"], &["--uninstall"], &env);
    assert!(o.status.success(), "{}", out(&o));
    assert!(!data.join("mime/packages/oma.xml").exists());
    assert_eq!(
        w.mime_log().len(),
        2,
        "rebuilt without it: {:?}",
        w.mime_log()
    );
}

#[test]
fn uninstall_removes_what_was_installed_and_nothing_else() {
    if skip_as_root() {
        return;
    }
    let w = World::new();
    w.publish("1.0.0");
    assert!(w.install(&[]).status.success());
    fs::write(w.home().join(".local/bin/other"), "mine").unwrap();
    fs::create_dir_all(w.home().join(".local/share/oma/data")).unwrap();
    fs::write(w.home().join(".local/share/oma/data/keep"), "data").unwrap();
    fs::write(w.home().join(".config/systemd/user/other.service"), "mine").unwrap();

    let o = w.install(&["--uninstall"]);
    assert!(o.status.success(), "{}", out(&o));
    for file in FILES {
        assert!(!w.home().join(file).exists(), "{file}");
    }
    assert!(
        !w.home().join(".local/lib/odk").exists(),
        "the empty directory goes too"
    );
    for kept in [
        ".local/bin/other",
        ".local/share/oma/data/keep",
        ".config/systemd/user/other.service",
    ] {
        assert!(w.home().join(kept).is_file(), "{kept}");
    }
    let mime = w.home().join(".local/share/mime").display().to_string();
    assert_eq!(
        w.mime_log(),
        [mime.clone(), mime],
        "rebuilt without the type"
    );
    assert!(
        w.log()
            .iter()
            .any(|l| l == "--user disable --now odkd.socket odkd.service"),
        "{:?}",
        w.log()
    );
    // And it does not need the network or a release to do it.
    fs::remove_dir_all(w.root.join("release")).unwrap();
    assert!(w.install(&["--uninstall"]).status.success());
}

#[test]
fn a_download_is_https_only_and_a_named_release_has_its_own_address() {
    if skip_as_root() {
        return;
    }
    // No network in a test: a `curl` that records its arguments and fails stands in, and the
    // script's own address and flags are what is looked at.
    let w = World::new();
    w.publish("1.0.0");
    let log = w.root.join("curl.log");
    executable(
        &w.root.join("curlstub/curl"),
        "#!/bin/sh\necho \"$*\" >>\"$CURL_LOG\"\nexit 22\n",
    );
    let path = format!(
        "{}:{}:/usr/bin:/bin",
        w.root.join("curlstub").display(),
        w.root.join("stubs").display()
    );
    let run = |args: &[&str]| {
        w.install_with(
            &["sh"],
            args,
            &[
                ("ODK_BASE_URL", String::new()),
                ("PATH", path.clone()),
                ("CURL_LOG", log.display().to_string()),
            ],
        )
    };
    assert!(!run(&["--version", "v9.9.9"]).status.success());
    assert!(!run(&[]).status.success());
    let logged = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = logged.lines().collect();
    assert!(
        lines[0].starts_with("--proto =https --proto-redir =https --tlsv1.2 -fsSL -o "),
        "{}",
        lines[0]
    );
    assert!(
        lines[0].ends_with(
            "https://github.com/csfh/odk/releases/download/v9.9.9/odk-x86_64-linux.tar.gz"
        ),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].ends_with(
            "https://github.com/csfh/odk/releases/latest/download/odk-x86_64-linux.tar.gz"
        ),
        "{}",
        lines[1]
    );
    assert!(w.tree().is_empty());
}

#[test]
fn bad_options_are_usage_errors() {
    let w = World::new();
    w.publish("1.0.0");
    for args in [&["--nope"][..], &["--version"]] {
        let o = w.install(args);
        assert!(!o.status.success(), "{args:?}");
        assert!(w.tree().is_empty());
    }
}
