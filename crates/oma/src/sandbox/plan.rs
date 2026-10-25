//! What the sandbox looks like, decided before anything privileged happens (ADR 0005, ADR 0012,
//! `docs/spec/sandbox-baseline.md`). A [`Plan`] is plain data: tests check it without
//! namespaces, and [`super::mounts`] and [`super::init`] carry it out.

use std::fs;
use std::path::{Path, PathBuf};

use crate::fmt::{Access, Permissions, UserDir};
use crate::sandbox::landlock::Rights;
use crate::userdirs::{Resolved, Unavailable};

/// One step in building the new root. Destinations are absolute paths inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// A directory, with its parents.
    Dir(PathBuf),
    Tmpfs(PathBuf),
    /// A fresh procfs of the application's own PID namespace (ADR 0024).
    Proc(PathBuf),
    /// Bind a host file or directory (recursively) at a destination.
    Bind {
        src: PathBuf,
        dst: PathBuf,
        readonly: bool,
        device: bool,
    },
    Symlink {
        dst: PathBuf,
        target: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub ops: Vec<Op>,
    pub landlock: Vec<(PathBuf, Rights)>,
    /// The whole environment of the application.
    pub env: Vec<(String, String)>,
    /// The executable, inside the sandbox.
    pub exe: PathBuf,
    pub cwd: PathBuf,
    pub network: bool,
    /// Named directories that were asked for but cannot be given (ADR 0012).
    pub unavailable: Vec<(UserDir, Unavailable)>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("WAYLAND_DISPLAY is not set; there is no display to draw on")]
    NoDisplay,
    #[error("the Wayland socket {} does not exist", .0.display())]
    NoDisplaySocket(PathBuf),
    #[error("XDG_RUNTIME_DIR is not set")]
    NoRuntimeDir,
    #[error(
        "the `dbus` permission is not supported yet: a raw session-bus socket would reach every service, and there is no filtering proxy"
    )]
    DbusUnsupported,
    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Everything the plan depends on, so tests can replace the host.
pub struct Inputs<'a> {
    pub bundle_root: &'a Path,
    /// The Manifest's `executable`, relative to the Bundle.
    pub executable: &'a str,
    pub permissions: &'a Permissions,
    /// The application's data directory; `None` gives a private tmpfs that disappears on exit
    /// (Run once, ADR 0018).
    pub data_dir: Option<&'a Path>,
    pub inbox_dir: Option<&'a Path>,
    /// The user's real home, whose path is also `$HOME` inside.
    pub home: &'a Path,
    pub uid: u32,
    pub runtime_dir: Option<&'a Path>,
    pub wayland_display: Option<&'a str>,
    pub named_dirs: &'a Resolved,
    /// `~/.local/state/omarchy/current`, which holds the theme (`sandbox-baseline.md`).
    pub theme_dir: &'a Path,
    /// oma's mirror of the user's text size and font, with `omarchy/` and `fontconfig/` in it
    /// (ADR 0044). A part that is not there is not mounted.
    pub settings_dir: &'a Path,
    pub host_etc: &'a Path,
    pub host_root: &'a Path,
    /// Variables copied from the host when set.
    pub host_env: &'a dyn Fn(&str) -> Option<String>,
}

const PASSED_ENV: &[&str] = &[
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    "TERM",
    "USER",
    "LOGNAME",
    "TZ",
];
const ETC_BASELINE: &[&str] = &[
    "ld.so.cache",
    "ld.so.conf",
    "ld.so.conf.d",
    "fonts",
    "localtime",
    "os-release",
    "passwd",
    "group",
    "nsswitch.conf",
    "xdg",
];
const ETC_NETWORK: &[&str] = &["hosts", "resolv.conf", "ssl", "ca-certificates"];
const ETC_AUDIO: &[&str] = &["pipewire", "alsa"];
const ETC_GPU: &[&str] = &["vulkan", "drirc.d"];
const DEVICES: &[&str] = &["null", "zero", "full", "random", "urandom", "tty"];
const ROOT_LINKS: &[&str] = &["bin", "lib", "lib64", "sbin"];

/// Where the Bundle appears inside the sandbox, read-only.
pub const APP: &str = "/app";
/// Where the application's own inbox appears with `updates`.
pub const INBOX: &str = "/inbox";

