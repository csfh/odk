//! End-to-end tests of `odk oma run`: a probe program is bundled, installed and run in the real
//! sandbox under a temporary home, and what it reports is checked. Nothing here touches the
//! real home, `~/Applications` or `~/.local/share/oma`.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;

mod common;

use common::*;

fn ok(report: &HashMap<String, String>, key: &str) -> bool {
    report.get(key).map(String::as_str) == Some("ok")
}

#[test]
fn the_probe_sees_only_its_own_small_world() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Processes: the init and the application only, with the application as PID 2.
    assert_eq!(r["procs"], "2", "{r:?}");
    assert_eq!(r["pid"], "2");
    // Identity and place.
    assert_eq!(r["uid"], unsafe { libc::getuid() }.to_string());
    assert_eq!(r["home"], w.home.display().to_string());
    assert_eq!(r["cwd"], r["home"]);
    // The real home is not there; only the data directory is, under the same path.
    assert_eq!(r["see_secret"], "false");
    assert_eq!(r["see_Documents"], "false");
    assert!(
        w.data.join("written-by-probe").exists(),
        "the probe wrote to $HOME, which is its data directory"
    );
    assert!(!w.home.join("written-by-probe").exists());
    // The filesystem.
    assert_eq!(r["root"], "app,bin,dev,etc,lib,lib64,proc,run,sbin,tmp,usr");
    assert_eq!(r["write_usr"], "30", "/usr is read-only");
    assert_eq!(r["write_root"], "30", "/ is read-only");
    assert_eq!(r["write_app"], "30", "the Bundle is read-only");
    assert!(ok(&r, "write_home") && ok(&r, "write_tmp"));
    assert_eq!(r["exists_dev_dri"], "false");
    assert_eq!(r["exists_dev_sda"], "false");
    assert_eq!(r["exists_sys"], "false");
    assert_eq!(r["exists_var"], "false");
    assert_eq!(r["read_etc_shadow"], "false");
    assert_eq!(r["read_etc_passwd"], "true");
    assert_eq!(r["ls_run_user"], "wayland-test");
    // The environment is rebuilt, not inherited.
    assert!(!r["env"].contains("SECRET_HOST_VARIABLE"), "{}", r["env"]);
    assert_eq!(r["env.WAYLAND_DISPLAY"], "wayland-test");
    assert_eq!(
        r["env.XDG_RUNTIME_DIR"],
        format!("/run/user/{}", unsafe { libc::getuid() })
    );
    assert_eq!(r["env.LANG"], "en_US.UTF-8");
    // Privileges: none, and no way to get them back.
    for cap in ["cap_eff", "cap_prm", "cap_amb"] {
        assert_eq!(r[cap], "0000000000000000", "{cap}");
    }
    assert_eq!(r["cap_bnd"], "0000000000000000");
    assert_eq!(r["no_new_privs"], "1");
    assert_eq!(r["seccomp"], "2");
    assert_eq!(r["mount"], "1", "mount returns EPERM");
    assert_eq!(
        r["unshare_user"], "28",
        "no nested user namespaces (ENOSPC)"
    );
    assert_eq!(r["ptrace"], "1");
    assert_eq!(r["read_proc1_environ"], "false", "the init is not readable");
    // The path the host installed `oma` at is not visible, and neither is the probe's own.
    assert_eq!(r["exe"], "/app/probe");
}

#[test]
fn consent_is_required_and_fails_closed() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("gpu = true");
    fs::remove_file(
        w.home
            .join(".local/share/oma/grants")
            .join(format!("{ID}.toml")),
    )
    .unwrap();
    // The test `oma` has no consent dialog helper next to it, so there is no way to ask.
    let out = w.oma(&["run", ID]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("oma-consent") && err.contains("--accept-permissions"),
        "{err}"
    );
    assert!(
        !w.data.exists() || fs::read_dir(&w.data).unwrap().count() == 0,
        "nothing was started"
    );
}

