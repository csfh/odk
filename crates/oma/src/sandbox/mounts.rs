//! Carrying out the mount part of a [`Plan`](super::plan::Plan): a new root made of a tmpfs
//! and bind mounts, entered with `pivot_root`. This runs in the child that is about to become
//! the init, inside its new user, mount and PID namespaces and with its full set of
//! capabilities there.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use super::plan::Op;
use crate::image::Image;

#[derive(Debug, thiserror::Error)]
#[error("{what}: {source}")]
pub struct Error {
    what: String,
    source: io::Error,
}

fn err(what: impl Into<String>, source: io::Error) -> Error {
    Error {
        what: what.into(),
        source,
    }
}

const MOUNT_ATTR_RDONLY: u64 = 0x1;
const MOUNT_ATTR_NOSUID: u64 = 0x2;
const MOUNT_ATTR_NODEV: u64 = 0x4;
const AT_RECURSIVE: u32 = 0x8000;

#[repr(C)]
struct MountAttr {
    attr_set: u64,
    attr_clr: u64,
    propagation: u64,
    userns_fd: u64,
}

fn c(path: &Path) -> CString {
    CString::new(path.as_os_str().as_bytes()).expect("paths have no NUL bytes")
}

fn mount(
    source: Option<&str>,
    target: &Path,
    fstype: Option<&str>,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> io::Result<()> {
    let source = source.map(|s| CString::new(s).unwrap());
    let fstype = fstype.map(|s| CString::new(s).unwrap());
    let data = data.map(|s| CString::new(s).unwrap());
    let target = c(target);
    let r = unsafe {
        libc::mount(
            source.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            target.as_ptr(),
            fstype.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr().cast()),
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Adds attributes to a mount, and to everything mounted beneath it when `recursive`.
fn set_attr(path: &Path, set: u64, recursive: bool) -> io::Result<()> {
    let attr = MountAttr {
        attr_set: set,
        attr_clr: 0,
        propagation: 0,
        userns_fd: 0,
    };
    let path = c(path);
    let flags = if recursive { AT_RECURSIVE } else { 0 };
    let r = unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            libc::AT_FDCWD,
            path.as_ptr(),
            flags,
            &attr as *const MountAttr,
            std::mem::size_of::<MountAttr>(),
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn inside(root: &Path, dst: &Path) -> PathBuf {
    root.join(dst.strip_prefix("/").unwrap_or(dst))
}

fn make_dir(path: &Path, mode: u32) -> Result<(), Error> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(mode)
        .create(path)
        .map_err(|e| err(format!("creating {}", path.display()), e))
}

/// Builds the new root under `root` (an existing empty directory on the host) from `ops`, then
/// makes it `/`. The old root is detached and unreachable afterwards.
pub fn enter(root: &Path, ops: &[Op]) -> Result<(), Error> {
    mount(
        None,
        Path::new("/"),
        None,
        libc::MS_REC | libc::MS_PRIVATE,
        None,
    )
    .map_err(|e| err("making mounts private", e))?;
    mount(
        Some("tmpfs"),
        root,
        Some("tmpfs"),
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("mode=0755"),
    )
    .map_err(|e| err("mounting the new root", e))?;

    for op in ops {
        apply(root, op)?;
    }

    let root_c = c(root);
    if unsafe { libc::chdir(root_c.as_ptr()) } != 0 {
        return Err(err("entering the new root", io::Error::last_os_error()));
    }
    let dot = c(Path::new("."));
    if unsafe { libc::syscall(libc::SYS_pivot_root, dot.as_ptr(), dot.as_ptr()) } != 0 {
        return Err(err("pivot_root", io::Error::last_os_error()));
    }
    if unsafe { libc::umount2(dot.as_ptr(), libc::MNT_DETACH) } != 0 {
        return Err(err("detaching the old root", io::Error::last_os_error()));
    }
    let slash = c(Path::new("/"));
    if unsafe { libc::chdir(slash.as_ptr()) } != 0 {
        return Err(err("entering /", io::Error::last_os_error()));
    }
    // Everything above `/` is read-only: only the mounts the plan made writable can be written.
    set_attr(Path::new("/"), MOUNT_ATTR_RDONLY, false).map_err(|e| err("making / read-only", e))?;
    Ok(())
}

/// A Bundle file's tree at `target` (ADR 0067): a tmpfs sized to the image's files, the verified
/// tree written into it, then read-only, `nosuid` and `nodev`, with execution still allowed. It
/// runs where the plan's other mounts are made, in the child that is about to become the init,
/// inside its namespaces, so the tmpfs is that mount namespace's and goes with it. The init does
/// nothing for it (ADR 0024).
pub fn unpack_image(target: &Path, image: &Image<'_>) -> Result<(), Error> {
    make_dir(target, 0o755)?;
    let options = format!(
        "mode=0755,size={},nr_inodes={}",
        image.unpacked_size(),
        image.nodes().len() + 1
    );
    mount(
        Some("tmpfs"),
        target,
        Some("tmpfs"),
        libc::MS_NOSUID | libc::MS_NODEV,
        Some(&options),
    )
    .map_err(|e| err(format!("mounting a tmpfs at {}", target.display()), e))?;
    image
        .unpack(target)
        .map_err(|e| err(format!("unpacking the Bundle at {}", target.display()), e))?;
    set_attr(
        target,
        MOUNT_ATTR_RDONLY | MOUNT_ATTR_NOSUID | MOUNT_ATTR_NODEV,
        false,
    )
    .map_err(|e| err(format!("making {} read-only", target.display()), e))
}

fn apply(root: &Path, op: &Op) -> Result<(), Error> {
    match op {
        Op::Dir(dst) => {
            let mode = if dst.starts_with("/run/user") {
                0o700
            } else {
                0o755
            };
            make_dir(&inside(root, dst), mode)
        }
        Op::Tmpfs(dst) => {
            let target = inside(root, dst);
            make_dir(&target, 0o755)?;
            let mode = if dst == Path::new("/tmp") || dst == Path::new("/dev/shm") {
                "mode=1777"
            } else {
                "mode=0755"
            };
            mount(
                Some("tmpfs"),
                &target,
                Some("tmpfs"),
                libc::MS_NOSUID | libc::MS_NODEV,
                Some(mode),
            )
            .map_err(|e| err(format!("mounting a tmpfs at {}", dst.display()), e))
        }
        Op::Proc(dst) => {
            let target = inside(root, dst);
            make_dir(&target, 0o755)?;
            mount(
                Some("proc"),
                &target,
                Some("proc"),
                libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                None,
            )
            .map_err(|e| err("mounting /proc", e))
        }
        Op::Symlink { dst, target } => {
            let link = inside(root, dst);
            if let Some(parent) = link.parent() {
                make_dir(parent, 0o755)?;
            }
            symlink(target, &link).map_err(|e| err(format!("linking {}", dst.display()), e))
        }
        Op::Bind {
            src,
            dst,
            readonly,
            device,
        } => {
            let target = inside(root, dst);
            let meta =
                fs::metadata(src).map_err(|e| err(format!("looking at {}", src.display()), e))?;
            if meta.is_dir() {
                make_dir(&target, 0o755)?;
            } else {
                if let Some(parent) = target.parent() {
                    make_dir(parent, 0o755)?;
                }
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&target)
                    .map_err(|e| err(format!("creating the mount point {}", dst.display()), e))?;
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o644));
            }
            let src_str = src.to_str().expect("bind sources are UTF-8 paths");
            mount(
                Some(src_str),
                &target,
                None,
                libc::MS_BIND | libc::MS_REC,
                None,
            )
            .map_err(|e| err(format!("binding {} at {}", src.display(), dst.display()), e))?;
            let mut set = MOUNT_ATTR_NOSUID;
            if !device {
                set |= MOUNT_ATTR_NODEV;
            }
            if *readonly {
                set |= MOUNT_ATTR_RDONLY;
            }
            set_attr(&target, set, true)
                .map_err(|e| err(format!("restricting {}", dst.display()), e))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::image::fixture::{Item, build, notes};
    use crate::trust::{self, Sealed};

    const CHILD: &str = "ODK_UNPACK_IMAGE_CHILD";

    /// Every file and directory under `dir`: its path, its permission bits and its bytes' digest.
    fn listing(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let rel = path.strip_prefix(base).unwrap().display().to_string();
            let mode = meta.permissions().mode() & 0o7777;
            if meta.is_dir() {
                out.push(format!("{rel} {mode:o} dir"));
                listing(&path, base, out);
            } else {
                let digest = Sha256::digest(fs::read(&path).unwrap());
                out.push(format!("{rel} {mode:o} {}", crate::trust::hex(&digest)));
            }
        }
    }

    /// The other half of the next test: it runs only inside the namespaces that test makes, where
    /// it is root, and prints what it finds.
    #[test]
    fn unpack_image_inside_a_namespace() {
        let Ok(dir) = std::env::var(CHILD) else {
            return;
        };
        let dir = Path::new(&dir);
        // A host can allow the namespaces and still refuse a mount in them.
        let probe = dir.join("probe");
        make_dir(&probe, 0o755).unwrap();
        if let Err(e) = mount(Some("tmpfs"), &probe, Some("tmpfs"), 0, Some("size=4096")) {
            println!("unsupported: {e}");
            return;
        }
        let path = dir.join("Notes.oma");
        fs::write(&path, build(&notes()).signed()).unwrap();
        let sealed = Sealed::memfd(&path).unwrap();
        let checked = trust::check(&sealed).unwrap();
        let image = Image::read(&checked).unwrap();
        let app = dir.join("app");
        unpack_image(&app, &image).unwrap();

        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::statvfs(c(&app).as_ptr(), &mut st) }, 0);
        println!("flags={}", st.f_flag);
        println!("size={}", st.f_blocks * st.f_frsize);
        let write = fs::write(app.join("new"), b"x").err();
        println!("write={:?}", write.and_then(|e| e.raw_os_error()));
        let mut files = Vec::new();
        listing(&app, &app, &mut files);
        files.sort();
        for line in files {
            println!("entry {line}");
        }
    }

    #[test]
    fn a_launchs_app_is_the_images_tree_read_only_nosuid_and_nodev() {
        let namespaces = ["--user", "--map-root-user", "--mount", "--"];
        let userns = Command::new("unshare")
            .args(namespaces)
            .arg("true")
            .status()
            .is_ok_and(|s| s.success());
        if !userns {
            eprintln!("skipped: needs unprivileged user and mount namespaces");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let out = Command::new("unshare")
            .args(namespaces)
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "sandbox::mounts::tests::unpack_image_inside_a_namespace",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, dir.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        if let Some(why) = stdout.lines().find_map(|l| l.split_once("unsupported: ")) {
            eprintln!(
                "skipped: a tmpfs cannot be mounted in the namespace ({})",
                why.1
            );
            return;
        }
        assert!(
            out.status.success(),
            "{stdout}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let value = |key: &str| {
            stdout
                .lines()
                .find_map(|l| l.split_once(key).map(|(_, v)| v))
                .unwrap_or_else(|| panic!("no {key} in {stdout}"))
        };
        let flags: u64 = value("flags=").parse().unwrap();
        let wanted = libc::ST_RDONLY | libc::ST_NOSUID | libc::ST_NODEV;
        assert_eq!(flags & wanted, wanted, "read-only, nosuid and nodev");
        assert_eq!(flags & libc::ST_NOEXEC, 0, "execution stays allowed");
        assert_eq!(value("write="), format!("Some({})", libc::EROFS));
        // Sized to the files: a page each for three, two for `share/big`, none for `share/empty`.
        assert_eq!(value("size="), (5 * 4096).to_string());

        let mut wanted = vec!["bin 755 dir".to_owned(), "share 755 dir".to_owned()];
        for (path, item) in notes() {
            if let Item::File { bytes, mode, .. } = item {
                let digest = crate::trust::hex(&Sha256::digest(&bytes));
                wanted.push(format!("{path} {mode:o} {digest}"));
            }
        }
        wanted.sort();
        let found: Vec<&str> = stdout
            .lines()
            .filter_map(|l| l.strip_prefix("entry "))
            .collect();
        assert_eq!(found, wanted);
        // The tmpfs went with the namespace: nothing reached the host's directory.
        assert_eq!(fs::read_dir(dir.path().join("app")).unwrap().count(), 0);
    }
}