impl Plan {
    pub fn new(i: &Inputs) -> Result<Self, Error> {
        let p = i.permissions;
        if !p.dbus_talk.is_empty() {
            return Err(Error::DbusUnsupported);
        }
        let display = i.wayland_display.ok_or(Error::NoDisplay)?;
        let runtime = i.runtime_dir.ok_or(Error::NoRuntimeDir)?;
        let wayland_src = if Path::new(display).is_absolute() {
            PathBuf::from(display)
        } else {
            runtime.join(display)
        };
        if !wayland_src.exists() {
            return Err(Error::NoDisplaySocket(wayland_src));
        }
        let wayland_name = wayland_src
            .file_name()
            .expect("a socket has a name")
            .to_string_lossy()
            .into_owned();

        let run_user = PathBuf::from(format!("/run/user/{}", i.uid));
        let mut ops = Vec::new();
        let mut landlock: Vec<(PathBuf, Rights)> = Vec::new();
        let dir = |p: &str| Op::Dir(PathBuf::from(p));

        // The /proc mount needs a fully visible /proc to exist, so it comes first (ADR 0024).
        for d in ["/proc", "/usr", "/dev", "/tmp", "/run", "/etc", APP] {
            ops.push(dir(d));
        }
        ops.push(Op::Proc("/proc".into()));
        landlock.push(("/proc".into(), Rights::All));

        ops.push(Op::Bind {
            src: i.host_root.join("usr"),
            dst: "/usr".into(),
            readonly: true,
            device: false,
        });
        landlock.push(("/usr".into(), Rights::ReadExec));
        for name in ROOT_LINKS {
            if let Ok(target) = fs::read_link(i.host_root.join(name)) {
                ops.push(Op::Symlink {
                    dst: Path::new("/").join(name),
                    target,
                });
            }
        }

        // /dev: a minimal private set.
        ops.push(Op::Tmpfs("/dev".into()));
        for name in DEVICES {
            let src = PathBuf::from("/dev").join(name);
            if src.exists() {
                let dst = PathBuf::from("/dev").join(name);
                ops.push(Op::Bind {
                    src,
                    dst: dst.clone(),
                    readonly: false,
                    device: true,
                });
                landlock.push((dst, Rights::All));
            }
        }
        ops.push(Op::Tmpfs("/dev/shm".into()));
        landlock.push(("/dev/shm".into(), Rights::All));
        for (link, target) in [
            ("fd", "/proc/self/fd"),
            ("stdin", "/proc/self/fd/0"),
            ("stdout", "/proc/self/fd/1"),
            ("stderr", "/proc/self/fd/2"),
        ] {
            ops.push(Op::Symlink {
                dst: Path::new("/dev").join(link),
                target: target.into(),
            });
        }
        if p.gpu && Path::new("/dev/dri").exists() {
            ops.push(Op::Bind {
                src: "/dev/dri".into(),
                dst: "/dev/dri".into(),
                readonly: false,
                device: true,
            });
            landlock.push(("/dev/dri".into(), Rights::All));
            gpu_sysfs(&mut ops, &mut landlock);
        }

        ops.push(Op::Tmpfs("/tmp".into()));
        landlock.push(("/tmp".into(), Rights::All));

        // /run holds only what the permissions bring.
        ops.push(Op::Tmpfs("/run".into()));
        landlock.push(("/run".into(), Rights::All));
        ops.push(Op::Dir(run_user.clone()));
        ops.push(Op::Bind {
            src: wayland_src,
            dst: run_user.join(&wayland_name),
            readonly: false,
            device: false,
        });
        if p.audio {
            let socket = runtime.join("pipewire-0");
            if socket.exists() {
                ops.push(Op::Bind {
                    src: socket,
                    dst: run_user.join("pipewire-0"),
                    readonly: false,
                    device: false,
                });
            }
        }

        // /etc: a private tmpfs with only the entries the permissions justify.
        ops.push(Op::Tmpfs("/etc".into()));
        landlock.push(("/etc".into(), Rights::Read));
        let mut entries: Vec<&str> = ETC_BASELINE.to_vec();
        if p.network {
            entries.extend(ETC_NETWORK);
        }
        if p.audio {
            entries.extend(ETC_AUDIO);
        }
        if p.gpu {
            entries.extend(ETC_GPU);
        }
        for name in entries {
            etc_entry(&mut ops, &mut landlock, i.host_etc, name)?;
        }

        // The Bundle, read-only.
        ops.push(Op::Bind {
            src: i.bundle_root.to_owned(),
            dst: APP.into(),
            readonly: true,
            device: false,
        });
        landlock.push((APP.into(), Rights::ReadExec));

        // $HOME is the real home's path, backed by the application's own data directory.
        ops.push(Op::Dir(i.home.to_owned()));
        match i.data_dir {
            Some(data) => ops.push(Op::Bind {
                src: data.to_owned(),
                dst: i.home.to_owned(),
                readonly: false,
                device: false,
            }),
            None => ops.push(Op::Tmpfs(i.home.to_owned())),
        }
        landlock.push((i.home.to_owned(), Rights::All));

        // Omarchy's theme: the parent directory, because Omarchy replaces `theme/` itself.
        if i.theme_dir.exists() {
            let theme_dst = i.home.join(".local/state/omarchy/current");
            ops.push(Op::Dir(theme_dst.clone()));
            ops.push(Op::Bind {
                src: i.theme_dir.to_owned(),
                dst: theme_dst.clone(),
                readonly: true,
                device: false,
            });
            landlock.push((theme_dst, Rights::Read));
        }

        // The user's text size and font, from oma's copy of them: Omarchy replaces the real
        // files by renaming, so a mount of one would go stale, and their directory holds more
        // than an application should read (ADR 0044). Each part appears where the real one is.
        for name in ["omarchy", "fontconfig"] {
            let src = i.settings_dir.join(name);
            if src.is_dir() {
                let dst = i.home.join(".config").join(name);
                ops.push(Op::Dir(dst.clone()));
                ops.push(Op::Bind {
                    src,
                    dst: dst.clone(),
                    readonly: true,
                    device: false,
                });
                landlock.push((dst, Rights::Read));
            }
        }

        // Named user directories.
        let mut unavailable = Vec::new();
        for grant in &p.filesystem {
            match i.named_dirs.get(grant.dir) {
                Ok(host) => {
                    let dst = i.home.join(dir_name(host));
                    let readonly = grant.access == Access::Read;
                    ops.push(Op::Dir(dst.clone()));
                    ops.push(Op::Bind {
                        src: host.clone(),
                        dst: dst.clone(),
                        readonly,
                        device: false,
                    });
                    landlock.push((dst, if readonly { Rights::Read } else { Rights::All }));
                }
                Err(why) => unavailable.push((grant.dir, *why)),
            }
        }

        if p.updates
            && let Some(inbox) = i.inbox_dir
        {
            ops.push(dir(INBOX));
            ops.push(Op::Bind {
                src: inbox.to_owned(),
                dst: INBOX.into(),
                readonly: false,
                device: false,
            });
            landlock.push((INBOX.into(), Rights::All));
        }

        let mut env = vec![
            ("HOME".to_owned(), i.home.display().to_string()),
            ("PATH".to_owned(), "/usr/local/bin:/usr/bin".to_owned()),
            ("XDG_RUNTIME_DIR".to_owned(), run_user.display().to_string()),
            ("WAYLAND_DISPLAY".to_owned(), wayland_name),
            ("XDG_SESSION_TYPE".to_owned(), "wayland".to_owned()),
            // The mounted `shell.toml` is oma's copy. odk-theme reads `[motion] reduced`
            // only when this is set (ADR 0048).
            ("OMA_SANDBOX".to_owned(), "1".to_owned()),
        ];
        for name in PASSED_ENV {
            if let Some(value) = (i.host_env)(name) {
                env.push(((*name).to_owned(), value));
            }
        }
        if p.updates && i.inbox_dir.is_some() {
            env.push(("OMA_INBOX".to_owned(), INBOX.to_owned()));
        }

        Ok(Self {
            ops,
            landlock,
            env,
            exe: Path::new(APP).join(i.executable),
            cwd: i.home.to_owned(),
            network: p.network,
            unavailable,
        })
    }
}

