//! The object a Bundle file is checked in (ADR 0060, section 5). The path is opened once, without
//! following a symlink, and must be a regular file. Its bytes are copied into a new object before
//! anything trusts them:
//! - a memfd sealed against every change;
//! - or a read-only file in a mode-`0700` directory that only this user can write.
//!
//! Every later check, and the launch, read that object, never the path. No extended attribute and
//! no stamp file records a result.

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fs::{FileType, MemfdFlags, Mode, OFlags, SealFlags};

use super::Error;

/// Every seal section 5 names.
const SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

/// A copy of a Bundle file that cannot change while it is checked and run.
#[derive(Debug)]
pub struct Sealed {
    file: File,
    len: u64,
    /// The copy's name in the store; `None` for a memfd.
    stored: Option<PathBuf>,
}

impl Sealed {
    /// A sealed memfd holding the file at `path`.
    pub fn memfd(path: &Path) -> Result<Self, Error> {
        let mut source = open_source(path)?;
        let copy = File::from(rustix::fs::memfd_create(
            "odk-oma-bundle",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?);
        let len = io::copy(&mut source, &mut &copy)?;
        rustix::fs::fcntl_add_seals(&copy, SEALS)?;
        let seals = rustix::fs::fcntl_get_seals(&copy)?;
        if !seals.contains(SEALS) {
            return Err(Error::Seals {
                seals: seals.bits(),
            });
        }
        Ok(Self {
            file: copy,
            len,
            stored: None,
        })
    }

    /// A read-only copy of the file at `path` in `store`, a directory that is mode `0700` and
    /// belongs to this user. The copy gets a new name there and is never opened for writing
    /// again.
    pub fn in_store(path: &Path, store: &Path) -> Result<Self, Error> {
        let mut source = open_source(path)?;
        let dir = rustix::fs::open(
            store,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| Error::Store {
            reason: format!("{} cannot be opened: {e}", store.display()),
        })?;
        let stat = rustix::fs::fstat(&dir)?;
        if stat.st_uid != rustix::process::geteuid().as_raw() {
            return Err(Error::Store {
                reason: format!("{} does not belong to this user", store.display()),
            });
        }
        if stat.st_mode & 0o7777 != 0o700 {
            return Err(Error::Store {
                reason: format!(
                    "{} is mode {:o}, not 0700",
                    store.display(),
                    stat.st_mode & 0o7777
                ),
            });
        }
        let name = unique_name();
        let writer = File::from(rustix::fs::openat(
            &dir,
            &name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR,
        )?);
        let len = io::copy(&mut source, &mut &writer)?;
        writer.sync_all()?;
        drop(writer);
        let file = File::from(rustix::fs::openat(
            &dir,
            &name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Ok(Self {
            file,
            len,
            stored: Some(store.join(name)),
        })
    }

    /// The copy's length: what was read from the path, once.
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Fills `buf` from `offset`, or fails if the copy ends first.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.file.read_exact_at(buf, offset)
    }

    /// The copy's path in its store; `None` for a memfd, which has none.
    pub fn stored_path(&self) -> Option<&Path> {
        self.stored.as_deref()
    }
}

impl AsFd for Sealed {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.file.as_fd()
    }
}

/// The path, opened once: `O_NOFOLLOW` so a symlink is refused, `O_NONBLOCK` so a fifo cannot
/// make the open wait for a writer, and then it must be a regular file.
fn open_source(path: &Path) -> Result<File, Error> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd: OwnedFd = match rustix::fs::open(path, flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::LOOP) => return Err(Error::Symlink),
        // A socket cannot be opened at all.
        Err(rustix::io::Errno::NXIO) => return Err(Error::NotRegular { kind: "socket" }),
        Err(e) => return Err(e.into()),
    };
    let kind = FileType::from_raw_mode(rustix::fs::fstat(&fd)?.st_mode);
    if kind != FileType::RegularFile {
        return Err(Error::NotRegular {
            kind: match kind {
                FileType::Directory => "directory",
                FileType::Fifo => "fifo",
                FileType::Socket => "socket",
                FileType::CharacterDevice | FileType::BlockDevice => "device",
                FileType::Symlink => "symbolic link",
                _ => "file of another kind",
            },
        });
    }
    let blocking = rustix::fs::fcntl_getfl(&fd)? - OFlags::NONBLOCK;
    rustix::fs::fcntl_setfl(&fd, blocking)?;
    Ok(File::from(fd))
}

