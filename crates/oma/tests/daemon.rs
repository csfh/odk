//! End-to-end tests of a Bundle's daemon (ADR 0050) in the real sandbox, under a temporary home
//! and runtime directory. The daemon here is a listener in the test, and the user manager is a
//! fake `systemd-run` that records what it was asked: no test starts a unit in the session it
//! runs in.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

mod common;

use common::*;

const DAEMON: &str = "executable = \"bin/d\"\nargs = [\"daemon\"]\nsocket = \"probe/d.sock\"";

/// Answers every connection's first line with `pong`, until the listener is dropped.
fn serve(socket: &Path) {
    fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let listener = UnixListener::bind(socket).unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            let mut reader = BufReader::new(&stream);
            if reader.read_line(&mut line).is_ok() {
                let _ = (&stream).write_all(b"pong\n");
            }
        }
    });
}

fn wait_for_file(path: &Path) -> String {
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        if let Ok(text) = fs::read_to_string(path) {
            return text;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("{} never appeared", path.display());
}

/// The application reaches its daemon through the one bound file, and sees nothing else of the
/// runtime directory around it. This is also the measurement the ADR rests on: that a pathname
/// `connect()` needs no Landlock right beyond the private `/run`'s.
#[test]
fn the_application_reaches_its_daemons_socket_and_nothing_beside_it() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install_with_daemon("", DAEMON);
    serve(&w.runtime.join("probe/d.sock"));
    fs::write(w.runtime.join("probe/cache.json"), "{}").unwrap();
    let other = UnixListener::bind(w.runtime.join("other.sock")).unwrap();
    w.control("daemon-socket", "probe/d.sock");

    // The fake manager is on PATH, and is not called: the socket is served already.
    let fakes = w.fake_systemd_run();
    let (out, report) = w.run_wait_with_path(&format!("{}:/usr/bin", fakes.display()));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");

    assert_eq!(
        report.get("daemon_socket").map(String::as_str),
        Some("pong"),
        "{err}\n{report:?}"
    );
    assert_eq!(
        report["daemon_dir"], "d.sock",
        "only the socket is in its directory"
    );
    let top: Vec<_> = report["ls_run_user"].split(',').collect();
    assert!(
        top.contains(&"probe") && top.contains(&"wayland-test") && !top.contains(&"other.sock"),
        "{top:?}"
    );
    assert!(
        !fakes.join("argv").exists(),
        "a daemon that is served already is not started a second time"
    );
    drop(other);
}

/// With no daemon on the socket, `oma run` starts one unit for the Bundle's own program with
/// the socket as a socket unit and a slice of the session, and the application goes on.
#[test]
fn without_a_daemon_oma_starts_one_unit_and_the_application_goes_on() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.install_with_daemon("", DAEMON);
    w.control("daemon-socket", "probe/d.sock");
    let fakes = w.fake_systemd_run();
    // What the user manager would do: the socket appears once the unit is asked for.
    let socket = w.runtime.join("probe/d.sock");
    let argv = fakes.join("argv");
    let waiter = thread::spawn({
        let (argv, socket) = (argv.clone(), socket.clone());
        move || {
            wait_for_file(&argv);
            serve(&socket);
        }
    });

    let (out, report) = w.run_wait_with_path(&format!("{}:/usr/bin", fakes.display()));
    waiter.join().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(err.contains("started the application's daemon"), "{err}");
    assert_eq!(report["daemon_socket"], "pong", "{err}");

    let args: Vec<String> = fs::read_to_string(&argv)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(args.contains(&"--user".to_owned()), "{args:?}");
    assert!(args.contains(&format!("--unit=oma-{ID}")), "{args:?}");
    assert!(
        args.contains(&format!(
            "--socket-property=ListenStream={}",
            socket.display()
        )),
        "{args:?}"
    );
    // The program is the Bundle's own, by its real path, outside the sandbox's /app.
    let dashes = args.iter().position(|a| a == "--").unwrap();
    let program = Path::new(&args[dashes + 1]);
    assert!(program.ends_with("Probe.oma/bin/d"), "{program:?}");
    assert!(
        program.starts_with(
            fs::canonicalize(&bundle)
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
        ) || program.starts_with(&w.home),
        "{program:?}"
    );
    assert_eq!(args[dashes + 2], "daemon");
    assert!(
        args.iter().any(|a| a.starts_with("--setenv=PATH=")),
        "the session's PATH goes on to the daemon"
    );
    assert!(
        args.iter().all(|a| !a.contains("SECRET_HOST_VARIABLE")),
        "only an allowlist of the session does: {args:?}"
    );
}