/// The slice of `/sys` that graphics drivers read to identify a GPU: for each node in
/// `/dev/dri`, its `/sys/dev/char/<major>:<minor>` link and the PCI device directory behind it,
/// and nothing else. The rest of `/sys` shows hardware serials, MAC addresses and more.
fn gpu_sysfs(ops: &mut Vec<Op>, landlock: &mut Vec<(PathBuf, Rights)>) {
    use std::os::unix::fs::MetadataExt;
    let Ok(nodes) = fs::read_dir("/dev/dri") else {
        return;
    };
    let mut devices: Vec<PathBuf> = Vec::new();
    let mut links: Vec<PathBuf> = Vec::new();
    let mut any = false;
    for node in nodes.flatten() {
        let Ok(meta) = fs::metadata(node.path()) else {
            continue;
        };
        let (major, minor) = (libc::major(meta.rdev()), libc::minor(meta.rdev()));
        let link = PathBuf::from(format!("/sys/dev/char/{major}:{minor}"));
        let (Ok(target), Ok(device)) =
            (fs::read_link(&link), fs::canonicalize(link.join("device")))
        else {
            continue;
        };
        if !any {
            ops.push(Op::Dir("/sys/dev/char".into()));
            landlock.push(("/sys/dev/char".into(), Rights::Read));
            any = true;
        }
        if !links.contains(&link) {
            ops.push(Op::Symlink {
                dst: link.clone(),
                target,
            });
            links.push(link);
        }
        if !devices.contains(&device) {
            ops.push(Op::Dir(device.clone()));
            ops.push(Op::Bind {
                src: device.clone(),
                dst: device.clone(),
                readonly: true,
                device: false,
            });
            landlock.push((device.clone(), Rights::Read));
            devices.push(device);
        }
    }
}

