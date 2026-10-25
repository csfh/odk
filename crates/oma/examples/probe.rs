//! A test program that reports what it can see and do from inside the sandbox. The
//! integration tests bundle it and read its output; it is not part of `oma`.

use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

fn line(key: &str, value: impl std::fmt::Display) {
    println!("{key}={value}");
}

fn errno(r: libc::c_long) -> String {
    if r >= 0 {
        "ok".into()
    } else {
        std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap()
            .to_string()
    }
}

fn status(field: &str) -> String {
    fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with(field))
        .map(|l| l[field.len()..].trim().to_owned())
        .unwrap_or_default()
}

fn main() {
    let home = std::env::var("HOME").unwrap_or_default();
    // Control files the tests leave in the application's data directory.
    let control = |name: &str| fs::read_to_string(Path::new(&home).join(name)).ok();

    line("pid", std::process::id());
    line("uid", unsafe { libc::getuid() });
    line("home", &home);
    line(
        "cwd",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
    let mut env: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    env.sort();
    line("env", env.join(","));
    for key in ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "OMA_INBOX", "LANG"] {
        line(
            &format!("env.{key}"),
            std::env::var(key).unwrap_or_default(),
        );
    }

    let procs = fs::read_dir("/proc")
        .map(|d| {
            d.flatten()
                .filter(|e| e.file_name().to_string_lossy().parse::<u32>().is_ok())
                .count()
        })
        .unwrap_or(0);
    line("procs", procs);
    // Landlock denies listing `/`, so ask about the usual names one by one.
    let root: Vec<&str> = [
        "app", "bin", "boot", "dev", "etc", "home", "inbox", "lib", "lib64", "mnt", "opt", "proc",
        "root", "run", "sbin", "srv", "sys", "tmp", "usr", "var",
    ]
    .into_iter()
    .filter(|name| Path::new("/").join(name).exists())
    .collect();
    line("root", root.join(","));

    line("cap_eff", status("CapEff:"));
    line("cap_prm", status("CapPrm:"));
    line("cap_bnd", status("CapBnd:"));
    line("cap_amb", status("CapAmb:"));
    line("no_new_privs", status("NoNewPrivs:"));
    line("seccomp", status("Seccomp:"));

    line(
        "write_usr",
        fs::write("/usr/oma-probe", "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "write_root",
        fs::write("/oma-probe", "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "write_app",
        fs::write("/app/oma-probe", "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    // Omarchy's text size and font, as oma's copy shows them (ADR 0044).
    let flat = |path: &str| {
        fs::read_to_string(Path::new(&home).join(path))
            .map(|t| t.replace('\n', "|"))
            .unwrap_or_else(|_| "none".into())
    };
    line("omarchy_shell", flat(".config/omarchy/shell.toml"));
    line("omarchy_fonts", flat(".config/fontconfig/fonts.conf"));
    // What fontconfig, as GPUI's font lookup uses it, makes of `monospace` in here.
    line(
        "fc_monospace",
        std::process::Command::new("/usr/bin/fc-match")
            .args(["monospace", "-f", "%{family[0]}"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_else(|e| format!("error: {e}")),
    );
    line(
        "omarchy_siblings",
        Path::new(&home).join(".config/omarchy/shell.json").exists(),
    );
    line(
        "omarchy_write",
        fs::write(Path::new(&home).join(".config/omarchy/shell.toml"), "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "write_home",
        fs::write(Path::new(&home).join("written-by-probe"), "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "write_tmp",
        fs::write("/tmp/oma-probe", "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "mount",
        errno(unsafe { libc::syscall(libc::SYS_mount, 0, 0, 0, 0, 0) }),
    );
    line(
        "unshare_user",
        errno(unsafe { libc::unshare(libc::CLONE_NEWUSER) } as libc::c_long),
    );
    line(
        "ptrace",
        errno(unsafe { libc::syscall(libc::SYS_ptrace, libc::PTRACE_ATTACH, 1, 0, 0) }),
    );
    line("read_etc_shadow", fs::read_to_string("/etc/shadow").is_ok());
    line("read_etc_passwd", fs::read_to_string("/etc/passwd").is_ok());
    line("read_proc1_environ", fs::read("/proc/1/environ").is_ok());
    line("exists_dev_dri", Path::new("/dev/dri").exists());
    line("exists_dev_sda", Path::new("/dev/sda").exists());
    line("exists_sys", Path::new("/sys/class").exists());
    line("exists_sys_net", Path::new("/sys/class/net").exists());
    line("exists_var", Path::new("/var/lib").exists());
    line(
        "ls_run_user",
        fs::read_dir(format!("/run/user/{}", unsafe { libc::getuid() }))
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default(),
    );
    for name in ["secret", "Documents", "Music"] {
        line(&format!("see_{name}"), Path::new(&home).join(name).exists());
    }
    line(
        "read_documents",
        fs::read_dir(Path::new(&home).join("Documents"))
            .map(|d| d.count().to_string())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "write_documents",
        fs::write(Path::new(&home).join("Documents/probe"), "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    line(
        "write_music",
        fs::write(Path::new(&home).join("Music/probe"), "x")
            .map(|()| "ok".to_owned())
            .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
    );
    if let Some(name) = control("abstract-socket") {
        use std::os::linux::net::SocketAddrExt;
        let result = std::os::unix::net::SocketAddr::from_abstract_name(name.trim().as_bytes())
            .and_then(|a| UnixStream::connect_addr(&a));
        line(
            "abstract_connect",
            result
                .map(|_| "ok".to_owned())
                .unwrap_or_else(|e| e.raw_os_error().unwrap_or(0).to_string()),
        );
    }
    // A daemon's socket under the runtime directory (ADR 0050): say `ping`, report the answer or
    // the errno. The test's listener answers `pong`.
    if let Some(relative) = control("daemon-socket") {
        use std::io::{BufRead, BufReader};
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
        let answer = UnixStream::connect(Path::new(&runtime).join(relative.trim())).and_then(
            |mut stream| {
                stream.write_all(b"ping\n")?;
                let mut reply = String::new();
                BufReader::new(stream).read_line(&mut reply)?;
                Ok(reply.trim().to_owned())
            },
        );
        line(
            "daemon_socket",
            answer.unwrap_or_else(|e| format!("errno {}", e.raw_os_error().unwrap_or(0))),
        );
        // What else is in the socket's directory: only the socket, if only it was bound.
        let parent = Path::new(&runtime).join(relative.trim());
        line(
            "daemon_dir",
            parent
                .parent()
                .and_then(|dir| fs::read_dir(dir).ok())
                .map(|d| {
                    let mut names: Vec<_> = d
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect();
                    names.sort();
                    names.join(",")
                })
                .unwrap_or_default(),
        );
    }
    line(
        "exe",
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
    std::io::stdout().flush().unwrap();

    // A database in the data directory, as an application's would be: WAL mode, which needs the
    // directory to hold its side files and a mapped file, a transaction, and a sort with a cache
    // too small to hold it, which needs somewhere to put a temporary file. Run with the system's
    // `sqlite3`, so this shows what the sandbox allows SQLite, not what ODK's own build does.
    if control("sqlite").is_some() {
        let sql = "PRAGMA journal_mode=WAL;\n\
            PRAGMA cache_size=-64;\n\
            PRAGMA temp_store=FILE;\n\
            CREATE TABLE t(n INTEGER, s TEXT);\n\
            BEGIN;\n\
            WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM c WHERE x<100000)\n\
              INSERT INTO t SELECT x, hex(randomblob(32)) FROM c;\n\
            COMMIT;\n\
            SELECT count(*) FROM (SELECT s FROM t ORDER BY s);\n\
            PRAGMA integrity_check;\n";
        let result = std::process::Command::new("/usr/bin/sqlite3")
            .arg(Path::new(&home).join("probe.db"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                child.stdin.take().unwrap().write_all(sql.as_bytes())?;
                child.wait_with_output()
            });
        match result {
            Ok(out) => {
                line("sqlite.status", out.status.code().unwrap_or(-1));
                line(
                    "sqlite.out",
                    String::from_utf8_lossy(&out.stdout).replace('\n', ","),
                );
                line(
                    "sqlite.err",
                    String::from_utf8_lossy(&out.stderr).replace('\n', ","),
                );
            }
            Err(e) => line("sqlite.status", format!("cannot run: {e}")),
        }
        std::io::stdout().flush().unwrap();
    }

    // Control behaviour for the tests.
    if control("wait-for-term").is_some() {
        extern "C" fn on_term(_: libc::c_int) {
            let _ = fs::write(std::env::var("HOME").unwrap() + "/got-term", "yes");
            unsafe { libc::_exit(0) };
        }
        unsafe {
            libc::signal(
                libc::SIGTERM,
                on_term as extern "C" fn(libc::c_int) as libc::sighandler_t,
            )
        };
        fs::write(Path::new(&home).join("ready"), "yes").unwrap();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    if control("orphan").is_some() {
        // Leave a child behind: the init must end it when this process exits.
        let _ = std::process::Command::new("/usr/bin/sleep")
            .arg("4242")
            .spawn();
        fs::write(Path::new(&home).join("ready"), "yes").unwrap();
    }
    let code = control("exit-code")
        .and_then(|c| c.trim().parse().ok())
        .unwrap_or(0);
    std::process::exit(code);
}
