//! A Bundle file seen as a directory (ADR 0053, ADR 0065): a read-only loop device of the file,
//! set up and mounted by udisks as the desktop mounts a disk image, and mounted `noexec`, so that
//! nothing in it runs outside the sandbox. `odk oma mount` makes or finds a view, and
//! `--unmount` takes it away.
//!
//! What the kernel has set up is read from `/sys/block` and `/proc/self/mountinfo`, never from
//! what a tool prints. [`Kernel`] says where they are, so a test can point it at files of its
//! own, and [`Udisks`] is the one way to change anything.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{ErrorKind, Read};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What a Bundle file starts with (ADR 0052).
const MAGIC: &[u8; 4] = b"OMA1";
/// Where the EROFS superblock starts, after the bytes the preamble uses.
const EROFS_OFFSET: usize = 1024;
/// The EROFS superblock's magic, as it is on disk.
const EROFS_MAGIC: [u8; 4] = 0xE0F5_E1E2_u32.to_le_bytes();

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} is a directory, which needs no mount", .0.display())]
    Directory(PathBuf),
    #[error("{} is not a Bundle file: it does not start with OMA1 and an EROFS image", .0.display())]
    NotABundle(PathBuf),
    #[error("{} is not mounted", .0.display())]
    NotMounted(PathBuf),
    #[error("{0}")]
    Tool(String),
}

/// Where the kernel says which loop devices and mounts there are.
pub struct Kernel {
    /// `/sys/block`.
    pub block: PathBuf,
    /// `/proc/self/mountinfo`.
    pub mountinfo: PathBuf,
}

/// A loop device: its node, such as `/dev/loop0`, and its `major:minor`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Loop {
    device: PathBuf,
    number: String,
}

/// Where a loop device is mounted, and whether that mount runs programs.
struct Mount {
    dir: PathBuf,
    noexec: bool,
}

impl Kernel {
    pub fn host() -> Self {
        Kernel {
            block: "/sys/block".into(),
            mountinfo: "/proc/self/mountinfo".into(),
        }
    }

    /// The loop devices whose backing file is `file`, by device and inode: the views of it,
    /// whoever made them.
    fn loops_of(&self, file: &fs::Metadata) -> Vec<Loop> {
        let Ok(entries) = fs::read_dir(&self.block) else {
            return Vec::new();
        };
        let mut loops: Vec<Loop> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_name().as_encoded_bytes().starts_with(b"loop"))
            .filter_map(|e| {
                let backing = fs::read(e.path().join("loop/backing_file")).ok()?;
                let backing = backing.strip_suffix(b"\n").unwrap_or(&backing);
                let backing = fs::metadata(OsString::from_vec(backing.to_vec())).ok()?;
                if backing.dev() != file.dev() || backing.ino() != file.ino() {
                    return None;
                }
                let number = fs::read_to_string(e.path().join("dev")).ok()?;
                Some(Loop {
                    device: Path::new("/dev").join(e.file_name()),
                    number: number.trim().to_owned(),
                })
            })
            .collect();
        loops.sort_by(|a, b| a.device.cmp(&b.device));
        loops
    }

    /// Where `device` is mounted: the first mount of its root in `mountinfo`.
    fn mount_of(&self, device: &Loop) -> Result<Option<Mount>, Error> {
        let text = fs::read(&self.mountinfo).map_err(|source| Error::Io {
            path: self.mountinfo.clone(),
            source,
        })?;
        Ok(text.split(|b| *b == b'\n').find_map(|line| {
            let fields: Vec<&[u8]> = line.split(|b| *b == b' ').collect();
            let mounted =
                fields.len() > 5 && fields[2] == device.number.as_bytes() && fields[3] == b"/";
            mounted.then(|| Mount {
                dir: unescape(fields[4]),
                noexec: fields[5].split(|b| *b == b',').any(|o| o == b"noexec"),
            })
        }))
    }
}

/// A path as `mountinfo` writes it, with a space, tab, newline or backslash as `\` and three
/// octal digits.
fn unescape(field: &[u8]) -> PathBuf {
    let mut out = Vec::with_capacity(field.len());
    let mut i = 0;
    while i < field.len() {
        match field.get(i + 1..i + 4) {
            Some(digits)
                if field[i] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)) =>
            {
                out.push(
                    digits
                        .iter()
                        .fold(0u8, |n, d| n.wrapping_mul(8).wrapping_add(d - b'0')),
                );
                i += 4;
            }
            _ => {
                out.push(field[i]);
                i += 1;
            }
        }
    }
    PathBuf::from(OsString::from_vec(out))
}

