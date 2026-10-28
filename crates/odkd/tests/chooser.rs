//! The real `odkd` opening a file chooser (ADR 0068), against a fake portal on a private bus.
//!
//! The user's own portal must never be reached: it would open a window on their desktop. So
//! `odkd` runs with an environment of its own whose only bus is a private `dbus-daemon`, which has
//! no service directories and so starts nothing, and the fake answers as the portal on it. The
//! test is skipped, saying so, where there is no `dbus-daemon`.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use odk_theme::chooser::{ChooserAnswer, ChooserKind, ChooserRequest, FileFilter, StartFolder};
use zbus::message::Header;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(start.elapsed() < Duration::from_secs(10), "no {path:?}");
        thread::sleep(Duration::from_millis(20));
    }
}

/// A `dbus-daemon` of its own, listening on `dir/bus`, that can start no service.
fn private_bus(dir: &Path) -> Option<(Process, String)> {
    let socket = dir.join("bus");
    let config = dir.join("bus.conf");
    fs::write(
        &config,
        format!(
            r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
            socket.display()
        ),
    )
    .unwrap();
    let child = Command::new("dbus-daemon")
        .arg(format!("--config-file={}", config.display()))
        .arg("--nofork")
        .env_clear()
        .stdout(Stdio::null())
        .spawn()
        .ok()?;
    let bus = Process(child);
    wait_for(&socket);
    Some((bus, format!("unix:path={}", socket.display())))
}

/// What the fake portal was asked: the method, the title and the options it understood.
#[derive(Debug, Default, Clone)]
struct Asked {
    method: String,
    title: String,
    directory: Option<bool>,
    multiple: Option<bool>,
    current_name: Option<String>,
    current_folder: Option<PathBuf>,
    filters: Vec<(String, Vec<String>)>,
}

/// `org.freedesktop.portal.FileChooser`, answering every request with `answer`: a response
/// code (0 chosen, 1 cancelled) and the URIs.
struct FakePortal {
    asked: Arc<Mutex<Vec<Asked>>>,
    answer: (u32, Vec<String>),
}

impl FakePortal {
    async fn respond(
        &self,
        method: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        header: &Header<'_>,
        connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let token: String = options
            .get("handle_token")
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_default();
        let sender = header
            .sender()
            .map(|s| s.trim_start_matches(':').replace('.', "_"))
            .unwrap_or_default();
        let path = OwnedObjectPath::try_from(format!(
            "/org/freedesktop/portal/desktop/request/{sender}/{token}"
        ))
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        self.asked.lock().unwrap().push(Asked {
            method: method.into(),
            title: title.into(),
            directory: options
                .get("directory")
                .and_then(|v| bool::try_from(v).ok()),
            multiple: options.get("multiple").and_then(|v| bool::try_from(v).ok()),
            current_name: options
                .get("current_name")
                .and_then(|v| String::try_from(v.clone()).ok()),
            current_folder: options.get("current_folder").and_then(|v| {
                let bytes: Vec<u8> = Vec::try_from(v.clone()).ok()?;
                let bytes = bytes.strip_suffix(&[0]).unwrap_or(&bytes).to_vec();
                Some(PathBuf::from(String::from_utf8(bytes).ok()?))
            }),
            filters: options
                .get("filters")
                .and_then(|v| <Vec<(String, Vec<(u32, String)>)>>::try_from(v.clone()).ok())
                .map(|fs| {
                    fs.into_iter()
                        .map(|(label, ps)| (label, ps.into_iter().map(|(_, p)| p).collect()))
                        .collect()
                })
                .unwrap_or_default(),
        });
        let mut results: HashMap<&str, Value> = HashMap::new();
        results.insert("uris", Value::from(self.answer.1.clone()));
        connection
            .emit_signal(
                None::<&str>,
                &path,
                "org.freedesktop.portal.Request",
                "Response",
                &(self.answer.0, results),
            )
            .await?;
        Ok(path)
    }
}

#[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
impl FakePortal {
    async fn open_file(
        &self,
        _parent: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.respond("OpenFile", title, options, &header, connection)
            .await
    }

    async fn save_file(
        &self,
        _parent: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        self.respond("SaveFile", title, options, &header, connection)
            .await
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        4
    }
}

struct Setup {
    _dir: tempfile::TempDir,
    _bus: Process,
    _portal: zbus::Connection,
    _odkd: Process,
    files: PathBuf,
    home: PathBuf,
    asked: Arc<Mutex<Vec<Asked>>>,
}

