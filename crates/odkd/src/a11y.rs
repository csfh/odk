//! A stand-in session bus, so that a sandboxed window can start its accessibility (ADR 0096).
//!
//! Rune's `accesskit_unix` connects to the session bus first and watches
//! `org.a11y.Status.IsEnabled` there, and only when that is true does it connect to the AT-SPI
//! bus. A sandboxed window has no session bus (ADR 0012, ADR 0068). `odkd` runs a private
//! `dbus-daemon` that lets a peer talk to nothing but `org.a11y.Bus`, serves that name itself, and
//! says what the user's own session bus says: whether accessibility is on and, if it is, where the
//! AT-SPI bus is. oma binds the stand-in and the AT-SPI bus into a sandbox only while it is on.
//!
//! The library watches `IsEnabled` as a stream of changes. zbus gives such a stream the current
//! value first, so a peer that subscribes after accessibility went on is told it is on (tested);
//! a later change reaches every peer that asked.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use futures_lite::StreamExt;
use zbus::zvariant::{ObjectPath, Value};
use zbus::{Connection, connection};

/// The name the stand-in serves, and its path: where a client looks for `org.a11y.Status`.
const NAME: &str = "org.a11y.Bus";
const PATH: &str = "/org/a11y/bus";

/// The socket the stand-in listens on and the file that says where the AT-SPI bus is, both in
/// the directory oma already binds into every sandbox (ADR 0062).
pub const SOCKET: &str = "bus";
pub const ADDRESS_FILE: &str = "a11y";

/// How long the daemon has to start listening.
const START_WITHIN: Duration = Duration::from_secs(10);

/// What the user's accessibility is: off, or on with the AT-SPI bus at an address.
pub type State = Option<String>;

/// The daemon's configuration: a session bus on `socket`, where a peer may call the bus driver and
/// `org.a11y.Bus`, answer a call made to it, and send a signal, which is how the stand-in tells a
/// peer what changed; where nothing else can be called and no name but `org.a11y.Bus` owned. This
/// daemon refuses what no rule allows, so what may be received is allowed outright and what may be
/// sent is what is gated.
pub fn config(socket: &Path) -> String {
    format!(
        r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <deny send_destination="*"/>
    <deny own="*"/>
    <allow send_destination="org.freedesktop.DBus"/>
    <allow send_destination="{NAME}"/>
    <allow send_destination="*" send_type="signal"/>
    <allow own="{NAME}"/>
    <allow send_requested_reply="true" send_type="method_return"/>
    <allow send_requested_reply="true" send_type="error"/>
    <allow receive_sender="*"/>
  </policy>
</busconfig>
"#,
        socket.display()
    )
}

/// A running stand-in. Dropping it stops the daemon.
pub struct Standin {
    daemon: Child,
    state: async_channel::Sender<State>,
}