/// A name no other copy in the store has: this process, a counter and the time.
fn unique_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!(
        "sealed-{}-{}-{nanos}.oma",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    fn bytes_of(sealed: &Sealed) -> Vec<u8> {
        let mut buf = vec![0; sealed.len() as usize];
        sealed.read_exact_at(&mut buf, 0).unwrap();
        buf
    }

    fn store(dir: &Path) -> PathBuf {
        let store = dir.join("store");
        std::fs::create_dir(&store).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
        store
    }

    #[test]
    fn a_memfd_copy_carries_all_four_seals_and_keeps_its_bytes_when_the_source_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("App.oma");
        std::fs::write(&path, b"OMA1 first").unwrap();
        let sealed = Sealed::memfd(&path).unwrap();
        let seals = rustix::fs::fcntl_get_seals(&sealed).unwrap();
        assert!(seals.contains(SEALS), "{seals:?}");

        std::fs::write(&path, b"OMA1 second, and longer").unwrap();
        assert_eq!(bytes_of(&sealed), b"OMA1 first");
        assert_eq!(sealed.len(), 10);
        assert!(sealed.stored_path().is_none());

        // The seals hold against this process too.
        let writable = std::fs::OpenOptions::new().write(true).open(format!(
            "/proc/self/fd/{}",
            std::os::fd::AsRawFd::as_raw_fd(&sealed.file)
        ));
        if let Ok(f) = writable {
            assert!(f.write_at(b"X", 0).is_err());
            assert!(f.set_len(1).is_err());
        }
    }

    #[test]
    fn a_copy_in_a_0700_store_is_read_only_and_keeps_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("App.oma");
        std::fs::write(&path, b"OMA1 first").unwrap();
        let store = store(dir.path());
        let sealed = Sealed::in_store(&path, &store).unwrap();
        std::fs::write(&path, b"OMA1 second").unwrap();
        assert_eq!(bytes_of(&sealed), b"OMA1 first");
        let stored = sealed.stored_path().unwrap();
        assert!(stored.starts_with(&store));
        let mode = std::fs::metadata(stored).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o400);
    }

    #[test]
    fn a_store_that_is_not_mode_0700_or_is_a_symlink_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("App.oma");
        std::fs::write(&path, b"OMA1").unwrap();
        let store = store(dir.path());
        for mode in [0o755, 0o750, 0o1700] {
            std::fs::set_permissions(&store, std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                matches!(Sealed::in_store(&path, &store), Err(Error::Store { .. })),
                "{mode:o}"
            );
        }
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = dir.path().join("link");
        symlink(&store, &link).unwrap();
        assert!(matches!(
            Sealed::in_store(&path, &link),
            Err(Error::Store { .. })
        ));
    }

    #[test]
    fn a_symlink_a_directory_a_fifo_and_a_socket_are_not_bundle_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("App.oma");
        std::fs::write(&target, b"OMA1").unwrap();
        let link = dir.path().join("link.oma");
        symlink(&target, &link).unwrap();
        let fifo = dir.path().join("fifo.oma");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &fifo,
            FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            0,
        )
        .unwrap();
        let socket = dir.path().join("socket.oma");
        let _listener = UnixListener::bind(&socket).unwrap();
        let directory = dir.path().join("Dir.oma");
        std::fs::create_dir(&directory).unwrap();

        assert!(matches!(Sealed::memfd(&link), Err(Error::Symlink)));
        assert!(matches!(
            Sealed::memfd(&directory),
            Err(Error::NotRegular { kind: "directory" })
        ));
        assert!(matches!(
            Sealed::memfd(&fifo),
            Err(Error::NotRegular { kind: "fifo" })
        ));
        assert!(matches!(
            Sealed::memfd(&socket),
            Err(Error::NotRegular { kind: "socket" })
        ));
        let store = store(dir.path());
        assert!(matches!(
            Sealed::in_store(&link, &store),
            Err(Error::Symlink)
        ));
        assert!(matches!(
            Sealed::in_store(&fifo, &store),
            Err(Error::NotRegular { kind: "fifo" })
        ));
    }
}