/// The last component of a host directory, which is its name under `$HOME` inside.
fn dir_name(host: &Path) -> &std::ffi::OsStr {
    host.file_name()
        .expect("a named directory is strictly inside home, so it has a name")
}

/// One `/etc` entry. A symlink is recreated as it is; if its target lies outside what is
/// exposed, exactly that one target is exposed too (`sandbox-baseline.md`, "Symlinks").
fn etc_entry(
    ops: &mut Vec<Op>,
    landlock: &mut Vec<(PathBuf, Rights)>,
    host_etc: &Path,
    name: &str,
) -> Result<(), Error> {
    let src = host_etc.join(name);
    let Ok(meta) = fs::symlink_metadata(&src) else {
        return Ok(());
    };
    let dst = Path::new("/etc").join(name);
    if meta.file_type().is_symlink() {
        let target = fs::read_link(&src).map_err(|source| Error::Io {
            path: src.clone(),
            source,
        })?;
        ops.push(Op::Symlink {
            dst,
            target: target.clone(),
        });
        let resolved = fs::canonicalize(&src).ok();
        if let Some(resolved) = resolved {
            let inside = resolved.starts_with("/usr") || resolved.starts_with("/etc");
            if !inside {
                if let Some(parent) = resolved.parent() {
                    ops.push(Op::Dir(parent.to_owned()));
                }
                ops.push(Op::Bind {
                    src: resolved.clone(),
                    dst: resolved.clone(),
                    readonly: true,
                    device: false,
                });
                landlock.push((resolved, Rights::Read));
            }
        }
    } else {
        ops.push(Op::Bind {
            src,
            dst,
            readonly: true,
            device: false,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    use tempfile::TempDir;

    use super::*;
    use crate::fmt::FsGrant;
    use crate::userdirs;

    struct World {
        dir: TempDir,
        home: PathBuf,
        runtime: PathBuf,
        etc: PathBuf,
        _socket: UnixListener,
    }

    fn world() -> World {
        let dir = TempDir::new().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let (home, runtime, etc) = (root.join("home"), root.join("run"), root.join("etc"));
        for d in [
            &home,
            &runtime,
            &etc,
            &home.join("Documents"),
            &home.join("Music"),
        ] {
            fs::create_dir_all(d).unwrap();
        }
        for f in ["ld.so.cache", "passwd", "hosts", "machine-id"] {
            fs::write(etc.join(f), "x").unwrap();
        }
        for d in ["ssl", "pipewire", "vulkan"] {
            fs::create_dir(etc.join(d)).unwrap();
        }
        fs::write(home.join(".config-user-dirs"), "").unwrap();
        let socket = UnixListener::bind(runtime.join("wayland-1")).unwrap();
        World {
            dir,
            home,
            runtime,
            etc,
            _socket: socket,
        }
    }

    fn plan(w: &World, permissions: &Permissions) -> Result<Plan, Error> {
        fs::write(w.home.join("user-dirs.dirs"), "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\nXDG_VIDEOS_DIR=\"$HOME/Nope\"\n").unwrap();
        let named = userdirs::resolve(&w.home, &w.home.join("user-dirs.dirs"));
        let data = w.dir.path().join("data");
        let inbox = w.dir.path().join("inbox");
        let bundle = w.dir.path().join("Tee.oma");
        let theme = w.dir.path().join("theme");
        let settings = w.dir.path().join("settings");
        let env = |name: &str| (name == "LANG").then(|| "en_US.UTF-8".to_owned());
        Plan::new(&Inputs {
            bundle_root: &bundle,
            executable: "bin/app",
            permissions,
            data_dir: Some(&data),
            inbox_dir: Some(&inbox),
            home: &w.home,
            uid: 1000,
            runtime_dir: Some(&w.runtime),
            wayland_display: Some("wayland-1"),
            named_dirs: &named,
            theme_dir: &theme,
            settings_dir: &settings,
            host_etc: &w.etc,
            host_root: Path::new("/"),
            host_env: &env,
        })
    }

    fn binds(plan: &Plan) -> Vec<(&Path, &Path, bool)> {
        plan.ops
            .iter()
            .filter_map(|op| match op {
                Op::Bind {
                    src, dst, readonly, ..
                } => Some((src.as_path(), dst.as_path(), *readonly)),
                _ => None,
            })
            .collect()
    }

    fn has_dst(plan: &Plan, dst: &str) -> bool {
        binds(plan).iter().any(|(_, d, _)| *d == Path::new(dst))
    }

    #[test]
    fn the_baseline_exposes_only_what_the_spec_lists() {
        let w = world();
        let p = plan(&w, &Permissions::default()).unwrap();
        let dsts: Vec<_> = binds(&p)
            .into_iter()
            .map(|(_, d, _)| d.to_owned())
            .collect();
        assert!(dsts.contains(&PathBuf::from("/usr")));
        assert!(dsts.contains(&PathBuf::from("/app")));
        assert!(dsts.contains(&w.home));
        assert!(dsts.contains(&PathBuf::from("/etc/passwd")));
        assert!(dsts.contains(&PathBuf::from("/dev/null")));
        // Nothing from the permission-gated lists.
        for gated in [
            "/etc/hosts",
            "/etc/ssl",
            "/etc/pipewire",
            "/etc/vulkan",
            "/etc/machine-id",
            "/dev/dri",
            "/inbox",
        ] {
            assert!(!has_dst(&p, gated), "{gated}");
        }
        assert!(!p.network);
        assert!(p.ops.contains(&Op::Proc("/proc".into())));
        assert_eq!(p.exe, Path::new("/app/bin/app"));
        assert_eq!(p.cwd, w.home);
    }

    /// The user's text size and font (ADR 0044): oma's copy, not the real files, appears where
    /// the real ones are, read-only, and only the parts that exist.
    #[test]
    fn oma_s_copy_of_the_text_size_and_font_is_mounted_read_only_where_the_real_ones_are() {
        let w = world();
        let settings = w.dir.path().join("settings");
        let config = w.home.join(".config");
        let p = plan(&w, &Permissions::default()).unwrap();
        for name in ["omarchy", "fontconfig"] {
            assert!(!has_dst(&p, config.join(name).to_str().unwrap()), "{name}");
        }

        fs::create_dir_all(settings.join("omarchy")).unwrap();
        let p = plan(&w, &Permissions::default()).unwrap();
        assert!(has_dst(&p, config.join("omarchy").to_str().unwrap()));
        assert!(
            !has_dst(&p, config.join("fontconfig").to_str().unwrap()),
            "a part that is not there is not mounted"
        );

        fs::create_dir_all(settings.join("fontconfig")).unwrap();
        let p = plan(&w, &Permissions::default()).unwrap();
        for name in ["omarchy", "fontconfig"] {
            let (src, dst) = (settings.join(name), config.join(name));
            assert!(
                binds(&p).contains(&(src.as_path(), dst.as_path(), true)),
                "{name}"
            );
            assert!(p.landlock.contains(&(dst, Rights::Read)), "{name}");
        }
        // Nothing of the real `~/.config` is a source.
        assert!(binds(&p).iter().all(|(src, ..)| !src.starts_with(&config)));
    }

    /// Hyprland's socket accepts commands (ADR 0048), so an application that could open it would
    /// not be sandboxed: nothing of `$XDG_RUNTIME_DIR/hypr` is bound or granted, whatever the
    /// application asked for.
    #[test]
    fn nothing_of_the_compositors_runtime_directory_is_reachable() {
        let w = world();
        let instance = w.runtime.join("hypr/abc");
        fs::create_dir_all(&instance).unwrap();
        fs::write(instance.join(".socket.sock"), "").unwrap();
        fs::write(instance.join(".socket2.sock"), "").unwrap();
        let everything = Permissions {
            network: true,
            gpu: true,
            audio: true,
            updates: true,
            filesystem: Vec::new(),
            dbus_talk: Vec::new(),
        };
        for permissions in [Permissions::default(), everything] {
            let p = plan(&w, &permissions).unwrap();
            let paths = || {
                p.ops
                    .iter()
                    .flat_map(|op| match op {
                        Op::Bind { src, dst, .. } => vec![src.clone(), dst.clone()],
                        Op::Dir(d) | Op::Tmpfs(d) | Op::Proc(d) => vec![d.clone()],
                        Op::Symlink { dst, target } => vec![dst.clone(), target.clone()],
                    })
                    .chain(p.landlock.iter().map(|(path, _)| path.clone()))
            };
            for path in paths() {
                assert!(
                    !path.components().any(|c| c.as_os_str() == "hypr"),
                    "{} reaches Hyprland's directory",
                    path.display()
                );
            }
            // The whole runtime directory is never what is bound.
            assert!(
                binds(&p)
                    .iter()
                    .all(|(src, ..)| *src != w.runtime.as_path()),
                "the host's runtime directory as a whole"
            );
        }
    }

    #[test]
    fn the_proc_mount_comes_before_anything_covers_the_old_root() {
        let w = world();
        let p = plan(&w, &Permissions::default()).unwrap();
        let proc = p
            .ops
            .iter()
            .position(|o| *o == Op::Proc("/proc".into()))
            .unwrap();
        let first_bind = p
            .ops
            .iter()
            .position(|o| matches!(o, Op::Bind { .. }))
            .unwrap();
        assert!(proc < first_bind);
    }

    #[test]
    fn the_bundle_and_usr_are_read_only_and_data_is_writable() {
        let w = world();
        let p = plan(&w, &Permissions::default()).unwrap();
        for (src, dst, readonly) in binds(&p) {
            match dst.to_str().unwrap() {
                "/usr" | "/app" => assert!(readonly, "{dst:?}"),
                d if d == w.home.to_str().unwrap() => {
                    assert!(!readonly);
                    assert!(src.ends_with("data"));
                }
                _ => {}
            }
        }
    }

    #[test]
    fn each_permission_brings_its_own_entries() {
        let w = world();
        let p = plan(
            &w,
            &Permissions {
                network: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(p.network);
        assert!(has_dst(&p, "/etc/hosts") && has_dst(&p, "/etc/ssl"));
        assert!(!has_dst(&p, "/etc/pipewire"));

        let p = plan(
            &w,
            &Permissions {
                audio: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(has_dst(&p, "/etc/pipewire"));

        let p = plan(
            &w,
            &Permissions {
                gpu: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(has_dst(&p, "/etc/vulkan"));
        assert_eq!(has_dst(&p, "/dev/dri"), Path::new("/dev/dri").exists());

        let p = plan(
            &w,
            &Permissions {
                updates: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(has_dst(&p, "/inbox"));
        assert!(
            p.env
                .contains(&("OMA_INBOX".to_owned(), "/inbox".to_owned()))
        );
    }

    #[test]
    fn named_directories_appear_under_home_and_unavailable_ones_are_reported() {
        let w = world();
        let perms = Permissions {
            filesystem: vec![
                FsGrant {
                    dir: UserDir::Documents,
                    access: Access::Write,
                },
                FsGrant {
                    dir: UserDir::Music,
                    access: Access::Read,
                },
                FsGrant {
                    dir: UserDir::Videos,
                    access: Access::Read,
                },
                FsGrant {
                    dir: UserDir::Desktop,
                    access: Access::Read,
                },
            ],
            ..Default::default()
        };
        let p = plan(&w, &perms).unwrap();
        let documents = binds(&p)
            .into_iter()
            .find(|(_, d, _)| *d == w.home.join("Documents"))
            .unwrap();
        assert!(!documents.2, "write access is not read-only");
        let music = binds(&p)
            .into_iter()
            .find(|(_, d, _)| *d == w.home.join("Music"))
            .unwrap();
        assert!(music.2);
        assert_eq!(
            p.unavailable,
            [
                (UserDir::Videos, Unavailable::Missing),
                (UserDir::Desktop, Unavailable::NoEntry)
            ]
        );
    }

    #[test]
    fn the_real_home_itself_is_never_exposed() {
        let w = world();
        let p = plan(&w, &Permissions::default()).unwrap();
        assert!(binds(&p).iter().all(|(src, _, _)| *src != w.home));
    }

    #[test]
    fn an_etc_symlink_to_the_outside_exposes_exactly_that_target() {
        let w = world();
        let outside = w.dir.path().join("resolved");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("stub-resolv.conf"), "nameserver 127.0.0.53").unwrap();
        fs::write(outside.join("other"), "secret").unwrap();
        symlink(outside.join("stub-resolv.conf"), w.etc.join("resolv.conf")).unwrap();
        let p = plan(
            &w,
            &Permissions {
                network: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            p.ops.iter().any(
                |o| matches!(o, Op::Symlink { dst, .. } if dst == Path::new("/etc/resolv.conf"))
            )
        );
        let target = fs::canonicalize(outside.join("stub-resolv.conf")).unwrap();
        assert!(
            binds(&p)
                .iter()
                .any(|(s, d, ro)| *s == target && *d == target && *ro)
        );
        assert!(!binds(&p).iter().any(|(s, _, _)| s.ends_with("other")));
    }

    #[test]
    fn an_etc_symlink_into_usr_is_just_recreated() {
        let w = world();
        symlink("/usr/share/zoneinfo/UTC", w.etc.join("localtime")).unwrap();
        let p = plan(&w, &Permissions::default()).unwrap();
        assert!(
            p.ops.iter().any(
                |o| matches!(o, Op::Symlink { dst, .. } if dst == Path::new("/etc/localtime"))
            )
        );
        assert!(!has_dst(&p, "/usr/share/zoneinfo/UTC"));
    }

    #[test]
    fn the_environment_is_cleared_and_rebuilt() {
        let w = world();
        let p = plan(&w, &Permissions::default()).unwrap();
        let get = |k: &str| p.env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("HOME"), w.home.to_str());
        assert_eq!(get("XDG_RUNTIME_DIR"), Some("/run/user/1000"));
        assert_eq!(get("WAYLAND_DISPLAY"), Some("wayland-1"));
        assert_eq!(get("LANG"), Some("en_US.UTF-8"));
        assert_eq!(get("DISPLAY"), None);
        assert_eq!(get("SSH_AUTH_SOCK"), None);
        assert_eq!(get("OMA_SANDBOX"), Some("1"));
    }

    #[test]
    fn refuses_dbus_and_a_missing_display() {
        let w = world();
        let dbus = Permissions {
            dbus_talk: vec!["org.a".into()],
            ..Default::default()
        };
        assert!(matches!(plan(&w, &dbus), Err(Error::DbusUnsupported)));
        fs::remove_file(w.runtime.join("wayland-1")).unwrap();
        assert!(matches!(
            plan(&w, &Permissions::default()),
            Err(Error::NoDisplaySocket(_))
        ));
    }
}