/// What a view asks of udisks. The real one is [`Udisksctl`]; tests put their own, so no test
/// sets up a loop device on the machine it runs on.
pub trait Udisks {
    fn loop_setup(&self, file: &Path) -> Result<(), String>;
    fn mount(&self, device: &Path) -> Result<(), String>;
    fn unmount(&self, device: &Path) -> Result<(), String>;
    /// Sets the loop device to go away when it is unmounted, by whatever unmounts it.
    fn autoclear(&self, device: &Path) -> Result<(), String>;
    fn loop_delete(&self, device: &Path) -> Result<(), String>;
}

/// `udisksctl`, and `gdbus` for the one call `udisksctl` does not make. None of them may ask
/// for a password: outside the active session that is refused, not asked.
pub struct Udisksctl;

impl Udisks for Udisksctl {
    fn loop_setup(&self, file: &Path) -> Result<(), String> {
        let args: [&OsStr; 4] = [
            "loop-setup".as_ref(),
            "--read-only".as_ref(),
            "--no-partition-scan".as_ref(),
            "--file".as_ref(),
        ];
        udisksctl(args.into_iter().chain([file.as_os_str()]))
    }

    fn mount(&self, device: &Path) -> Result<(), String> {
        let args: [&OsStr; 4] = [
            "mount".as_ref(),
            "--options".as_ref(),
            "noexec".as_ref(),
            "--block-device".as_ref(),
        ];
        udisksctl(args.into_iter().chain([device.as_os_str()]))
    }

    fn unmount(&self, device: &Path) -> Result<(), String> {
        udisksctl([
            "unmount".as_ref(),
            "--block-device".as_ref(),
            device.as_os_str(),
        ])
    }

    fn autoclear(&self, device: &Path) -> Result<(), String> {
        let name = device.file_name().unwrap_or_default().to_string_lossy();
        tool(
            "gdbus",
            "glib2",
            [
                "call",
                "--system",
                "--dest",
                "org.freedesktop.UDisks2",
                "--object-path",
                &format!("/org/freedesktop/UDisks2/block_devices/{name}"),
                "--method",
                "org.freedesktop.UDisks2.Loop.SetAutoclear",
                "true",
                "{'auth.no_user_interaction': <true>}",
            ],
        )
    }

    fn loop_delete(&self, device: &Path) -> Result<(), String> {
        udisksctl([
            "loop-delete".as_ref(),
            "--block-device".as_ref(),
            device.as_os_str(),
        ])
    }
}

fn udisksctl<'a>(args: impl IntoIterator<Item = &'a OsStr>) -> Result<(), String> {
    tool(
        "udisksctl",
        "udisks2",
        args.into_iter().chain(["--no-user-interaction".as_ref()]),
    )
}

/// Runs one of udisks' tools in the C locale, so what it says is the same everywhere, and keeps
/// what it says on standard error for a failure.
fn tool<S: AsRef<OsStr>>(
    program: &str,
    package: &str,
    args: impl IntoIterator<Item = S>,
) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => format!("{program} is not installed: it is part of {package}"),
            _ => format!("{program}: {e}"),
        })?;
    if out.status.success() {
        return Ok(());
    }
    let why = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    let why = if why.is_empty() {
        format!("{program} {}", out.status)
    } else {
        why
    };
    // Polkit lets the active local session do this without a password, and nothing else.
    if why.contains("NotAuthorized") {
        Err(format!(
            "{why}\n(udisks allows this only from the active desktop session)"
        ))
    } else {
        Err(why)
    }
}

/// The file's metadata, once its first bytes say it is a Bundle file: `OMA1`, and an EROFS
/// image after the preamble. Nothing else is mounted.
fn check(file: &Path) -> Result<fs::Metadata, Error> {
    let io = |source| Error::Io {
        path: file.to_owned(),
        source,
    };
    let mut f = fs::File::open(file).map_err(io)?;
    let meta = f.metadata().map_err(io)?;
    if meta.is_dir() {
        return Err(Error::Directory(file.to_owned()));
    }
    let mut head = [0u8; EROFS_OFFSET + 4];
    match f.read_exact(&mut head) {
        Ok(()) if head.starts_with(MAGIC) && head[EROFS_OFFSET..] == EROFS_MAGIC => Ok(meta),
        Ok(()) => Err(Error::NotABundle(file.to_owned())),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Err(Error::NotABundle(file.to_owned())),
        Err(e) => Err(io(e)),
    }
}

