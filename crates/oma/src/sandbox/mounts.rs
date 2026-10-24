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