impl Drop for Standin {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

impl Standin {
    /// Starts the daemon listening on `dir/bus`, serves `org.a11y.Bus` on it, and keeps `dir/a11y`
    /// as the address of the AT-SPI bus while it is on, and absent while it is off. `Err` says
    /// why there is no stand-in, and nothing is left running.
    pub fn start(dir: &Path) -> Result<Self, String> {
        let socket = dir.join(SOCKET);
        let _ = fs::remove_file(&socket);
        let config = dir.join("bus.conf");
        fs::write(&config, self::config(&socket)).map_err(|e| format!("no bus.conf: {e}"))?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).ok();
        let mut command = Command::new("dbus-daemon");
        command
            .arg(format!("--config-file={}", config.display()))
            .arg("--nofork")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // The daemon goes when `odkd` does, however that happens.
        // SAFETY: `prctl` is async-signal-safe and touches nothing of the parent's.
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let mut daemon = command
            .spawn()
            .map_err(|e| format!("no dbus-daemon to run: {e}"))?;
        let started = Instant::now();
        while !socket.exists() {
            if started.elapsed() > START_WITHIN || daemon.try_wait().ok().flatten().is_some() {
                let _ = daemon.kill();
                let _ = daemon.wait();
                return Err("the stand-in session bus did not start".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
        let (state, updates) = async_channel::unbounded::<State>();
        let address = format!("unix:path={}", socket.display());
        let file = dir.join(ADDRESS_FILE);
        let served = thread::Builder::new()
            .name("odkd-a11y".into())
            .spawn(move || {
                if let Err(e) = futures_lite::future::block_on(serve(&address, &file, updates)) {
                    eprintln!("odkd: the accessibility stand-in stopped: {e}");
                }
            });
        if let Err(e) = served {
            let _ = daemon.kill();
            let _ = daemon.wait();
            return Err(format!("no thread for the stand-in: {e}"));
        }
        Ok(Self { daemon, state })
    }

    /// Says what the user's accessibility is now.
    #[cfg(test)]
    pub fn set(&self, state: State) {
        let _ = self.state.try_send(state);
    }

    /// A way to say it from another thread.
    pub fn sender(&self) -> async_channel::Sender<State> {
        self.state.clone()
    }
}

/// What the stand-in answers on `org.a11y.Bus` and `org.a11y.Status`.
struct Answers {
    state: Arc<Mutex<State>>,
}

#[zbus::interface(name = "org.a11y.Bus")]
impl Answers {
    /// Where the AT-SPI bus is, while accessibility is on, and nothing while it is off.
    fn get_address(&self) -> String {
        self.state.lock().unwrap().clone().unwrap_or_default()
    }
}

struct Status {
    state: Arc<Mutex<State>>,
}

#[zbus::interface(name = "org.a11y.Status")]
impl Status {
    #[zbus(property)]
    fn is_enabled(&self) -> bool {
        self.state.lock().unwrap().is_some()
    }

    #[zbus(property)]
    fn screen_reader_enabled(&self) -> bool {
        self.state.lock().unwrap().is_some()
    }
}

/// Serves the names on the stand-in until the updates stop.
async fn serve(
    address: &str,
    file: &Path,
    updates: async_channel::Receiver<State>,
) -> zbus::Result<()> {
    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(None));
    let conn = connection::Builder::address(address)?
        .serve_at(
            PATH,
            Answers {
                state: state.clone(),
            },
        )?
        .serve_at(
            PATH,
            Status {
                state: state.clone(),
            },
        )?
        .name(NAME)?
        .build()
        .await?;
    let follow = async {
        while let Ok(next) = updates.recv().await {
            publish(file, &next);
            let enabled = next.is_some();
            *state.lock().unwrap() = next;
            send_status(&conn, enabled).await?;
        }
        Ok(())
    };
    follow.await
}

/// `PropertiesChanged` of `IsEnabled` to everyone who asked.
async fn send_status(conn: &Connection, enabled: bool) -> zbus::Result<()> {
    let mut changed = std::collections::HashMap::new();
    changed.insert("IsEnabled", Value::from(enabled));
    changed.insert("ScreenReaderEnabled", Value::from(enabled));
    let invalidated: Vec<&str> = Vec::new();
    let path = ObjectPath::try_from(PATH)?;
    let message =
        zbus::Message::signal(path, "org.freedesktop.DBus.Properties", "PropertiesChanged")?
            .build(&("org.a11y.Status", changed, invalidated))?;
    conn.send(&message).await
}

/// Writes the AT-SPI bus's address where oma looks for it, or removes the file when there is none.
/// A bus that is not at a path (an abstract socket) cannot be bound into a sandbox, so it is not
/// published.
fn publish(file: &Path, state: &State) {
    let path = state.as_deref().and_then(socket_path_of);
    match path {
        Some(path) => {
            let temporary = file.with_extension("new");
            let written = fs::File::create(&temporary)
                .and_then(|mut f| writeln!(f, "{}", path.display()))
                .and_then(|()| fs::rename(&temporary, file));
            if let Err(e) = written {
                eprintln!("odkd: not publishing the accessibility bus: {e}");
            }
        }
        None => {
            let _ = fs::remove_file(file);
        }
    }
}

/// The path in a D-Bus address that names a socket by path, such as
/// `unix:path=/run/user/1000/at-spi/bus_0,guid=...`.
pub fn socket_path_of(address: &str) -> Option<PathBuf> {
    address.split(';').find_map(|one| {
        let rest = one.strip_prefix("unix:")?;
        rest.split(',')
            .find_map(|pair| pair.strip_prefix("path="))
            .map(PathBuf::from)
    })
}

/// The user's own session bus, watched: whether accessibility is on and where the AT-SPI bus is.
/// `Err` when there is no session bus, which leaves accessibility off.
pub async fn follow_session(report: impl Fn(State)) -> zbus::Result<()> {
    let session = Connection::session().await?;
    let status = zbus::Proxy::new(&session, NAME, PATH, "org.a11y.Status").await?;
    let mut changes = status.receive_property_changed::<bool>("IsEnabled").await;
    let now = |session: Connection| async move {
        let status = zbus::Proxy::new(&session, NAME, PATH, "org.a11y.Status").await?;
        if !status.get_property::<bool>("IsEnabled").await? {
            return Ok::<State, zbus::Error>(None);
        }
        let bus = zbus::Proxy::new(&session, NAME, PATH, "org.a11y.Bus").await?;
        let address: String = bus.call("GetAddress", &()).await?;
        Ok((!address.is_empty()).then_some(address))
    };
    report(now(session.clone()).await.unwrap_or(None));
    while changes.next().await.is_some() {
        report(now(session.clone()).await.unwrap_or(None));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("odkd-a11y-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_path_is_found_in_an_address_and_an_abstract_socket_has_none() {
        assert_eq!(
            socket_path_of("unix:path=/run/user/1000/at-spi/bus_0,guid=abc"),
            Some(PathBuf::from("/run/user/1000/at-spi/bus_0"))
        );
        assert_eq!(socket_path_of("unix:abstract=/tmp/dbus-x,guid=abc"), None);
        assert_eq!(
            socket_path_of("tcp:host=h,port=1;unix:path=/a/b"),
            Some(PathBuf::from("/a/b"))
        );
        assert_eq!(socket_path_of(""), None);
    }

    /// A stand-in on a private bus, or `None` where there is no `dbus-daemon` (said, not failed).
    fn standin(dir: &Path) -> Option<Standin> {
        match Standin::start(dir) {
            Ok(standin) => Some(standin),
            Err(why) => {
                eprintln!("skipped: {why}");
                None
            }
        }
    }

    /// A client as `accesskit_unix` is one: it connects to the bus, watches `IsEnabled` as a
    /// stream of changes, and reads the value when one comes.
    async fn watch(
        dir: &Path,
    ) -> zbus::Result<(Connection, zbus::proxy::PropertyStream<'static, bool>)> {
        let address = format!("unix:path={}", dir.join(SOCKET).display());
        let conn = connection::Builder::address(address.as_str())?
            .build()
            .await?;
        let status = zbus::Proxy::new(&conn, NAME, PATH, "org.a11y.Status").await?;
        let changes = status.receive_property_changed::<bool>("IsEnabled").await;
        Ok((conn, changes))
    }

    async fn next(changes: &mut zbus::proxy::PropertyStream<'static, bool>) -> Option<bool> {
        let wait = async_io_timeout();
        futures_lite::future::or(
            async {
                let change = changes.next().await?;
                change.get().await.ok()
            },
            wait,
        )
        .await
    }

    async fn async_io_timeout() -> Option<bool> {
        let (tx, rx) = async_channel::bounded::<()>(1);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(5));
            let _ = tx.try_send(());
        });
        let _ = rx.recv().await;
        None
    }