/// Makes the view of a Bundle file, or finds the one there is, and returns its directory.
pub fn mount(file: &Path, kernel: &Kernel, udisks: &dyn Udisks) -> Result<PathBuf, Error> {
    let meta = check(file)?;
    let (device, made) = match kernel.loops_of(&meta).into_iter().next() {
        Some(device) => (device, false),
        None => {
            udisks.loop_setup(file).map_err(Error::Tool)?;
            let device = kernel.loops_of(&meta).into_iter().next().ok_or_else(|| {
                Error::Tool(format!(
                    "udisks set up no loop device for {}",
                    file.display()
                ))
            })?;
            (device, true)
        }
    };
    let viewed = mount_noexec(kernel, udisks, &device).and_then(|dir| {
        udisks.autoclear(&device.device).map_err(Error::Tool)?;
        Ok(dir)
    });
    if viewed.is_err() && made && !matches!(kernel.mount_of(&device), Ok(Some(_))) {
        // A loop device this call set up and could not mount is not left behind.
        let _ = udisks.loop_delete(&device.device);
    }
    viewed
}

/// Mounts `device` with `noexec`, or finds it so mounted. The desktop's automounter mounts a
/// new loop device as soon as there is one, without `noexec`; that mount is taken down and
/// made again.
fn mount_noexec(kernel: &Kernel, udisks: &dyn Udisks, device: &Loop) -> Result<PathBuf, Error> {
    for _ in 0..3 {
        match kernel.mount_of(device)? {
            Some(Mount { dir, noexec: true }) => return Ok(dir),
            Some(_) => udisks.unmount(&device.device).map_err(Error::Tool)?,
            None => {}
        }
        if let Err(why) = udisks.mount(&device.device) {
            // Something mounted it in between: look again.
            if kernel.mount_of(device)?.is_none() {
                return Err(Error::Tool(why));
            }
        }
    }
    match kernel.mount_of(device)? {
        Some(Mount { dir, noexec: true }) => Ok(dir),
        _ => Err(Error::Tool(format!(
            "something else keeps mounting {} without noexec",
            device.device.display()
        ))),
    }
}

/// Takes away every view of a Bundle file: unmounts each of its loop devices and deletes it.
pub fn unmount(file: &Path, kernel: &Kernel, udisks: &dyn Udisks) -> Result<(), Error> {
    let meta = check(file)?;
    let devices = kernel.loops_of(&meta);
    if devices.is_empty() {
        return Err(Error::NotMounted(file.to_owned()));
    }
    for device in &devices {
        if kernel.mount_of(device)?.is_some() {
            udisks.unmount(&device.device).map_err(Error::Tool)?;
        }
        // One set to clear itself is gone once it is unmounted.
        let there = || kernel.loops_of(&meta).contains(device);
        if there()
            && let Err(why) = udisks.loop_delete(&device.device)
            && there()
        {
            return Err(Error::Tool(why));
        }
    }
    Ok(())
}

