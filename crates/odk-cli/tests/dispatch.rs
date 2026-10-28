//! The real `odk` binary, in an install layout of its own: `<root>/bin/odk` and the private
//! directory `<root>/lib/odk`. The programs it finds are shell scripts that report how they
//! were run.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Install {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Install {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("lib/odk")).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_odk"), root.join("bin/odk")).unwrap();
        Self { dir, root }
    }

    fn odk(&self) -> PathBuf {
        self.root.join("bin/odk")
    }

    /// A program that prints its `argv[0]`, the `ODK` variable and each argument, then exits
    /// with `code`.
    fn program(&self, path: &Path, code: i32) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "#!/bin/sh\necho \"argv0=$0\"\necho \"odk=$ODK\"\nfor a in \"$@\"; do echo \"arg=[$a]\"; done\nexit {code}\n"
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn run(&self, path: &str, args: &[&str]) -> Output {
        // Another test thread may be writing an executable while this one forks, which makes
        // the exec fail with ETXTBSY for a moment (the same retry as oma's own tests).
        for _ in 0..50 {
            let result = Command::new(self.odk())
                .args(args)
                .env_clear()
                .env("PATH", path)
                .current_dir(self.dir.path())
                .output();
            match result {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => return other.unwrap(),
            }
        }
        panic!("the odk file stayed busy");
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn oma_runs_the_private_program_with_the_arguments_unchanged_and_its_status() {
    let w = Install::new();
    let private = w.root.join("lib/odk/odk-oma");
    w.program(&private, 7);
    let o = w.run("/usr/bin", &["oma", "run", "--wait", "", "./App.oma"]);
    assert_eq!(o.status.code(), Some(7), "the program's own status");
    let text = out(&o);
    assert!(
        text.contains(&format!("argv0={}", private.display())),
        "{text}"
    );
    assert!(
        text.contains(&format!("odk={}", w.odk().display())),
        "{text}"
    );
    for arg in ["run", "--wait", "", "./App.oma"] {
        assert!(text.contains(&format!("arg=[{arg}]")), "{arg:?} in {text}");
    }
    assert!(!text.contains("arg=[oma]"), "the name is not forwarded");
}

#[test]
fn a_copy_of_odk_oma_on_path_is_never_the_program() {
    let w = Install::new();
    let elsewhere = tempfile::tempdir().unwrap();
    w.program(&elsewhere.path().join("odk-oma"), 0);
    let o = w.run(elsewhere.path().to_str().unwrap(), &["oma", "run"]);
    assert_eq!(o.status.code(), Some(127));
    let e = err(&o);
    assert!(e.contains("no such command: 'oma'"), "{e}");
    assert!(e.contains("a copy on PATH is not used"), "{e}");
    assert!(
        e.contains(w.root.join("lib/odk/odk-oma").to_str().unwrap()),
        "{e}"
    );
}

#[test]
fn another_name_is_looked_for_in_the_private_directory_then_in_absolute_path_entries() {
    let w = Install::new();
    let (first, second) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    w.program(&second.path().join("odk-tool"), 0);
    let path = format!("{}:{}", first.path().display(), second.path().display());
    let o = w.run(&path, &["tool", "x"]);
    assert!(o.status.success());
    assert!(out(&o).contains(&format!(
        "argv0={}",
        second.path().join("odk-tool").display()
    )));

    // The private directory wins over PATH.
    w.program(&w.root.join("lib/odk/odk-tool"), 0);
    let o = w.run(&path, &["tool"]);
    assert!(out(&o).contains(&format!(
        "argv0={}",
        w.root.join("lib/odk/odk-tool").display()
    )));
}

#[test]
fn the_current_directory_is_never_searched_whatever_path_says() {
    let w = Install::new();
    w.program(&w.dir.path().join("odk-tool"), 0);
    for path in ["", ".", ":", "relative", "/usr/bin:."] {
        let o = w.run(path, &["tool"]);
        assert_eq!(o.status.code(), Some(127), "PATH={path:?}: {}", err(&o));
    }
}

#[test]
fn a_directory_is_not_a_program_and_a_missing_name_suggests_a_near_one() {
    let w = Install::new();
    fs::create_dir(w.root.join("lib/odk/odk-dir")).unwrap();
    w.program(&w.root.join("lib/odk/odk-oma"), 0);
    w.program(&w.root.join("lib/odk/odk-tool"), 0);
    assert_eq!(w.run("/usr/bin", &["dir"]).status.code(), Some(127));
    let o = w.run("/usr/bin", &["tol"]);
    assert_eq!(o.status.code(), Some(127));
    let e = err(&o);
    assert!(e.contains("Did you mean: odk tool?"), "{e}");
    assert!(e.contains("Bundle commands are `odk oma <command>`"), "{e}");
    assert!(e.contains("odk --list"), "{e}");
}

#[test]
fn a_program_that_cannot_be_executed_is_126_and_named() {
    let w = Install::new();
    let private = w.root.join("lib/odk/odk-oma");
    w.program(&private, 0);
    fs::set_permissions(&private, fs::Permissions::from_mode(0o644)).unwrap();
    let o = w.run("/usr/bin", &["oma"]);
    assert_eq!(o.status.code(), Some(126));
    assert!(err(&o).contains(private.to_str().unwrap()), "{}", err(&o));
}

#[test]
fn usage_errors_are_2_and_run_nothing() {
    let w = Install::new();
    let marker = w.root.join("lib/odk/odk-oma");
    w.program(&marker, 0);
    for args in [
        &["--nope"][..],
        &["../odk-oma"],
        &["/usr/lib/odk/odk-oma"],
        &["Oma"],
        &["--version", "x"],
        &["help", "a", "b"],
    ] {
        let o = w.run("/usr/bin", args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", err(&o));
        assert!(err(&o).starts_with("odk: "), "{args:?}: {}", err(&o));
        assert!(out(&o).is_empty());
    }
}

#[test]
fn version_help_and_list_do_not_run_anything_and_list_is_in_byte_order() {
    let w = Install::new();
    w.program(&w.root.join("lib/odk/odk-oma"), 9);
    w.program(&w.root.join("lib/odk/odk-b"), 9);
    w.program(&w.root.join("lib/odk/odk-a"), 9);
    let o = w.run("/usr/bin", &["--version"]);
    assert!(o.status.success());
    assert!(out(&o).starts_with("odk "), "{}", out(&o));

    let o = w.run("/usr/bin", &["--list"]);
    assert!(o.status.success());
    let lines: Vec<String> = out(&o).lines().map(str::to_owned).collect();
    let dir = w.root.join("lib/odk");
    assert_eq!(
        lines,
        [
            format!("a\t{}", dir.join("odk-a").display()),
            format!("b\t{}", dir.join("odk-b").display()),
            format!("oma\t{}", dir.join("odk-oma").display()),
        ]
    );

    let o = w.run("/usr/bin", &[]);
    assert!(o.status.success());
    let h = out(&o);
    assert!(
        h.contains("usage: odk <command>") && h.contains("odk oma"),
        "{h}"
    );
    assert!(h.contains("  oma\n"), "{h}");
}

#[test]
fn help_for_a_command_is_that_command_with_help() {
    let w = Install::new();
    w.program(&w.root.join("lib/odk/odk-oma"), 0);
    let o = w.run("/usr/bin", &["help", "oma"]);
    assert!(o.status.success());
    assert!(out(&o).contains("arg=[--help]"), "{}", out(&o));
}

#[test]
fn in_a_cargo_directory_the_sibling_is_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let target = fs::canonicalize(dir.path()).unwrap().join("debug");
    fs::create_dir_all(&target).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_odk"), target.join("odk")).unwrap();
    let w = Install::new();
    w.program(&target.join("odk-oma"), 0);
    let o = (0..50)
        .find_map(|_| {
            let r = Command::new(target.join("odk"))
                .args(["oma", "--version"])
                .env_clear()
                .output();
            match r {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    None
                }
                other => Some(other.unwrap()),
            }
        })
        .expect("the odk file stayed busy");
    assert!(o.status.success());
    assert!(out(&o).contains("arg=[--version]"), "{}", out(&o));
}
