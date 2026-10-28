use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Component, Path};

use semver::Version;
use serde::Deserialize;

use crate::fmt::{Error, Identifier};

/// A validated `manifest.toml`. Path fields are checked for shape only; whether the files
/// exist and stay inside the Bundle is checked by [`crate::fmt::Bundle::open`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub identifier: Identifier,
    pub name: String,
    pub version: Version,
    pub executable: String,
    pub permissions: Permissions,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub categories: Vec<String>,
}

/// What a Bundle asks for beyond the baseline. Anything absent is denied (ADR 0012).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions {
    pub network: bool,
    pub gpu: bool,
    pub audio: bool,
    pub updates: bool,
    pub filesystem: Vec<FsGrant>,
    pub dbus_talk: Vec<String>,
    /// The Manifest's `[daemon]` table (ADR 0050). It is a permission, the largest: a program
    /// of the Bundle's that runs outside the sandbox.
    pub daemon: Option<Daemon>,
}

/// A program the Bundle runs outside the sandbox, and the socket the application reaches it by
/// (ADR 0050).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Daemon {
    /// A path inside the Bundle, held to the rules of the Manifest's `executable`.
    pub executable: String,
    pub args: Vec<String>,
    /// Relative to the user's runtime directory: where the daemon listens, and the one file of
    /// that directory the application is given.
    pub socket: String,
}

/// First components of the runtime directory that are other things': the compositors' command
/// sockets, the display and sound servers, the buses. A Bundle cannot name them as its daemon's
/// socket, since the baseline never grants them (`docs/spec/sandbox-baseline.md`).
const RESERVED_RUNTIME_NAMES: &[&str] = &[
    "hypr", "wayland", "pipewire", "pulse", "bus", "dbus", "systemd", "gnupg", "sway", "niri",
    "ssh",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsGrant {
    pub dir: UserDir,
    pub access: Access,
}

/// The six named user directories of ADR 0012.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserDir {
    Documents,
    Downloads,
    Pictures,
    Videos,
    Music,
    Desktop,
}

impl UserDir {
    /// From the lower-case name used in Manifests and consent records.
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Documents,
            Self::Downloads,
            Self::Pictures,
            Self::Videos,
            Self::Music,
            Self::Desktop,
        ]
        .into_iter()
        .find(|d| d.as_str() == name)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::Downloads => "downloads",
            Self::Pictures => "pictures",
            Self::Videos => "videos",
            Self::Music => "music",
            Self::Desktop => "desktop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    /// Includes read.
    Write,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    manifest_version: i64,
    identifier: String,
    name: String,
    version: String,
    executable: String,
    permissions: RawPermissions,
    daemon: Option<RawDaemon>,
    description: Option<String>,
    icon: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDaemon {
    executable: String,
    #[serde(default)]
    args: Vec<String>,
    socket: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPermissions {
    #[serde(default)]
    network: bool,
    #[serde(default)]
    gpu: bool,
    #[serde(default)]
    audio: bool,
    #[serde(default)]
    updates: bool,
    #[serde(default)]
    filesystem: Vec<RawFsGrant>,
    #[serde(default)]
    dbus: RawDbus,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawDbus {
    #[serde(default)]
    talk: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFsGrant {
    dir: UserDirName,
    access: AccessName,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum UserDirName {
    Documents,
    Downloads,
    Pictures,
    Videos,
    Music,
    Desktop,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum AccessName {
    Read,
    Write,
}

impl Manifest {
    /// Parses and validates the text of a `manifest.toml`.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let raw: Raw = toml::from_str(text)?;
        if raw.manifest_version != 1 {
            return Err(Error::ManifestVersion(raw.manifest_version));
        }
        let identifier = raw.identifier.parse()?;
        check_name(&raw.name)?;
        let version =
            Version::parse(&raw.version).map_err(|e| Error::Version(raw.version.clone(), e))?;
        if let Some(description) = &raw.description
            && description.chars().any(char::is_control)
        {
            return Err(Error::Field(
                "description",
                "must not contain control characters",
            ));
        }
        for category in &raw.categories {
            if category.is_empty()
                || !category
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err(Error::Field(
                    "categories",
                    "each must be letters, digits and '-' only",
                ));
            }
        }
        check_relative_path("executable", &raw.executable)?;
        if let Some(icon) = &raw.icon {
            check_relative_path("icon", icon)?;
        }
        let mut permissions = permissions(raw.permissions)?;
        permissions.daemon = raw.daemon.map(daemon).transpose()?;
        Ok(Self {
            identifier,
            name: raw.name,
            version,
            executable: raw.executable,
            permissions,
            description: raw.description,
            icon: raw.icon,
            categories: raw.categories,
        })
    }
}

fn check_name(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::Name("must not be empty"));
    }
    if name.chars().any(char::is_control) {
        return Err(Error::Name("must not contain control characters"));
    }
    Ok(())
}

/// A path inside the Bundle: relative, no `..`. Symlinks are checked against the real
/// directory later, in [`crate::fmt::Bundle::open`].
fn check_relative_path(field: &'static str, path: &str) -> Result<(), Error> {
    let bad = |reason| Error::Path {
        field,
        path: path.to_owned(),
        reason,
    };
    if path.is_empty() {
        return Err(bad("must not be empty"));
    }
    if path.chars().any(char::is_control) {
        return Err(bad("must not contain control characters"));
    }
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => return Err(bad("must not contain '..'")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(bad("must be relative to the Bundle"));
            }
        }
    }
    Ok(())
}

