//! A Bundle's daemon (ADR 0050): the one program of the Bundle's that runs outside the sandbox.
//!
//! `oma run` makes sure it is there before it starts the application, and never stays to watch
//! it. It is started as a transient systemd user unit, so `oma` is not its parent (ADR 0034) and
//! `oma` is not a daemon (ADR 0025), with the socket as a socket unit of its own: systemd owns
//! the socket, so its file survives the daemon restarting, which a bind mount of it into the
//! sandbox needs. Nothing here runs in the init (ADR 0024).

use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::fmt::{Bundle, Daemon, Identifier};

/// How long to wait for the socket to appear once the unit has been started.
const WAIT: Duration = Duration::from_secs(5);

/// The part of the user's session that the daemon is given: what its tools need to find the
/// desktop and no more. `oma run` is started from the session, so these are the session's.
pub const SESSION_ENV: &[&str] = &[
    "PATH",
    "WAYLAND_DISPLAY",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_RUNTIME_DIR",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "LANG",
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the daemon cannot be started: {0}")]
    Start(String),
    #[error("the daemon was started, but {} did not appear", .0.display())]
    NoSocket(PathBuf),
}

/// A transient unit to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub name: String,
    pub description: String,
    pub socket: PathBuf,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// `systemd-escape` of `text`, as ADR 0015 has `oma` build unit names: `/` becomes `-`, and
