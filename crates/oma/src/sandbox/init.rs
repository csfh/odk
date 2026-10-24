//! The init of ADR 0024: PID 1 of an application's PID namespace, which is `oma` re-executed.
//!
//! It does five things and no policy: reap orphans, forward signals to the application, exit
//! with the application's status, exit when the application exits, and finish the sandbox
//! (the `/proc` mount happened just before it started; it blocks nested user namespaces,
//! applies Landlock and seccomp to the application, and drops capabilities, then its own).

use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::Write;
use std::os::fd::{FromRawFd, RawFd};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::landlock::{self, Rights};
use super::seccomp;

/// What the init is told. It arrives as one argument, built by [`crate::sandbox::launch`].
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub exe: PathBuf,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub landlock: Vec<(PathBuf, String)>,
    pub scope_abstract_unix_sockets: bool,
    /// Setup errors are written here; it closes when the application starts (end of file).
    pub report_fd: RawFd,
    /// `oma run --wait` reads the application's status from here; -1 when nothing does.
    pub status_fd: RawFd,
}

impl Config {
    pub fn rights(name: &str) -> Option<Rights> {
        match name {
            "read" => Some(Rights::Read),
            "read-exec" => Some(Rights::ReadExec),
            "all" => Some(Rights::All),
            _ => None,
        }
    }

    pub fn rights_name(rights: Rights) -> &'static str {
        match rights {
            Rights::Read => "read",
            Rights::ReadExec => "read-exec",
            Rights::All => "all",
        }
    }
}

/// Every signal the init passes on to the application: all of them except the two that cannot
/// be caught and `SIGCHLD`, which is the init's own business (ADR 0024). Numbers the C library
/// reserves for itself are refused by `sigaddset` and skipped.
fn forwarded_signals() -> Vec<libc::c_int> {
    let mut probe: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe { libc::sigemptyset(&mut probe) };
    (1..=libc::SIGRTMAX())
        .filter(|s| ![libc::SIGKILL, libc::SIGSTOP, libc::SIGCHLD].contains(s))
        .filter(|s| unsafe { libc::sigaddset(&mut probe, *s) } == 0)
        .collect()
}

fn report(fd: RawFd, message: &str) {
    let mut pipe = unsafe { std::fs::File::from_raw_fd(fd) };
    let _ = pipe.write_all(message.as_bytes());
    std::mem::forget(pipe);
}

fn die(fd: RawFd, message: &str) -> ! {
    report(fd, message);
    unsafe { libc::_exit(1) }
}

/// Runs the init. Never returns: it exits with the application's status.
pub fn run(config_text: &str) -> ! {
    let config: Config = match toml::from_str(config_text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("oma: bad init configuration: {e}");
            std::process::exit(1);
        }
    };
    let fd = config.report_fd;

    // Other processes of the same user must not be able to read the init's memory, exe or
    // environment through /proc.
    unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };

    // No further user namespaces, whichever syscall creates them (docs/spec/seccomp.md).
    if let Err(e) = std::fs::write("/proc/sys/user/max_user_namespaces", "0") {
        die(fd, &format!("cannot block nested user namespaces: {e}"));
    }

    // Take signals through a file descriptor, so none is lost or run on a handler.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe { libc::sigemptyset(&mut set) };
    for s in forwarded_signals().into_iter().chain([libc::SIGCHLD]) {
        unsafe { libc::sigaddset(&mut set, s) };
    }
    unsafe { libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) };
    let signals = unsafe { libc::signalfd(-1, &set, libc::SFD_CLOEXEC) };
    if signals < 0 {
        die(
            fd,
            &format!("signalfd: {}", std::io::Error::last_os_error()),
        );
    }

    let app = unsafe { libc::fork() };
    if app < 0 {
        die(fd, &format!("fork: {}", std::io::Error::last_os_error()));
    }
    if app == 0 {
        start_application(&config, fd);
    }

    // The init: it keeps nothing the application could use.
    unsafe { libc::close(fd) };
    drop_capabilities();
    unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };

    loop {
        let mut info: libc::signalfd_siginfo = unsafe { std::mem::zeroed() };
        let n = unsafe {
            libc::read(
                signals,
                (&mut info as *mut libc::signalfd_siginfo).cast(),
                std::mem::size_of::<libc::signalfd_siginfo>(),
            )
        };
        if n < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            unsafe { libc::_exit(1) };
        }
        let signal = info.ssi_signo as libc::c_int;
        if signal != libc::SIGCHLD {
            unsafe { libc::kill(app, signal) };
            continue;
        }
        // Reap everything that has ended; the application's end is ours.
        loop {
            let mut status = 0;
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            if pid == app {
                let code = if libc::WIFEXITED(status) {
                    libc::WEXITSTATUS(status)
                } else {
                    128 + libc::WTERMSIG(status)
                };
                if config.status_fd >= 0 {
                    let bytes = code.to_le_bytes();
                    unsafe { libc::write(config.status_fd, bytes.as_ptr().cast(), 4) };
                }
                unsafe { libc::_exit(code) };
            }
        }
    }
}

