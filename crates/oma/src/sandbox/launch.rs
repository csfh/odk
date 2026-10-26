//! `oma run`: from a Bundle to a running, sandboxed application (ADR 0005, ADR 0024).
//!
//! The steps that need no privilege come first (consent, the plan), then the namespaces are
//! created with `clone`, the child builds the new root and re-executes `oma` as the init, and
//! the init starts the application. Setup errors travel back over a pipe that closes when the
//! application has started, so `oma run` reports them even though it does not stay.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::fd::RawFd;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};

use super::init::{self, Config};
use super::landlock;
use super::mounts;
use super::plan::{self, Inputs, Plan};
use crate::fmt::{self, Bundle, Identifier};
use crate::grants;
use crate::install;
use crate::layout::Layout;
use crate::userdirs;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no installed application is called {0}")]
    NotInstalled(Identifier),
    #[error(transparent)]
    Invalid(#[from] fmt::Error),
    #[error(transparent)]
    Grants(#[from] grants::Error),
    #[error("it asks for permissions that have not been granted:\n{}\ngrant them with `oma install --accept-permissions`", .0.iter().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"))]
    ConsentRequired(Vec<String>),
    #[error(transparent)]
    Plan(#[from] plan::Error),
    #[error(transparent)]
    Landlock(#[from] landlock::Error),
    #[error("cannot create the sandbox: {0}")]
    Namespaces(io::Error),
    #[error("the sandbox could not be set up: {0}")]
    Setup(String),
    #[error("cannot access {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// What `oma run` needs to know about the host.
pub struct HostEnv {
    pub uid: u32,
    pub gid: u32,
    pub runtime_dir: Option<PathBuf>,
    pub wayland_display: Option<String>,
}

impl HostEnv {
    pub fn from_process() -> Self {
        Self {
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            runtime_dir: std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute()),
            wayland_display: std::env::var("WAYLAND_DISPLAY")
                .ok()
                .filter(|d| !d.is_empty()),
        }
    }
}

pub struct Options {
    /// Stay, relay terminal signals and return the application's status (ADR 0024).
    pub wait: bool,
    /// Run once (ADR 0018): a private tmpfs for data, no inbox, and no consent record needed,
    /// because the permissions apply to this launch only.
    pub run_once: bool,
}

/// An installed application by Identifier, or a Bundle directory by path.
pub fn resolve(layout: &Layout, target: &str) -> Result<Bundle, Error> {
    match target.parse::<Identifier>() {
        Ok(id) => install::find(layout, &id).ok_or(Error::NotInstalled(id)),
        Err(_) => Ok(Bundle::open(Path::new(target))?),
    }
}

/// What must hold before anyone is asked for consent, so nobody agrees to something that then
/// cannot start: the permissions this build can enforce, and a kernel that can enforce them.
pub fn preflight(bundle: &Bundle) -> Result<(), Error> {
    let permissions = &bundle.manifest().permissions;
    if !permissions.dbus_talk.is_empty() {
        return Err(plan::Error::DbusUnsupported.into());
    }
    // Applications that share the host's network namespace need abstract-socket scoping, or
    // they could reach the session bus without a `dbus` permission (ADR 0005).
    let abi = landlock::abi_version()?;
    if permissions.network && abi < 6 {
        return Err(landlock::Error::ScopingNeedsNewerAbi(abi).into());
    }
    Ok(())
}

/// Runs the Bundle. Returns the application's status under `wait`, and 0 once it has started
/// otherwise. Fails closed when the consent record does not cover what the Bundle asks for.
pub fn run(
    layout: &Layout,
    bundle: &Bundle,
    host: &HostEnv,
    options: &Options,
) -> Result<i32, Error> {
    let manifest = bundle.manifest();
    let id = &manifest.identifier;

    if !options.run_once {
        let granted = grants::load(layout, id)?.unwrap_or_default();
        let missing = grants::missing(&granted, &manifest.permissions);
        if !grants::is_empty(&missing) {
            return Err(Error::ConsentRequired(fmt::permission_lines(&missing)));
        }
    }
    preflight(bundle)?;

    let named_dirs = userdirs::resolve(layout.home(), &layout.user_dirs_file());
    let data_dir = layout.data_dir(id);
    let inbox_dir = layout.inbox_dir(id);
    if !options.run_once {
        private_dir(&data_dir)?;
        if manifest.permissions.updates {
            private_dir(&inbox_dir)?;
        }
    }
    let env = |name: &str| std::env::var(name).ok();
    let plan = Plan::new(&Inputs {
        bundle_root: bundle.root(),
        executable: &manifest.executable,
        permissions: &manifest.permissions,
        data_dir: (!options.run_once).then_some(data_dir.as_path()),
        inbox_dir: (!options.run_once).then_some(inbox_dir.as_path()),
        home: layout.home(),
        uid: host.uid,
        runtime_dir: host.runtime_dir.as_deref(),
        wayland_display: host.wayland_display.as_deref(),
        named_dirs: &named_dirs,
        host_etc: Path::new("/etc"),
        host_root: Path::new("/"),
        host_env: &env,
    })?;
    for (dir, why) in &plan.unavailable {
        eprintln!(
            "odk oma: note: the {} folder is not available ({why:?}), so the application starts without it",
            dir.as_str()
        );
    }

    let runtime = host
        .runtime_dir
        .as_deref()
        .expect("the plan requires a runtime directory");
    spawn(&plan, runtime, host, options.wait)
}

fn private_dir(path: &Path) -> Result<(), Error> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|e| io_error(path, e))
}

fn init_config(plan: &Plan, report_fd: RawFd, status_fd: RawFd) -> Config {
    Config {
        exe: plan.exe.clone(),
        cwd: plan.cwd.clone(),
        env: plan.env.iter().cloned().collect::<BTreeMap<_, _>>(),
        landlock: plan
            .landlock
            .iter()
            .map(|(p, r)| (p.clone(), Config::rights_name(*r).to_owned()))
            .collect(),
        scope_abstract_unix_sockets: plan.network,
        report_fd,
        status_fd,
    }
}

static CHILD: AtomicI32 = AtomicI32::new(0);

extern "C" fn forward_signal(signal: libc::c_int) {
    let child = CHILD.load(Ordering::Relaxed);
    if child > 0 {
        unsafe { libc::kill(child, signal) };
    }
}

fn pipe() -> Result<(RawFd, RawFd), Error> {
    let mut fds = [0; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(Error::Namespaces(io::Error::last_os_error()));
    }
    Ok((fds[0], fds[1]))
}

fn write_file(path: &str, text: &str) -> Result<(), Error> {
    fs::write(path, text).map_err(|e| io_error(Path::new(path), e))
}

fn spawn(plan: &Plan, runtime_dir: &Path, host: &HostEnv, wait: bool) -> Result<i32, Error> {
    let staging = runtime_dir.join(format!("oma-{}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&staging)
        .map_err(|e| io_error(&staging, e))?;
    let result = spawn_in(plan, &staging, host, wait);
    let _ = fs::remove_dir(&staging);
    result
}

fn spawn_in(plan: &Plan, staging: &Path, host: &HostEnv, wait: bool) -> Result<i32, Error> {
    // The init is this same binary, started through a descriptor opened now so that it does
    // not depend on where `oma` is installed being visible inside the sandbox (ADR 0024).
    let exe =
        fs::File::open("/proc/self/exe").map_err(|e| io_error(Path::new("/proc/self/exe"), e))?;
    let (ready_r, ready_w) = pipe()?;
    let (sync_r, sync_w) = pipe()?;
    let (pid_r, pid_w) = pipe()?;
    let (status_r, status_w) = pipe()?;
    let status_fd = if wait { status_w } else { -1 };
    let config = toml::to_string(&init_config(plan, ready_w, status_fd))
        .expect("the init configuration serialises");

    // The application must never be a child of `oma` (ADR 0034), so an intermediate process
    // starts the init and leaves at once. The init is then the host's orphan from the start,
    // in its own session, and `oma` only ever talks to it through pipes and signals.
    let middle = unsafe { libc::fork() };
    if middle < 0 {
        return Err(Error::Namespaces(io::Error::last_os_error()));
    }
    if middle == 0 {
        unsafe {
            libc::close(ready_r);
            libc::close(sync_w);
            libc::close(pid_r);
            libc::close(status_r);
        }
        start_namespaces(
            plan, staging, &exe, sync_r, ready_w, pid_w, status_fd, &config,
        )
    }
    unsafe {
        libc::close(sync_r);
        libc::close(ready_w);
        libc::close(pid_w);
        libc::close(status_w);
    }
    reap(middle);

    let mut pid_bytes = [0u8; 4];
    let got = unsafe { libc::read(pid_r, pid_bytes.as_mut_ptr().cast(), 4) };
    unsafe { libc::close(pid_r) };
    if got != 4 {
        // The intermediate process failed before it had an init; it says why on the other pipe.
        let message = read_all(ready_r);
        unsafe {
            libc::close(ready_r);
            libc::close(sync_w);
            libc::close(status_r);
        }
        return Err(Error::Setup(if message.is_empty() {
            "the sandbox could not be started".into()
        } else {
            message
        }));
    }
    let init = i32::from_le_bytes(pid_bytes);

    // The init cannot do this itself: only a process outside the new user namespace may map
    // another process's ids, and only the caller's own id may be mapped without privilege.
    let maps = write_file(&format!("/proc/{init}/setgroups"), "deny")
        .and_then(|()| {
            write_file(
                &format!("/proc/{init}/uid_map"),
                &format!("{} {} 1\n", host.uid, host.uid),
            )
        })
        .and_then(|()| {
            write_file(
                &format!("/proc/{init}/gid_map"),
                &format!("{} {} 1\n", host.gid, host.gid),
            )
        });
    if let Err(e) = maps {
        unsafe {
            libc::kill(init, libc::SIGKILL);
            libc::close(ready_r);
            libc::close(sync_w);
            libc::close(status_r);
        }
        return Err(e);
    }

    if wait {
        CHILD.store(init, Ordering::Relaxed);
        let relayed = [
            libc::SIGINT,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGQUIT,
            libc::SIGCONT,
            libc::SIGUSR1,
            libc::SIGUSR2,
            libc::SIGWINCH,
        ];
        for signal in relayed {
            unsafe {
                libc::signal(
                    signal,
                    forward_signal as extern "C" fn(libc::c_int) as libc::sighandler_t,
                )
            };
        }
    }
    unsafe {
        libc::write(sync_w, [1u8].as_ptr().cast(), 1);
        libc::close(sync_w);
    }

    let message = read_all(ready_r);
    unsafe { libc::close(ready_r) };
    if !message.is_empty() {
        unsafe { libc::close(status_r) };
        return Err(Error::Setup(message));
    }
    if !wait {
        unsafe { libc::close(status_r) };
        return Ok(0);
    }
    // The init writes the application's status just before it exits (ADR 0024). Nothing at all
    // means it was killed outright.
    let mut code = [0u8; 4];
    let n = read_exactly(status_r, &mut code);
    unsafe { libc::close(status_r) };
    Ok(if n == 4 {
        i32::from_le_bytes(code)
    } else {
        128 + libc::SIGKILL
    })
}

fn read_exactly(fd: RawFd, buf: &mut [u8]) -> usize {
    let mut have = 0;
    while have < buf.len() {
        let n = unsafe { libc::read(fd, buf[have..].as_mut_ptr().cast(), buf.len() - have) };
        if n > 0 {
            have += n as usize;
        } else if n == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            break;
        }
    }
    have
}

/// The intermediate process: create the namespaces with `clone`, tell `oma` the init's pid, and
/// exit, so the init is orphaned at once.
#[allow(clippy::too_many_arguments)]
fn start_namespaces(
    plan: &Plan,
    staging: &Path,
    exe: &fs::File,
    sync_r: RawFd,
    ready_w: RawFd,
    pid_w: RawFd,
    status_fd: RawFd,
    config: &str,
) -> ! {
    let mut flags =
        libc::CLONE_NEWUSER | libc::CLONE_NEWNS | libc::CLONE_NEWPID | libc::CLONE_NEWIPC;
    if !plan.network {
        flags |= libc::CLONE_NEWNET;
    }
    // A fork that starts in new namespaces. This process is single-threaded, so it is safe.
    let child = unsafe {
        libc::syscall(
            libc::SYS_clone,
            (flags | libc::SIGCHLD) as libc::c_long,
            0,
            0,
            0,
            0,
        )
    };
    if child < 0 {
        let message = format!("cannot create the sandbox: {}", io::Error::last_os_error());
        unsafe {
            libc::write(ready_w, message.as_ptr().cast(), message.len());
            libc::_exit(1)
        }
    }
    if child == 0 {
        unsafe { libc::close(pid_w) };
        become_init(plan, staging, exe, sync_r, ready_w, status_fd, config)
    }
    let bytes = (child as i32).to_le_bytes();
    unsafe {
        libc::write(pid_w, bytes.as_ptr().cast(), 4);
        libc::_exit(0)
    }
}

fn read_all(fd: RawFd) -> String {
    let mut out = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n > 0 {
            out.extend_from_slice(&buf[..n as usize]);
        } else if n == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            break;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn reap(pid: libc::pid_t) -> libc::c_int {
    let mut status = 0;
    loop {
        let r = unsafe { libc::waitpid(pid, &mut status, 0) };
        if r >= 0 || io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return status;
        }
    }
}

/// The child of `clone`, which is PID 1 of the new namespaces: wait for the id maps, build
/// the new root, and re-execute `oma` as the init. It reports a failure over the pipe.
fn become_init(
    plan: &Plan,
    staging: &Path,
    exe: &fs::File,
    sync_r: RawFd,
    ready_w: RawFd,
    status_fd: RawFd,
    config: &str,
) -> ! {
    use std::os::fd::AsRawFd;
    let fail = |message: String| -> ! {
        unsafe { libc::write(ready_w, message.as_ptr().cast(), message.len()) };
        unsafe { libc::_exit(1) }
    };

    let mut byte = [0u8; 1];
    if unsafe { libc::read(sync_r, byte.as_mut_ptr().cast(), 1) } != 1 {
        unsafe { libc::_exit(1) };
    }
    unsafe { libc::close(sync_r) };
    // Not the parent's session: a closing terminal must not take the application with it.
    unsafe { libc::setsid() };

    if let Err(e) = mounts::enter(staging, &plan.ops) {
        fail(e.to_string());
    }
    // The report pipe, and the status pipe if there is one, must survive into the init.
    unsafe { libc::fcntl(ready_w, libc::F_SETFD, 0) };
    if status_fd >= 0 {
        unsafe { libc::fcntl(status_fd, libc::F_SETFD, 0) };
    }
    if let Err(e) = init::raise_ambient_capabilities() {
        fail(format!("cannot keep capabilities for the init: {e}"));
    }

    let argv0 = std::ffi::CString::new("odk-oma").unwrap();
    let argv1 = std::ffi::CString::new("--init").unwrap();
    let argv2 = std::ffi::CString::new(config).unwrap();
    let argv = [
        argv0.as_ptr(),
        argv1.as_ptr(),
        argv2.as_ptr(),
        std::ptr::null(),
    ];
    let envp: [*const libc::c_char; 1] = [std::ptr::null()];
    let empty = std::ffi::CString::new("").unwrap();
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            exe.as_raw_fd(),
            empty.as_ptr(),
            argv.as_ptr(),
            envp.as_ptr(),
            libc::AT_EMPTY_PATH,
        )
    };
    fail(format!(
        "cannot start the init: {}",
        io::Error::last_os_error()
    ))
}