/// anything but ASCII letters, digits, `:`, `_` and `.` (not first) becomes `\xNN`. An
/// Identifier has `-` in it, which is how it comes to be `\x2d` here.
fn escape(text: &str) -> String {
    let mut out = String::new();
    for (at, byte) in text.bytes().enumerate() {
        match byte {
            b'/' => out.push('-'),
            b'.' if at == 0 => out.push_str("\\x2e"),
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b':' | b'_' | b'.' => out.push(byte as char),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out
}

/// The name of the transient unit that carries the application `id`'s daemon, which starting
/// and stopping both use.
pub fn unit_name(id: &Identifier) -> String {
    format!("oma-{}", escape(id.as_str()))
}

impl Unit {
    /// The unit that carries `daemon` of the application `id`.
    pub fn new(
        id: &Identifier,
        daemon: &Daemon,
        executable: &Path,
        runtime_dir: &Path,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Self {
        Self {
            name: unit_name(id),
            description: format!("{id} daemon, started by oma"),
            socket: runtime_dir.join(&daemon.socket),
            executable: executable.to_owned(),
            args: daemon.args.clone(),
            env: SESSION_ENV
                .iter()
                .filter_map(|name| Some(((*name).to_owned(), env(name)?)))
                .collect(),
        }
    }

    /// The arguments of `systemd-run` that start it.
    pub fn systemd_run_args(&self) -> Vec<String> {
        let mut args: Vec<String> = [
            "--user",
            "--quiet",
            "--collect",
            // The user's own socket: private to them, in a private directory.
            "--socket-property=SocketMode=0600",
            "--socket-property=DirectoryMode=0700",
        ]
        .map(str::to_owned)
        .into();
        args.push(format!("--unit={}", self.name));
        args.push(format!("--description={}", self.description));
        args.push(format!(
            "--socket-property=ListenStream={}",
            self.socket.display()
        ));
        for (name, value) in &self.env {
            args.push(format!("--setenv={name}={value}"));
        }
        args.push("--".into());
        args.push(self.executable.display().to_string());
        args.extend(self.args.iter().cloned());
        args
    }
}

/// Starts a unit. The real one is [`SystemdRun`]; tests put their own, so no test starts
/// anything in the session it runs in.
pub trait Starter {
    fn start(&self, unit: &Unit) -> Result<(), String>;
}

pub struct SystemdRun;

impl Starter for SystemdRun {
    fn start(&self, unit: &Unit) -> Result<(), String> {
        let out = Command::new("systemd-run")
            .args(unit.systemd_run_args())
            .output()
            .map_err(|e| format!("systemd-run: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            let why = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            // A unit of this name that is still loaded, such as one that failed, is not
            // replaced: say how to clear it, which is what the user would need to know.
            let hint = if why.contains("already") {
                format!(
                    "\n(a unit named {} is loaded; `systemctl --user reset-failed {}.service {}.socket` clears a failed one)",
                    unit.name, unit.name, unit.name
                )
            } else {
                String::new()
            };
            Err(format!("{why}{hint}"))
        }
    }
}

/// Stops a unit `oma` started. The real one is [`SystemctlStop`]; tests put their own.
pub trait Stopper {
    fn stop(&self, id: &Identifier) -> Result<(), String>;
}

/// `systemctl --user stop` of the application's socket and service.
pub struct SystemctlStop;

impl Stopper for SystemctlStop {
    fn stop(&self, id: &Identifier) -> Result<(), String> {
        let name = unit_name(id);
        let status = Command::new("systemctl")
            .args(["--user", "--quiet", "stop"])
            .arg(format!("{name}.socket"))
            .arg(format!("{name}.service"))
            .status()
            .map_err(|e| format!("systemctl: {e}"))?;
        // 5 is "not loaded": nothing of ours was running, which is what was wanted.
        match status.code() {
            Some(0 | 5) => Ok(()),
            _ => Err(format!("systemctl --user stop {name} {status}")),
        }
    }
}

/// Whether something accepts connections on the daemon's socket under `runtime_dir`.
pub fn serving(runtime_dir: &Path, daemon: &Daemon) -> bool {
    reachable(&runtime_dir.join(&daemon.socket))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Something already accepts connections on the socket, so nothing was started.
    Running,
    Started,
}

fn reachable(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

fn is_socket(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.file_type().is_socket())
}

/// Makes sure the Bundle's daemon is there. `None` when the Bundle declares none.
///
/// If the socket accepts connections, something is serving it already, perhaps a daemon the
/// application installed units for, and a second must not run. Otherwise a unit is started and
/// the socket waited for.
pub fn ensure(
    bundle: &Bundle,
    runtime_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    starter: &dyn Starter,
    wait: Duration,
) -> Result<Option<Outcome>, Error> {
    let Some(daemon) = &bundle.manifest().permissions.daemon else {
        return Ok(None);
    };
    Daemon::check_socket(&daemon.socket).map_err(Error::Start)?;
    let executable = bundle
        .daemon_executable()
        .ok_or_else(|| Error::Start("the Bundle does not hold its executable".into()))?;
    let unit = Unit::new(
        &bundle.manifest().identifier,
        daemon,
        executable,
        runtime_dir,
        env,
    );
    if reachable(&unit.socket) {
        return Ok(Some(Outcome::Running));
    }
    starter.start(&unit).map_err(Error::Start)?;
    let until = Instant::now() + wait;
    while Instant::now() < until {
        if is_socket(&unit.socket) {
            return Ok(Some(Outcome::Started));
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    if is_socket(&unit.socket) {
        return Ok(Some(Outcome::Started));
    }
    Err(Error::NoSocket(unit.socket))
}

/// [`ensure`] with the real starter and the time to wait that `oma run` allows.
pub fn ensure_for_run(
    bundle: &Bundle,
    runtime_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Outcome>, Error> {
    ensure(bundle, runtime_dir, env, &SystemdRun, WAIT)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::os::unix::net::UnixListener;

    use tempfile::TempDir;

    use super::*;
    use crate::testutil::elf;

    struct Fake {
        started: RefCell<Vec<Unit>>,
        /// What a started unit does to the socket path.
        then: Box<dyn Fn(&Unit)>,
        result: Result<(), String>,
    }

    impl Starter for Fake {
        fn start(&self, unit: &Unit) -> Result<(), String> {
            self.started.borrow_mut().push(unit.clone());
            (self.then)(unit);
            self.result.clone()
        }
    }

    fn fake(then: impl Fn(&Unit) + 'static, result: Result<(), String>) -> Fake {
        Fake {
            started: RefCell::new(Vec::new()),
            then: Box::new(then),
            result,
        }
    }

    /// A Bundle with a daemon, and a runtime directory.
    fn world(daemon: bool) -> (TempDir, Bundle, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("T.oma");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        let tail = if daemon {
            "[daemon]\nexecutable = \"bin/d\"\nargs = [\"daemon\", \"--x\"]\nsocket = \"app/d.sock\"\n"
        } else {
            ""
        };
        std::fs::write(
            root.join("manifest.toml"),
            format!(
                "manifest_version = 1\nidentifier = \"com.example.t\"\nname = \"T\"\nversion = \"1.0.0\"\nexecutable = \"bin/t\"\n{tail}\n[permissions]\n"
            ),
        )
        .unwrap();
        for exe in ["bin/t", "bin/d"] {
            std::fs::write(root.join(exe), elf()).unwrap();
            std::fs::set_permissions(
                root.join(exe),
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .unwrap();
        }
        let runtime = dir.path().join("run");
        std::fs::create_dir_all(&runtime).unwrap();
        let bundle = Bundle::open(&root).unwrap();
        (dir, bundle, runtime)
    }

    fn session(name: &str) -> Option<String> {
        match name {
            "PATH" => Some("/session/bin:/usr/bin".into()),
            "WAYLAND_DISPLAY" => Some("wayland-1".into()),
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("sig".into()),
            // Everything else the session has stays with the session.
            "LD_PRELOAD" => Some("/evil.so".into()),
            "SSH_AUTH_SOCK" => Some("/agent".into()),
            _ => None,
        }
    }

    #[test]
    fn an_identifier_is_escaped_as_systemd_would() {
        assert_eq!(escape("com.example.t"), "com.example.t");
        assert_eq!(escape("com.my-app.t"), "com.my\\x2dapp.t");
        assert_eq!(escape(".hidden"), "\\x2ehidden");
        assert_eq!(escape("a/b"), "a-b");
        // The real thing agrees, when it is installed.
        if let Ok(out) = Command::new("systemd-escape").arg("com.my-app.t").output()
            && out.status.success()
        {
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                escape("com.my-app.t")
            );
        }
    }

    #[test]
    fn a_bundle_without_a_daemon_starts_nothing() {
        let (_dir, bundle, runtime) = world(false);
        let starter = fake(|_| {}, Ok(()));
        let outcome = ensure(&bundle, &runtime, &session, &starter, Duration::ZERO).unwrap();
        assert_eq!(outcome, None);
        assert!(starter.started.borrow().is_empty());
    }

    /// A socket that accepts connections is a daemon already: the user's rule is one daemon.
    #[test]
    fn a_socket_that_is_served_already_starts_nothing() {
        let (_dir, bundle, runtime) = world(true);
        std::fs::create_dir_all(runtime.join("app")).unwrap();
        let _serving = UnixListener::bind(runtime.join("app/d.sock")).unwrap();
        let starter = fake(|_| {}, Ok(()));
        let outcome = ensure(&bundle, &runtime, &session, &starter, Duration::ZERO).unwrap();
        assert_eq!(outcome, Some(Outcome::Running));
        assert!(starter.started.borrow().is_empty());
    }

    #[test]
    fn otherwise_one_unit_is_started_with_the_bundles_program_and_a_slice_of_the_session() {
        let (_dir, bundle, runtime) = world(true);
        let listener = RefCell::new(None);
        let starter = fake(
            move |unit| {
                std::fs::create_dir_all(unit.socket.parent().unwrap()).unwrap();
                *listener.borrow_mut() = Some(UnixListener::bind(&unit.socket).unwrap());
            },
            Ok(()),
        );
        let outcome = ensure(
            &bundle,
            &runtime,
            &session,
            &starter,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(outcome, Some(Outcome::Started));
        let started = starter.started.borrow();
        assert_eq!(started.len(), 1);
        let unit = &started[0];
        assert_eq!(unit.name, "oma-com.example.t");
        assert_eq!(unit.socket, runtime.join("app/d.sock"));
        assert_eq!(unit.executable, bundle.root().join("bin/d"));
        assert_eq!(unit.args, ["daemon", "--x"]);
        let names: Vec<_> = unit.env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            names,
            ["PATH", "WAYLAND_DISPLAY", "HYPRLAND_INSTANCE_SIGNATURE"]
        );
    }

    #[test]
    fn the_systemd_run_arguments_name_the_socket_the_program_and_the_session() {
        let (_dir, bundle, runtime) = world(true);
        let daemon = bundle.manifest().permissions.daemon.clone().unwrap();
        let unit = Unit::new(
            &bundle.manifest().identifier,
            &daemon,
            bundle.daemon_executable().unwrap(),
            &runtime,
            &session,
        );
        let args = unit.systemd_run_args();
        let at = |needle: &str| args.iter().position(|a| a == needle);
        assert!(at("--user").is_some() && at("--collect").is_some());
        assert!(at("--unit=oma-com.example.t").is_some());
        assert!(
            at(&format!(
                "--socket-property=ListenStream={}",
                runtime.join("app/d.sock").display()
            ))
            .is_some()
        );
        assert!(at("--setenv=PATH=/session/bin:/usr/bin").is_some());
        assert!(
            args.iter()
                .all(|a| !a.contains("LD_PRELOAD") && !a.contains("SSH_AUTH_SOCK")),
            "only the allowlist is passed on"
        );
        // The program and its arguments are last, after `--`.
        let dashes = at("--").unwrap();
        assert_eq!(
            args[dashes + 1..],
            [
                bundle.root().join("bin/d").display().to_string(),
                "daemon".to_owned(),
                "--x".to_owned()
            ]
        );
    }

    #[test]
    fn a_unit_that_cannot_start_fails_the_launch_and_one_that_never_listens_does_too() {
        let (_dir, bundle, runtime) = world(true);
        let refusing = fake(|_| {}, Err("no user manager".into()));
        assert!(matches!(
            ensure(&bundle, &runtime, &session, &refusing, Duration::ZERO),
            Err(Error::Start(m)) if m == "no user manager"
        ));
        let silent = fake(|_| {}, Ok(()));
        assert!(matches!(
            ensure(
                &bundle,
                &runtime,
                &session,
                &silent,
                Duration::from_millis(80)
            ),
            Err(Error::NoSocket(_))
        ));
    }
}