impl Daemon {
    /// Whether `socket` may be a daemon's: a path under the runtime directory, made of plain
    /// names, and not in a part of it that belongs to something else. The Manifest's parser and
    /// the sandbox's plan both ask, so a hand-built [`Permissions`] cannot reach what the
    /// baseline never grants (`docs/spec/sandbox-baseline.md`).
    pub fn check_socket(socket: &str) -> Result<(), String> {
        let path = Path::new(socket);
        let plain = !socket.is_empty()
            && !socket.chars().any(char::is_control)
            && path.components().all(|c| matches!(c, Component::Normal(_)));
        if !plain {
            return Err(format!(
                "socket {socket:?} must be a path under the runtime directory, with no `..` and not absolute"
            ));
        }
        let first = path.components().next().map_or_else(String::new, |c| {
            c.as_os_str().to_string_lossy().into_owned()
        });
        if RESERVED_RUNTIME_NAMES.iter().any(|r| first.starts_with(r)) {
            return Err(format!(
                "socket {socket:?} is in a part of the runtime directory that belongs to something else"
            ));
        }
        Ok(())
    }
}

fn daemon(raw: RawDaemon) -> Result<Daemon, Error> {
    check_relative_path("daemon.executable", &raw.executable)?;
    let bad = |what: String| Error::Permissions(format!("daemon {what}"));
    if raw.args.iter().any(|a| a.chars().any(char::is_control)) {
        return Err(bad("args must not contain control characters".into()));
    }
    Daemon::check_socket(&raw.socket).map_err(bad)?;
    Ok(Daemon {
        executable: raw.executable,
        args: raw.args,
        socket: raw.socket,
    })
}

fn permissions(raw: RawPermissions) -> Result<Permissions, Error> {
    let mut seen = HashSet::new();
    let mut filesystem = Vec::new();
    for grant in raw.filesystem {
        let dir = match grant.dir {
            UserDirName::Documents => UserDir::Documents,
            UserDirName::Downloads => UserDir::Downloads,
            UserDirName::Pictures => UserDir::Pictures,
            UserDirName::Videos => UserDir::Videos,
            UserDirName::Music => UserDir::Music,
            UserDirName::Desktop => UserDir::Desktop,
        };
        if !seen.insert(dir) {
            return Err(Error::Permissions(format!(
                "filesystem dir {:?} appears more than once",
                dir.as_str()
            )));
        }
        let access = match grant.access {
            AccessName::Read => Access::Read,
            AccessName::Write => Access::Write,
        };
        filesystem.push(FsGrant { dir, access });
    }
    for name in &raw.dbus.talk {
        if name.is_empty() || name.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(Error::Permissions(format!(
                "invalid D-Bus service name {name:?}"
            )));
        }
    }
    Ok(Permissions {
        network: raw.network,
        gpu: raw.gpu,
        audio: raw.audio,
        updates: raw.updates,
        filesystem,
        dbus_talk: raw.dbus.talk,
        daemon: None,
    })
}