/// In the forked child: confine and become the application.
fn start_application(config: &Config, fd: RawFd) -> ! {
    // Undo the init's signal mask before the application inherits it.
    let mut empty: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe { libc::sigemptyset(&mut empty) };
    unsafe { libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut()) };

    // The application holds neither of the init's pipes, except the report pipe, which closes
    // when it starts.
    if config.status_fd >= 0 {
        unsafe { libc::close(config.status_fd) };
    }
    let fail = |what: &str, e: &dyn std::fmt::Display| -> ! {
        report(fd, &format!("{what}: {e}"));
        unsafe { libc::_exit(127) }
    };

    if let Err(e) = std::env::set_current_dir(&config.cwd) {
        fail("cannot enter the working directory", &e);
    }

    // Capabilities first, while the bounding set can still be changed; then the restrictions
    // that need no privilege.
    drop_capabilities();
    let mut rules = Vec::new();
    for (path, rights) in &config.landlock {
        match Config::rights(rights) {
            Some(r) => rules.push((path.clone(), r)),
            None => fail("bad Landlock rule", rights),
        }
    }
    let landlock_config = landlock::Config {
        rules,
        scope_abstract_unix_sockets: config.scope_abstract_unix_sockets,
    };
    if let Err(e) = landlock::restrict(&landlock_config) {
        fail("cannot confine the application", &e);
    }
    if let Err(e) = seccomp::install() {
        fail("cannot install the seccomp filter", &e);
    }

    // The report pipe closes when the exec succeeds, which tells `oma run` the application started.
    unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };

    let exe = CString::new(config.exe.as_os_str().to_str().expect("UTF-8 path")).unwrap();
    let argv = [exe.as_ptr(), std::ptr::null()];
    let env: Vec<CString> = config
        .env
        .iter()
        .map(|(k, v)| CString::new(format!("{k}={v}")).unwrap())
        .collect();
    let mut envp: Vec<*const libc::c_char> = env.iter().map(|e| e.as_ptr()).collect();
    envp.push(std::ptr::null());
    unsafe { libc::execve(exe.as_ptr(), argv.as_ptr(), envp.as_ptr()) };
    let e = std::io::Error::last_os_error();
    fail(&format!("cannot start {}", config.exe.display()), &e)
}

/// Empties the bounding, ambient, inheritable, permitted and effective capability sets.
pub fn drop_capabilities() {
    for cap in 0..64 {
        // Capabilities past the kernel's last one fail with EINVAL, which is fine.
        unsafe { libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0) };
    }
    unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    };

    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    const VERSION_3: u32 = 0x2008_0522;
    let mut header = Header {
        version: VERSION_3,
        pid: 0,
    };
    let data = [Data {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    unsafe { libc::syscall(libc::SYS_capset, &mut header, data.as_ptr()) };
}

/// Makes every capability the process holds survive an exec as a non-root user, so the
/// re-executed init keeps what it needs to finish the sandbox (ADR 0024).
pub fn raise_ambient_capabilities() -> std::io::Result<()> {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    const VERSION_3: u32 = 0x2008_0522;
    let mut header = Header {
        version: VERSION_3,
        pid: 0,
    };
    let mut data = [Data {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    if unsafe { libc::syscall(libc::SYS_capget, &mut header, data.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    for d in &mut data {
        d.inheritable = d.permitted;
    }
    if unsafe { libc::syscall(libc::SYS_capset, &mut header, data.as_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    for cap in 0..64 {
        // Past the kernel's last capability this fails with EINVAL, which is fine.
        unsafe { libc::prctl(libc::PR_CAP_AMBIENT, libc::PR_CAP_AMBIENT_RAISE, cap, 0, 0) };
    }
    Ok(())
}