/// When the unit cannot be started, nothing starts: the application would have no daemon the
/// user agreed to and no way to say so.
#[test]
fn a_unit_that_cannot_be_started_stops_the_launch() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install_with_daemon("", DAEMON);
    w.control("daemon-socket", "probe/d.sock");
    // `systemd-run` is not on this PATH at all.
    let (out, report) = w.run_wait_with_path("/nonexistent");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("daemon cannot be started"), "{err}");
    assert!(report.is_empty(), "the application never started");
}

/// A daemon that is not asked for does not appear, and a record without it grants none: a
/// Bundle that adds a daemon in an update is asked again.
#[test]
fn a_bundle_that_adds_a_daemon_is_asked_again() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    assert!(
        !fs::read_to_string(w.grant_file())
            .unwrap()
            .contains("daemon"),
        "a record of a Bundle without one has none"
    );
    // The same Identifier now declares a daemon; the old record does not cover it.
    // `bundle` makes its Bundle where `install` did, so the installed one steps aside.
    fs::rename(w.home.join("out"), w.home.join("out-installed")).unwrap();
    let bundle = w.bundle("");
    let manifest = bundle.join("manifest.toml");
    let mut text = fs::read_to_string(&manifest).unwrap();
    text.push_str(&format!("\n[daemon]\n{DAEMON}\n"));
    fs::write(&manifest, text).unwrap();
    fs::create_dir_all(bundle.join("bin")).unwrap();
    fs::copy(probe_binary(), bundle.join("bin/d")).unwrap();
    let out = w.oma(&["run", bundle.to_str().unwrap()]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("oma-consent") || err.contains("no consent was recorded"),
        "{err}"
    );
}

/// The consent covers the installed program. A copy of the Bundle elsewhere, whether the
/// application wrote it into its own data directory or it was downloaded, would otherwise have
/// its `bin/d` started outside the sandbox with the installed one's consent and no question.
#[test]
fn a_copy_of_an_installed_bundle_never_starts_its_own_daemon() {
    if !supported() {
        return;
    }
    let w = World::new();
    let installed = w.install_with_daemon("", DAEMON);
    w.control("daemon-socket", "probe/d.sock");

    let copy = w.home.join("downloads/Probe.oma");
    fs::create_dir_all(copy.parent().unwrap()).unwrap();
    let cp = std::process::Command::new("cp")
        .arg("-a")
        .arg(fs::canonicalize(w.home.join("Applications/Probe.oma")).unwrap())
        .arg(&copy)
        .status()
        .unwrap();
    assert!(cp.success());
    // Its daemon is a different program from the one the user agreed to.
    let other = copy.join("bin/d");
    fs::remove_file(&other).unwrap();
    let mut bytes = fs::read(probe_binary()).unwrap();
    bytes.push(0);
    fs::write(&other, bytes).unwrap();
    fs::set_permissions(&other, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert!(installed.exists());

    let fakes = w.fake_systemd_run();
    let path = format!("{}:/usr/bin", fakes.display());
    let (out, report) = w.run_target_with_path(copy.to_str().unwrap(), &path);
    assert!(!out.status.success(), "the copy ran");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("only the installed copy"), "{err}");
    assert!(report.is_empty(), "the application never started");
    assert!(
        !fakes.join("argv").exists(),
        "nothing was started for it: {err}"
    );

    // The installed one still runs, by its Identifier or by its path.
    serve(&w.runtime.join("probe/d.sock"));
    let (out, report) = w.run_target_with_path(ID, &path);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(report.contains_key("pid"));
}