#[test]
fn named_directories_follow_the_granted_access() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("[[permissions.filesystem]]\ndir = \"documents\"\naccess = \"write\"\n\n[[permissions.filesystem]]\ndir = \"music\"\naccess = \"read\"");
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(r["see_Documents"], "true");
    assert_eq!(r["read_documents"], "1");
    assert!(ok(&r, "write_documents"));
    assert!(
        w.home.join("Documents/probe").exists(),
        "the write reached the real directory"
    );
    assert_eq!(r["write_music"], "30", "read-only");
    assert_eq!(r["see_secret"], "false");
}

#[test]
fn the_gpu_permission_brings_the_devices_and_only_a_slice_of_sys() {
    if !supported() {
        return;
    }
    if !Path::new("/dev/dri").exists() {
        return;
    }
    let w = World::new();
    w.install("gpu = true");
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(r["exists_dev_dri"], "true");
    assert_eq!(
        r["exists_sys"], "false",
        "/sys/class stays hidden; only the GPU's own device directory is shown"
    );
    assert_eq!(
        r["exists_sys_net"], "false",
        "no network interface addresses"
    );
}

#[test]
fn network_apps_share_the_network_but_cannot_reach_outside_abstract_sockets() {
    if !supported() {
        return;
    }
    use std::os::linux::net::SocketAddrExt;
    let w = World::new();
    let name = format!("oma-itest-{}", std::process::id());
    let _outside = UnixListener::bind_addr(
        &std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap(),
    )
    .unwrap();
    w.install("network = true");
    w.control("abstract-socket", &name);
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        r["abstract_connect"], "1",
        "Landlock scoping refuses it with EPERM"
    );

    // Without `network` the application has its own network namespace, where the host's
    // abstract sockets do not exist.
    let w = World::new();
    w.install("");
    w.control("abstract-socket", &name);
    let (_, r) = w.run_wait();
    assert_eq!(
        r["abstract_connect"], "111",
        "ECONNREFUSED: not there at all"
    );
}

#[test]
fn the_applications_exit_status_passes_through() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    w.control("exit-code", "7");
    let out = w.oma(&["run", "--wait", ID]);
    assert_eq!(out.status.code(), Some(7));
}

#[test]
fn sigterm_reaches_the_application_and_the_status_is_128_plus_the_signal() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    w.control("wait-for-term", "yes");
    let mut child = retrying(|| {
        Command::new(&w.oma_bin)
            .args(["run", "--wait", ID])
            .env_clear()
            .env("HOME", &w.home)
            .env("XDG_RUNTIME_DIR", &w.runtime)
            .env("WAYLAND_DISPLAY", "wayland-test")
            .stdout(std::process::Stdio::null())
            .spawn()
    });
    wait_for(&w.data.join("ready"));
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let status = child.wait().unwrap();
    assert_eq!(
        fs::read_to_string(w.data.join("got-term")).unwrap(),
        "yes",
        "the signal reached the application"
    );
    assert_eq!(
        status.code(),
        Some(0),
        "the application handled it and exited 0"
    );
}

/// A database in the application's own data directory works inside the sandbox: WAL, a
/// transaction, and a sort that spills to `/tmp` (ADR 0049). It is the system's `sqlite3` that is
/// run, so this shows what the sandbox allows SQLite, not what ODK's own build does; the
/// sandbox's seccomp is a denylist and its Landlock grants the data directory and `/tmp`.
#[test]
fn a_database_in_the_data_directory_works_inside_the_sandbox() {
    if !supported() || !Path::new("/usr/bin/sqlite3").exists() {
        eprintln!("skipped: needs the sandbox and /usr/bin/sqlite3");
        return;
    }
    let w = World::new();
    w.install("");
    w.control("sqlite", "yes");
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(r["sqlite.status"], "0", "{}", r["sqlite.err"]);
    assert_eq!(r["sqlite.err"], "");
    // The journal mode it switched to, the rows that were sorted, and a sound database.
    assert_eq!(r["sqlite.out"], "wal,100000,ok,");
    assert!(
        w.data.join("probe.db").exists(),
        "kept in the data directory"
    );
}

