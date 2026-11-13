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
    w.install_file_with("", Some(DAEMON));
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

/// With no daemon on the socket, `odk oma run` starts one unit for the Bundle's own program with
/// the socket as a socket unit and a slice of the session, and the application goes on.
#[test]
fn without_a_daemon_oma_starts_one_unit_and_the_application_goes_on() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install_file_with("", Some(DAEMON));
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
    // The program is the Bundle's own, written out beside the pair's data, outside the
    // sandbox's /app (ADR 0084).
    let dashes = args.iter().position(|a| a == "--").unwrap();
    let program = Path::new(&args[dashes + 1]);
    assert_eq!(
        program,
        w.data.parent().unwrap().join("daemon/d"),
        "{args:?}"
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
    w.install_file_with("", Some(DAEMON));
    w.control("daemon-socket", "probe/d.sock");
    // `systemd-run` is not on this PATH at all.
    let (out, report) = w.run_wait_with_path("/nonexistent");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("daemon cannot be started"), "{err}");
    assert!(report.is_empty(), "the application never started");
}

/// A daemon that is not asked for does not appear, and a record without it grants none: a
/// Bundle that adds a daemon in an update is asked again, and nothing starts until then.
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
    // The installed file now declares a daemon, as after an upgrade; the record does not
    // cover it.
    w.install_file_with("", Some(DAEMON));
    w.set_consent("");
    let fakes = w.fake_systemd_run();
    let (out, report) = w.run_wait_with_path(&format!("{}:/usr/bin", fakes.display()));
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no consent was recorded"), "{err}");
    assert!(report.is_empty(), "the application never started");
    assert!(!fakes.join("argv").exists(), "no daemon was started");
}

/// A Bundle file's daemon runs from its program written out of the verified image into the
/// pair's own directory, never from the image or `/app` (ADR 0084).
#[test]
fn a_bundle_files_daemon_runs_from_its_program_written_out_of_the_image() {
    if !supported() {
        return;
    }
    let w = World::new();
    let data = w.install_file_with("", Some(DAEMON));
    // The probe reads its instructions from its data directory, which is the pair's.
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("daemon-socket"), "probe/d.sock").unwrap();
    let fakes = w.fake_systemd_run();
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
    assert_eq!(report["daemon_socket"], "pong", "{err}");

    let args: Vec<String> = fs::read_to_string(&argv)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let dashes = args.iter().position(|a| a == "--").unwrap();
    let program = Path::new(&args[dashes + 1]);
    // `data` is `apps/<root>/<app>/data`; the program is beside it, in `daemon/`.
    assert_eq!(program, data.parent().unwrap().join("daemon/d"), "{args:?}");
    assert_eq!(
        fs::read(program).unwrap(),
        fs::read(probe_binary()).unwrap()
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(program).unwrap().permissions().mode() & 0o777,
        0o755
    );
}