/// Semver precedence: build metadata is ignored, unlike `Version`'s own ordering.
pub fn precedence(a: &Version, b: &Version) -> Ordering {
    (a.major, a.minor, a.patch)
        .cmp(&(b.major, b.minor, b.patch))
        .then_with(|| a.pre.cmp(&b.pre))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
manifest_version = 1
identifier = "com.example.notes"
name = "Notes"
version = "1.4.0"
description = "A small notes app"
executable = "bin/notes"
icon = "share/notes.svg"
categories = ["Utility", "TextEditor"]

[permissions]
network = false
gpu = true
audio = false
updates = true

[[permissions.filesystem]]
dir = "documents"
access = "write"

[[permissions.filesystem]]
dir = "downloads"
access = "read"

[permissions.dbus]
talk = ["org.freedesktop.Notifications"]
"#;

    const MINIMAL: &str = r#"
manifest_version = 1
identifier = "com.example.min"
name = "Min"
version = "0.1.0"
executable = "min"

[permissions]
"#;

    fn with(text: &str, from: &str, to: &str) -> String {
        assert!(text.contains(from), "{from}");
        text.replace(from, to)
    }

    #[test]
    fn parses_the_spec_example() {
        let m = Manifest::parse(EXAMPLE).unwrap();
        assert_eq!(m.identifier.as_str(), "com.example.notes");
        assert_eq!(m.version, Version::new(1, 4, 0));
        assert!(m.permissions.gpu && m.permissions.updates);
        assert!(!m.permissions.network && !m.permissions.audio);
        assert_eq!(
            m.permissions.filesystem,
            [
                FsGrant {
                    dir: UserDir::Documents,
                    access: Access::Write
                },
                FsGrant {
                    dir: UserDir::Downloads,
                    access: Access::Read
                },
            ]
        );
        assert_eq!(m.permissions.dbus_talk, ["org.freedesktop.Notifications"]);
        assert_eq!(m.categories, ["Utility", "TextEditor"]);
    }

    #[test]
    fn empty_permissions_deny_everything() {
        let m = Manifest::parse(MINIMAL).unwrap();
        assert_eq!(m.permissions, Permissions::default());
        assert_eq!(m.icon, None);
    }

    #[test]
    fn rejects_unknown_keys_at_every_level() {
        for text in [
            with(MINIMAL, "name = \"Min\"", "name = \"Min\"\nextra = 1"),
            with(MINIMAL, "[permissions]", "[permissions]\nmicrophone = true"),
            with(MINIMAL, "[permissions]", "[permissions.dbus]\nlisten = []"),
            with(
                MINIMAL,
                "[permissions]",
                "[[permissions.filesystem]]\ndir = \"music\"\naccess = \"read\"\nextra = 1",
            ),
        ] {
            assert!(
                matches!(Manifest::parse(&text), Err(Error::Toml(_))),
                "{text}"
            );
        }
    }

    #[test]
    fn rejects_missing_required_keys() {
        for key in [
            "manifest_version = 1\n",
            "identifier = \"com.example.min\"\n",
            "name = \"Min\"\n",
            "version = \"0.1.0\"\n",
            "executable = \"min\"\n",
            "[permissions]\n",
        ] {
            let text = with(MINIMAL, key, "");
            assert!(Manifest::parse(&text).is_err(), "without {key:?}");
        }
    }

    #[test]
    fn rejects_other_manifest_versions() {
        let text = with(MINIMAL, "manifest_version = 1", "manifest_version = 2");
        assert!(matches!(
            Manifest::parse(&text),
            Err(Error::ManifestVersion(2))
        ));
    }

    #[test]
    fn rejects_bad_names() {
        for name in ["", "Bad\\nName", "Tab\\there"] {
            let text = with(MINIMAL, "name = \"Min\"", &format!("name = \"{name}\""));
            assert!(
                matches!(Manifest::parse(&text), Err(Error::Name(_))),
                "{name:?}"
            );
        }
    }

    #[test]
    fn rejects_bad_versions() {
        for v in ["1", "1.2", "v1.2.3", "1.2.3.4", "01.2.3"] {
            let text = with(MINIMAL, "0.1.0", v);
            assert!(
                matches!(Manifest::parse(&text), Err(Error::Version(..))),
                "{v}"
            );
        }
    }

    #[test]
    fn rejects_escaping_paths() {
        for path in [
            "",
            "/bin/notes",
            "../notes",
            "bin/../../notes",
            "bin/\\u0000x",
        ] {
            for field in ["executable", "icon"] {
                let text = if field == "executable" {
                    with(
                        MINIMAL,
                        "executable = \"min\"",
                        &format!("executable = \"{path}\""),
                    )
                } else {
                    with(
                        MINIMAL,
                        "executable = \"min\"",
                        &format!("executable = \"min\"\nicon = \"{path}\""),
                    )
                };
                assert!(
                    matches!(Manifest::parse(&text), Err(Error::Path { .. })),
                    "{field} {path:?}"
                );
            }
        }
    }

    /// These values end up in a `.desktop` file, where a newline would start a new line such
    /// as `Exec=`.
    #[test]
    fn rejects_control_characters_in_text_that_reaches_a_desktop_file() {
        for edit in [
            (
                "name = \"Min\"",
                "description = \"a\\nExec=evil\"\nname = \"Min\"",
            ),
            (
                "name = \"Min\"",
                "categories = [\"Utility\\nExec=evil\"]\nname = \"Min\"",
            ),
            (
                "executable = \"min\"",
                "executable = \"min\"\nicon = \"x\\ny.png\"",
            ),
            ("executable = \"min\"", "executable = \"m\\nin\""),
            (
                "[permissions]",
                "[permissions.dbus]\ntalk = [\"org.a\\nExec=evil\"]",
            ),
        ] {
            let text = with(MINIMAL, edit.0, edit.1);
            assert!(Manifest::parse(&text).is_err(), "{}", edit.1);
        }
    }

    #[test]
    fn rejects_bad_categories() {
        for category in ["", "Util ity", "A;B", "Utility;"] {
            let text = with(
                MINIMAL,
                "name = \"Min\"",
                &format!("name = \"Min\"\ncategories = [\"{category}\"]"),
            );
            assert!(
                matches!(Manifest::parse(&text), Err(Error::Field("categories", _))),
                "{category:?}"
            );
        }
    }

    #[test]
    fn rejects_bad_filesystem_entries() {
        let entry = |dir: &str, access: &str| {
            format!("[[permissions.filesystem]]\ndir = \"{dir}\"\naccess = \"{access}\"\n")
        };
        for body in [
            entry("home", "read"),
            entry("documents", "execute"),
            entry("documents", "read") + &entry("documents", "write"),
        ] {
            let text = with(
                MINIMAL,
                "[permissions]\n",
                &format!("[permissions]\n{body}"),
            );
            assert!(Manifest::parse(&text).is_err(), "{body}");
        }
    }

    #[test]
    fn rejects_bad_dbus_names() {
        for name in ["", "has space"] {
            let text = with(
                MINIMAL,
                "[permissions]",
                &format!("[permissions.dbus]\ntalk = [\"{name}\"]"),
            );
            assert!(
                matches!(Manifest::parse(&text), Err(Error::Permissions(_))),
                "{name:?}"
            );
        }
    }

    fn daemon_text(table: &str) -> String {
        format!("{MINIMAL}\n[daemon]\n{table}\n")
    }

    #[test]
    fn a_daemon_is_a_program_of_the_bundle_and_one_socket() {
        let m = Manifest::parse(&daemon_text(
            "executable = \"bin/ratmos\"\nargs = [\"daemon\"]\nsocket = \"atmos/ratmos.sock\"",
        ))
        .unwrap();
        assert_eq!(
            m.permissions.daemon,
            Some(Daemon {
                executable: "bin/ratmos".into(),
                args: vec!["daemon".into()],
                socket: "atmos/ratmos.sock".into()
            })
        );
        // Arguments are optional, and nothing else is asked for when there is no daemon.
        let m = Manifest::parse(&daemon_text("executable = \"d\"\nsocket = \"d.sock\"")).unwrap();
        assert_eq!(m.permissions.daemon.unwrap().args, Vec::<String>::new());
        assert_eq!(Manifest::parse(MINIMAL).unwrap().permissions.daemon, None);
    }

    #[test]
    fn a_daemon_that_is_not_plainly_inside_the_bundle_and_the_runtime_directory_is_refused() {
        let socket = "socket = \"a/s.sock\"";
        for bad in [
            format!("executable = \"../d\"\n{socket}"),
            format!("executable = \"/usr/bin/d\"\n{socket}"),
            format!("executable = \"\"\n{socket}"),
            "executable = \"d\"\nsocket = \"\"".to_owned(),
            "executable = \"d\"\nsocket = \"/run/user/1000/s\"".to_owned(),
            "executable = \"d\"\nsocket = \"../s\"".to_owned(),
            "executable = \"d\"\nsocket = \"a/../../s\"".to_owned(),
            "executable = \"d\"\nsocket = \"./s\"".to_owned(),
            format!("executable = \"d\"\nargs = [\"a\\u0000b\"]\n{socket}"),
            "executable = \"d\"".to_owned(),
            format!("executable = \"d\"\n{socket}\nextra = 1"),
        ] {
            assert!(Manifest::parse(&daemon_text(&bad)).is_err(), "{bad}");
        }
    }

    /// The compositor's command socket, the buses and the display and sound servers are never
    /// granted (`docs/spec/sandbox-baseline.md`), so a daemon cannot name them as its own.
    #[test]
    fn a_daemon_cannot_claim_a_socket_that_belongs_to_something_else() {
        for socket in [
            "hypr/abc/.socket.sock",
            "hypr",
            "wayland-1",
            "pipewire-0",
            "pulse/native",
            "bus",
            "systemd/private",
            "gnupg/S.gpg-agent",
            "sway-ipc.1000.1.sock",
        ] {
            let text = daemon_text(&format!("executable = \"d\"\nsocket = \"{socket}\""));
            assert!(
                matches!(Manifest::parse(&text), Err(Error::Permissions(_))),
                "{socket}"
            );
        }
        for fine in ["atmos/ratmos.sock", "notes.sock", "hyprland-notes/a.sock"] {
            let text = daemon_text(&format!("executable = \"d\"\nsocket = \"{fine}\""));
            // A name that merely starts like a reserved one is still refused: the check is on
            // the first component's prefix, which errs toward refusing.
            let parsed = Manifest::parse(&text);
            assert_eq!(parsed.is_ok(), !fine.starts_with("hypr"), "{fine}");
        }
    }

    #[test]
    fn precedence_ignores_build_metadata_and_orders_prereleases() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert_eq!(precedence(&v("1.2.0+a"), &v("1.2.0+b")), Ordering::Equal);
        assert_eq!(precedence(&v("1.2.0-beta.1"), &v("1.2.0")), Ordering::Less);
        assert_eq!(precedence(&v("1.2.1"), &v("1.2.0")), Ordering::Greater);
    }
}