/// Opens a view's directory with the default file manager: `xdg-open` of the directory, not of
/// the `.oma` file, which would open the file again. It is not waited for: it runs the file
/// manager in the foreground.
pub fn open(dir: &Path) -> Result<(), Error> {
    Command::new("xdg-open")
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| {
            Error::Tool(match e.kind() {
                ErrorKind::NotFound => "xdg-open is not installed: it is part of xdg-utils".into(),
                _ => format!("xdg-open: {e}"),
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    const NUMBER: &str = "7:3";

    /// A kernel of files in a temporary directory, and a udisks that changes them as the real
    /// one changes the kernel's, and keeps what it was asked.
    struct Machine {
        dir: tempfile::TempDir,
        kernel: Kernel,
        calls: RefCell<Vec<&'static str>>,
        /// Mounts a new loop device, without `noexec`, as soon as it is set up, as `udiskie` does.
        automount: bool,
        refuse_mount: bool,
        autoclear: Cell<bool>,
    }

    impl Machine {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir(dir.path().join("block")).unwrap();
            fs::write(
                dir.path().join("mountinfo"),
                "22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n",
            )
            .unwrap();
            let kernel = Kernel {
                block: dir.path().join("block"),
                mountinfo: dir.path().join("mountinfo"),
            };
            Machine {
                dir,
                kernel,
                calls: RefCell::default(),
                automount: false,
                refuse_mount: false,
                autoclear: Cell::new(false),
            }
        }

        /// A Bundle file: `OMA1`, noise, and the EROFS magic at 1024.
        fn bundle(&self, name: &str) -> PathBuf {
            let mut bytes = vec![7u8; 4096];
            bytes[..4].copy_from_slice(MAGIC);
            bytes[EROFS_OFFSET..EROFS_OFFSET + 4].copy_from_slice(&EROFS_MAGIC);
            let path = self.dir.path().join(name);
            fs::write(&path, bytes).unwrap();
            path
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }

        fn loop_dir(&self) -> PathBuf {
            self.kernel.block.join("loop3")
        }

        /// `loop3`, backed by `file`.
        fn attach(&self, file: &Path) {
            fs::create_dir_all(self.loop_dir().join("loop")).unwrap();
            fs::write(
                self.loop_dir().join("loop/backing_file"),
                format!("{}\n", file.display()),
            )
            .unwrap();
            fs::write(self.loop_dir().join("dev"), format!("{NUMBER}\n")).unwrap();
        }

        fn mount_with(&self, options: &str) {
            let mut text = fs::read_to_string(&self.kernel.mountinfo).unwrap();
            text.push_str(&format!(
                "386 29 {NUMBER} / /run/media/me/My\\040App {options} shared:554 - erofs /dev/loop3 ro\n"
            ));
            fs::write(&self.kernel.mountinfo, text).unwrap();
        }

        /// The options of the mount of `loop3`, if it is mounted.
        fn mounted(&self) -> Option<String> {
            fs::read_to_string(&self.kernel.mountinfo)
                .unwrap()
                .lines()
                .find(|l| l.contains(NUMBER))
                .map(|l| l.split(' ').nth(5).unwrap().to_owned())
        }
    }

    impl Udisks for Machine {
        fn loop_setup(&self, file: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push("loop-setup");
            self.attach(file);
            if self.automount {
                self.mount_with("ro,nosuid,nodev,relatime");
            }
            Ok(())
        }

        fn mount(&self, _: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push("mount");
            if self.refuse_mount {
                return Err("cannot mount".into());
            }
            if self.mounted().is_some() {
                return Err("AlreadyMounted".into());
            }
            self.mount_with("ro,nosuid,nodev,noexec,relatime");
            Ok(())
        }

        fn unmount(&self, _: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push("unmount");
            let text: String = fs::read_to_string(&self.kernel.mountinfo)
                .unwrap()
                .lines()
                .filter(|l| !l.contains(NUMBER))
                .map(|l| format!("{l}\n"))
                .collect();
            fs::write(&self.kernel.mountinfo, text).unwrap();
            if self.autoclear.get() {
                fs::remove_dir_all(self.loop_dir()).unwrap();
            }
            Ok(())
        }

        fn autoclear(&self, _: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push("autoclear");
            self.autoclear.set(true);
            Ok(())
        }

        fn loop_delete(&self, _: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push("loop-delete");
            fs::remove_dir_all(self.loop_dir()).unwrap();
            Ok(())
        }
    }

    #[test]
    fn a_view_is_a_read_only_loop_device_mounted_noexec_that_goes_with_its_mount() {
        let m = Machine::new();
        let file = m.bundle("App.oma");
        let dir = mount(&file, &m.kernel, &m).unwrap();
        assert_eq!(dir, Path::new("/run/media/me/My App"));
        assert_eq!(m.calls(), ["loop-setup", "mount", "autoclear"]);
        assert!(m.mounted().unwrap().split(',').any(|o| o == "noexec"));
    }

    #[test]
    fn what_the_automounter_mounted_without_noexec_is_mounted_again_with_it() {
        let m = Machine {
            automount: true,
            ..Machine::new()
        };
        let file = m.bundle("App.oma");
        mount(&file, &m.kernel, &m).unwrap();
        assert_eq!(m.calls(), ["loop-setup", "unmount", "mount", "autoclear"]);
        assert!(m.mounted().unwrap().split(',').any(|o| o == "noexec"));
    }

    #[test]
    fn the_view_there_is_is_used_again_and_found_by_inode_not_by_name() {
        let m = Machine::new();
        let file = m.bundle("App.oma");
        let other = m.dir.path().join("Same.oma");
        fs::hard_link(&file, &other).unwrap();
        m.attach(&file);
        m.mount_with("ro,nosuid,nodev,noexec,relatime");
        assert_eq!(
            mount(&other, &m.kernel, &m).unwrap(),
            Path::new("/run/media/me/My App")
        );
        assert_eq!(
            m.calls(),
            ["autoclear"],
            "nothing set up and nothing mounted"
        );

        let copy = m.dir.path().join("Copy.oma");
        fs::copy(&file, &copy).unwrap();
        let copy = fs::metadata(&copy).unwrap();
        assert!(
            m.kernel.loops_of(&copy).is_empty(),
            "a copy is another file"
        );
    }

    #[test]
    fn a_loop_device_that_could_not_be_mounted_is_not_left_behind() {
        let m = Machine {
            refuse_mount: true,
            ..Machine::new()
        };
        let file = m.bundle("App.oma");
        let err = mount(&file, &m.kernel, &m).unwrap_err().to_string();
        assert_eq!(err, "cannot mount");
        assert_eq!(m.calls(), ["loop-setup", "mount", "loop-delete"]);
        assert!(!m.loop_dir().exists());
    }

    #[test]
    fn only_a_bundle_file_is_mounted() {
        let m = Machine::new();
        let plain = m.dir.path().join("plain.oma");
        fs::write(&plain, vec![0u8; 4096]).unwrap();
        let short = m.dir.path().join("short.oma");
        fs::write(&short, b"OMA1").unwrap();
        let mut no_image = vec![0u8; 4096];
        no_image[..4].copy_from_slice(MAGIC);
        let header_only = m.dir.path().join("header.oma");
        fs::write(&header_only, no_image).unwrap();
        for file in [&plain, &short, &header_only] {
            assert!(
                matches!(mount(file, &m.kernel, &m), Err(Error::NotABundle(_))),
                "{}",
                file.display()
            );
        }
        assert!(matches!(
            mount(m.dir.path(), &m.kernel, &m),
            Err(Error::Directory(_))
        ));
        assert!(matches!(
            unmount(&plain, &m.kernel, &m),
            Err(Error::NotABundle(_))
        ));
        assert!(m.calls().is_empty());
    }

    #[test]
    fn unmount_takes_the_view_away_with_its_loop_device() {
        let m = Machine::new();
        let file = m.bundle("App.oma");
        mount(&file, &m.kernel, &m).unwrap();
        unmount(&file, &m.kernel, &m).unwrap();
        // It cleared itself, so there was nothing to delete.
        assert_eq!(m.calls()[3..], ["unmount"]);
        assert!(!m.loop_dir().exists() && m.mounted().is_none());

        // A view another tool made, which does not clear itself.
        let m = Machine::new();
        let file = m.bundle("App.oma");
        m.attach(&file);
        m.mount_with("ro,nosuid,nodev,relatime");
        unmount(&file, &m.kernel, &m).unwrap();
        assert_eq!(m.calls(), ["unmount", "loop-delete"]);
        assert!(!m.loop_dir().exists());
    }

    #[test]
    fn unmount_of_a_file_with_no_view_says_so() {
        let m = Machine::new();
        let file = m.bundle("App.oma");
        let err = unmount(&file, &m.kernel, &m).unwrap_err();
        assert!(matches!(err, Error::NotMounted(_)), "{err}");
        assert!(m.calls().is_empty());
    }

    #[test]
    fn a_mountinfo_path_is_unescaped() {
        assert_eq!(
            unescape(br"/run/media/me/My\040App\134x\011y"),
            Path::new("/run/media/me/My App\\x\ty")
        );
        assert_eq!(
            unescape(br"/a\04"),
            Path::new(r"/a\04"),
            "too short to be an escape"
        );
        assert_eq!(unescape(br"/a\089"), Path::new(r"/a\089"), "not octal");
    }
}