    #[test]
    fn a_client_that_subscribes_is_told_what_accessibility_is_and_hears_it_change() {
        let dir = tempdir("told");
        let Some(standin) = standin(&dir) else { return };
        standin.set(Some("unix:path=/run/user/1000/at-spi/bus_0,guid=x".into()));
        // Wait for the stand-in to have taken it before a client comes, so the client has to be
        // told and not just hear a change.
        let published = dir.join(ADDRESS_FILE);
        let started = Instant::now();
        while !published.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "never published"
            );
            thread::sleep(Duration::from_millis(20));
        }
        futures_lite::future::block_on(async {
            let (conn, mut changes) = watch(&dir).await.expect("a client");
            assert_eq!(next(&mut changes).await, Some(true), "told on subscribing");
            // And it hears the change after that.
            standin.set(None);
            assert_eq!(
                next(&mut changes).await,
                Some(false),
                "told when it goes off"
            );
            drop(conn);
        });
        drop(standin);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_address_is_published_while_it_is_on_and_only_when_it_is_a_path() {
        let dir = tempdir("file");
        let Some(standin) = standin(&dir) else { return };
        let file = dir.join(ADDRESS_FILE);
        let settle = |present: bool| {
            let started = Instant::now();
            while file.exists() != present {
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "file present: {present}"
                );
                thread::sleep(Duration::from_millis(20));
            }
        };
        standin.set(Some("unix:path=/run/user/1000/at-spi/bus_0,guid=x".into()));
        settle(true);
        assert_eq!(
            fs::read_to_string(&file).unwrap().trim(),
            "/run/user/1000/at-spi/bus_0"
        );
        standin.set(None);
        settle(false);
        standin.set(Some("unix:abstract=/tmp/dbus-x,guid=x".into()));
        thread::sleep(Duration::from_millis(500));
        assert!(
            !file.exists(),
            "an abstract socket cannot be bound into a sandbox"
        );
        drop(standin);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_stand_in_serves_get_address_and_nothing_else_is_reachable_on_it() {
        let dir = tempdir("policy");
        let Some(standin) = standin(&dir) else { return };
        standin.set(Some("unix:path=/run/user/1000/at-spi/bus_0,guid=x".into()));
        thread::sleep(Duration::from_millis(500));
        futures_lite::future::block_on(async {
            let (conn, _changes) = watch(&dir).await.expect("a client");
            let bus = zbus::Proxy::new(&conn, NAME, PATH, "org.a11y.Bus")
                .await
                .unwrap();
            let address: String = bus.call("GetAddress", &()).await.expect("GetAddress");
            assert!(address.starts_with("unix:path=/run/user/1000/at-spi/bus_0"));
            // Another service on this bus is not reachable.
            let other = conn
                .call_method(
                    Some("org.freedesktop.Notifications"),
                    "/org/freedesktop/Notifications",
                    Some("org.freedesktop.Notifications"),
                    "GetCapabilities",
                    &(),
                )
                .await;
            assert!(other.is_err(), "nothing but org.a11y.Bus answers");
            // A client cannot call another client either.
            let other = connection::Builder::address(
                format!("unix:path={}", dir.join(SOCKET).display()).as_str(),
            )
            .unwrap()
            .method_timeout(Duration::from_secs(3))
            .build()
            .await
            .expect("a second client");
            let peer = conn.unique_name().expect("a unique name").to_string();
            let ping = other
                .call_method(
                    Some(peer.as_str()),
                    "/",
                    Some("org.freedesktop.DBus.Peer"),
                    "Ping",
                    &(),
                )
                .await;
            // Refused by the bus, not merely unanswered.
            match ping {
                Err(zbus::Error::MethodError(name, ..)) => {
                    assert!(name.as_str().ends_with("AccessDenied"), "{name}");
                }
                other => panic!("no peer-to-peer calls on the stand-in: {other:?}"),
            }
            // And a client cannot take a name of its own.
            let named = conn.request_name("org.example.Mine").await;
            assert!(named.is_err(), "no name may be owned but the stand-in's");
        });
        drop(standin);
        let _ = fs::remove_dir_all(&dir);
    }
}