fn setup(answer: (u32, Vec<String>)) -> Option<Setup> {
    let dir = tempfile::tempdir().unwrap();
    let Some((bus, address)) = private_bus(dir.path()) else {
        eprintln!("skipped: no dbus-daemon to run a private bus on");
        return None;
    };
    let asked = Arc::new(Mutex::new(Vec::new()));
    let portal = futures_lite::future::block_on(async {
        zbus::connection::Builder::address(address.as_str())?
            .name("org.freedesktop.portal.Desktop")?
            .serve_at(
                "/org/freedesktop/portal/desktop",
                FakePortal {
                    asked: asked.clone(),
                    answer,
                },
            )?
            .build()
            .await
    })
    .unwrap();
    let home = dir.path().join("home");
    let runtime = dir.path().join("run");
    fs::create_dir_all(home.join("Pictures")).unwrap();
    fs::create_dir_all(home.join(".config")).unwrap();
    fs::write(
        home.join(".config/user-dirs.dirs"),
        "XDG_PICTURES_DIR=\"$HOME/Pictures\"\n",
    )
    .unwrap();
    fs::create_dir_all(&runtime).unwrap();
    let socket = runtime.join("odk/odkd.sock");
    let odkd = Process(
        Command::new(env!("CARGO_BIN_EXE_odkd"))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", &home)
            .env("XDG_RUNTIME_DIR", &runtime)
            .env("ODKD_SOCKET", &socket)
            // The private bus and nothing else.
            .env("DBUS_SESSION_BUS_ADDRESS", &address)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let files = runtime.join("odk/files.sock");
    wait_for(&files);
    Some(Setup {
        _dir: dir,
        _bus: bus,
        _portal: portal,
        _odkd: odkd,
        files,
        home,
        asked,
    })
}

fn ask(files: &Path, request: &ChooserRequest) -> ChooserAnswer {
    let mut stream = UnixStream::connect(files).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    writeln!(stream, "{}", request.to_line()).unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    ChooserAnswer::from_line(line.trim_end()).unwrap()
}

#[test]
fn an_open_dialog_answers_with_the_paths_the_portal_gave() {
    let Some(s) = setup((0, vec!["file:///home/me/face%20one.png".into()])) else {
        return;
    };
    let mut request = ChooserRequest::new(ChooserKind::Open, "Choose a face");
    request.filters.push(FileFilter {
        label: "Images".into(),
        patterns: vec!["*.png".into(), "*.jpg".into()],
    });
    request.start_in = Some(StartFolder::Pictures);
    assert_eq!(
        ask(&s.files, &request),
        ChooserAnswer::Chosen(vec!["/home/me/face one.png".into()])
    );
    let asked = s.asked.lock().unwrap()[0].clone();
    assert_eq!(asked.method, "OpenFile");
    assert_eq!(asked.title, "Choose a face");
    assert_eq!(asked.directory, Some(false));
    assert_eq!(
        asked.filters,
        vec![(
            "Images".to_owned(),
            vec!["*.png".to_owned(), "*.jpg".to_owned()]
        )]
    );
    assert_eq!(
        asked.current_folder,
        Some(s.home.join("Pictures")),
        "the user's Pictures, looked up outside the sandbox"
    );
}

#[test]
fn a_folder_and_a_save_dialog_ask_for_what_they_are() {
    let Some(s) = setup((0, vec!["file:///home/me/Shared".into()])) else {
        return;
    };
    let mut folder = ChooserRequest::new(ChooserKind::Folder, "Share a folder");
    folder.multiple = true;
    assert!(matches!(ask(&s.files, &folder), ChooserAnswer::Chosen(_)));
    let mut save = ChooserRequest::new(ChooserKind::Save, "Save diagnostic report");
    save.name = Some("omarchy-debug.txt".into());
    save.start_in = Some(StartFolder::Home);
    assert!(matches!(ask(&s.files, &save), ChooserAnswer::Chosen(_)));
    let asked = s.asked.lock().unwrap().clone();
    assert_eq!(
        (
            asked[0].method.as_str(),
            asked[0].directory,
            asked[0].multiple
        ),
        ("OpenFile", Some(true), Some(true))
    );
    assert_eq!(asked[1].method, "SaveFile");
    assert_eq!(asked[1].current_name.as_deref(), Some("omarchy-debug.txt"));
    assert_eq!(asked[1].current_folder.as_deref(), Some(s.home.as_path()));
}

#[test]
fn a_cancelled_dialog_is_cancelled() {
    let Some(s) = setup((1, Vec::new())) else {
        return;
    };
    let request = ChooserRequest::new(ChooserKind::Open, "Install a hook");
    assert_eq!(ask(&s.files, &request), ChooserAnswer::Cancelled);
}