/// A field of `/proc/<pid>/stat` counted from the one after the command name: 0 is the state,
/// 1 the parent, 2 the process group and 3 the session.
fn stat_field(pid: i32, index: usize) -> i32 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let after = &stat[stat.rfind(')').unwrap() + 2..];
    after
        .split_whitespace()
        .nth(index)
        .unwrap()
        .parse()
        .unwrap()
}

/// The host pid of the application: the init's only child.
fn find_application(w: &World) -> i32 {
    let init = find_init(w);
    let children =
        fs::read_to_string(format!("/proc/{init}/task/{init}/children")).unwrap_or_default();
    children
        .split_whitespace()
        .next()
        .and_then(|p| p.parse().ok())
        .expect("the init has no child")
}

fn ancestors(mut pid: i32) -> Vec<i32> {
    let mut found = Vec::new();
    while pid > 1 {
        pid = stat_field(pid, 1);
        found.push(pid);
    }
    found
}

#[test]
fn the_application_is_never_a_child_or_descendant_of_oma_even_when_waiting() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    w.control("wait-for-term", "yes");
    let mut oma = retrying(|| {
        Command::new(&w.oma_bin)
            .args(["run", "--wait", ID])
            .env_clear()
            .env("HOME", &w.home)
            .env("XDG_RUNTIME_DIR", &w.runtime)
            .env("WAYLAND_DISPLAY", "wayland-test")
            .stdout(std::process::Stdio::null())
            .spawn()
    });
    wait_for(&w.data.join("ready"));
    let oma_pid = oma.id() as i32;
    let init = find_init(&w);
    let app = find_application(&w);

    assert_ne!(stat_field(init, 1), oma_pid, "oma is not the init's parent");
    assert!(
        !ancestors(init).contains(&oma_pid),
        "oma is not an ancestor of the init"
    );
    assert!(
        !ancestors(app).contains(&oma_pid),
        "oma is not an ancestor of the application"
    );
    assert_eq!(stat_field(init, 3), init, "the init leads its own session");
    assert_ne!(
        stat_field(init, 3),
        stat_field(oma_pid, 3),
        "and it is not oma's session"
    );

    unsafe { libc::kill(oma_pid, libc::SIGTERM) };
    assert_eq!(
        oma.wait().unwrap().code(),
        Some(0),
        "the status still comes back"
    );
}

#[test]
fn when_the_application_exits_everything_in_its_namespace_ends() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    w.control("orphan", "yes");
    let (out, _) = w.run_wait();
    assert!(out.status.success());
    // The sleep the probe left behind was in the namespace, so it died with the init.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let leftovers = Command::new("pgrep")
        .args(["-f", "sleep 4242"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&leftovers.stdout).trim().is_empty(),
        "a process outlived its sandbox"
    );
}

#[test]
fn a_detached_run_returns_at_once_and_the_application_goes_on() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    w.control("wait-for-term", "yes");
    // The application keeps the stdout it inherited, so do not wait on a pipe for it.
    let status = retrying(|| {
        Command::new(&w.oma_bin)
            .args(["run", ID])
            .env_clear()
            .env("HOME", &w.home)
            .env("XDG_RUNTIME_DIR", &w.runtime)
            .env("WAYLAND_DISPLAY", "wayland-test")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    });
    assert!(
        status.success(),
        "oma run returns as soon as the application has started"
    );
    wait_for(&w.data.join("ready"));

    // `oma` is no longer in the picture: the init was left to the host and is not our child.
    let init = find_init(&w);
    assert_ne!(parent_of(init), std::process::id() as i32);
    // Stopping it from outside the way a user would: the signal reaches the application.
    unsafe { libc::kill(init, libc::SIGTERM) };
    wait_for(&w.data.join("got-term"));
}

/// The pid of the `oma --init` whose configuration mentions this test's home.
fn find_init(w: &World) -> i32 {
    let out = Command::new("pgrep")
        .args(["-f", "oma --init"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<i32>().ok())
        .find(|pid| {
            fs::read_to_string(format!("/proc/{pid}/cmdline"))
                .is_ok_and(|c| c.contains(&w.home.display().to_string()))
        })
        .expect("no init is running for this test")
}

fn parent_of(pid: i32) -> i32 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    // The command name may contain spaces, so count from the closing parenthesis.
    let after = &stat[stat.rfind(')').unwrap() + 2..];
    after.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn setup_errors_reach_the_terminal_even_when_detached() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    // No display: the plan refuses before anything starts.
    let out = retrying(|| {
        Command::new(&w.oma_bin)
            .args(["run", ID])
            .env_clear()
            .env("HOME", &w.home)
            .env("XDG_RUNTIME_DIR", &w.runtime)
            .output()
    });
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("WAYLAND_DISPLAY"));

    // A Bundle that passes validation but whose program cannot start (a bare ELF header): the
    // failure comes back over the pipe, so a detached `odk oma run` still reports it.
    let bundle = w.home.join("Applications/Probe.oma");
    let mut header = vec![0u8; 64];
    header[..4].copy_from_slice(b"\x7fELF");
    header[4] = 2;
    header[5] = 1;
    header[16] = 3;
    header[18] = 62;
    fs::write(bundle.join("probe"), header).unwrap();
    let out = w.oma(&["run", ID]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("cannot start /app/probe"), "{err}");
}

/// What the host says (ADR 0062) reaches the application through the directory of `odkd`'s
/// socket, which oma binds read-only: the application hears `odkd` and cannot change anything
/// there, and it sees none of Omarchy's own files. Without `odkd` there is no such directory.
#[test]
fn the_application_hears_odkd_through_a_read_only_directory_and_sees_none_of_omarchys_files() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("");
    fs::create_dir_all(w.home.join(".config/omarchy")).unwrap();
    fs::write(
        w.home.join(".config/omarchy/shell.toml"),
        "[font]\nbase-size = 14\n",
    )
    .unwrap();

    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(r["odkd_ls"], "none", "no odkd, no directory: {r:?}");
    assert_eq!(
        r["config_omarchy"], "false",
        "none of Omarchy's files is shown"
    );

    // An odkd of the test's own, as systemd would keep its socket.
    let dir = w.runtime.join("odk");
    fs::create_dir_all(&dir).unwrap();
    let serve = |path: &Path, line: &'static str| {
        let listener = std::os::unix::net::UnixListener::bind(path).unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let _ = writeln!(stream, "{line}");
            }
        })
    };
    serve(
        &dir.join("odkd.sock"),
        "{\"v\":1,\"text\":{\"base_size\":14}}",
    );
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(r["odkd_ls"], "odkd.sock", "{r:?}");
    assert_eq!(
        r["odkd_line"], "{\"v\":1,\"text\":{\"base_size\":14}}",
        "{r:?}"
    );
    assert_ne!(r["odkd_write"], "ok", "the directory is read-only");
    assert_eq!(r["config_omarchy"], "false");

    // A socket made again at the same path is the one the next application hears: the
    // directory is bound, not the socket.
    fs::remove_file(dir.join("odkd.sock")).unwrap();
    serve(
        &dir.join("odkd.sock"),
        "{\"v\":1,\"text\":{\"base_size\":16}}",
    );
    let (_, r) = w.run_wait();
    assert_eq!(
        r["odkd_line"], "{\"v\":1,\"text\":{\"base_size\":16}}",
        "{r:?}"
    );
}

#[test]
fn an_installed_bundle_file_runs_from_a_read_only_tmpfs_at_app() {
    if !supported() {
        return;
    }
    let w = World::new();
    let data = w.install_file("");
    let (out, r) = w.run_wait();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(r["exe"], "/app/probe");
    assert_eq!(r["write_app"], "30", "the tree is read-only");
    assert_eq!(r["root"], "app,bin,dev,etc,lib,lib64,proc,run,sbin,tmp,usr");
    // Its data is the pair's (ADR 0060, section 2), not the Identifier's.
    assert!(data.join("written-by-probe").exists());
    assert!(!w.data.join("written-by-probe").exists());

    // Another file under the same name is not the installed one, and does not run.
    let apps = w.home.join("Applications/Probe.oma");
    let mut bytes = fs::read(&apps).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&apps, bytes).unwrap();
    let (out, _) = w.run_wait();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("payload digest"), "{err}");
}
