//! Landlock (ADR 0005): a second layer under the mount namespace. The ABI version is read at
//! run time and nothing newer is assumed. Abstract-unix-socket scoping needs ABI 6 and is
//! required for applications that share the host's network namespace.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const SYS_CREATE_RULESET: libc::c_long = 444;
const SYS_ADD_RULE: libc::c_long = 445;
const SYS_RESTRICT_SELF: libc::c_long = 446;
const CREATE_RULESET_VERSION: u32 = 1;
const RULE_PATH_BENEATH: u32 = 1;

const EXECUTE: u64 = 1 << 0;
const WRITE_FILE: u64 = 1 << 1;
const READ_FILE: u64 = 1 << 2;
const READ_DIR: u64 = 1 << 3;
const REMOVE_DIR: u64 = 1 << 4;
const REMOVE_FILE: u64 = 1 << 5;
const MAKE_CHAR: u64 = 1 << 6;
const MAKE_DIR: u64 = 1 << 7;
const MAKE_REG: u64 = 1 << 8;
const MAKE_SOCK: u64 = 1 << 9;
const MAKE_FIFO: u64 = 1 << 10;
const MAKE_BLOCK: u64 = 1 << 11;
const MAKE_SYM: u64 = 1 << 12;
const REFER: u64 = 1 << 13;
const TRUNCATE: u64 = 1 << 14;
const IOCTL_DEV: u64 = 1 << 15;

const SCOPE_ABSTRACT_UNIX_SOCKET: u64 = 1 << 0;

/// Rights that make sense on a file rather than a directory.
const FILE_RIGHTS: u64 = EXECUTE | WRITE_FILE | READ_FILE | TRUNCATE | IOCTL_DEV;

/// What a rule lets the application do beneath its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rights {
    /// Read files and list directories.
    Read,
    /// Read, and run programs.
    ReadExec,
    /// Everything the kernel's Landlock supports: read, write, create, remove, rename, run
    /// and device ioctls.
    All,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub rules: Vec<(PathBuf, Rights)>,
    /// Stop the application from connecting to abstract unix sockets made outside it.
    pub scope_abstract_unix_sockets: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Landlock is not available on this kernel: {0}")]
    Unavailable(io::Error),
    #[error("abstract-unix-socket scoping needs Landlock ABI 6, and this kernel has ABI {0}")]
    ScopingNeedsNewerAbi(u32),
    #[error("cannot add a Landlock rule for {}: {source}", path.display())]
    Rule { path: PathBuf, source: io::Error },
    #[error("cannot enforce Landlock: {0}")]
    Enforce(io::Error),
}

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: i32,
}

/// The Landlock ABI version of the running kernel.
pub fn abi_version() -> Result<u32, Error> {
    let r = unsafe {
        libc::syscall(
            SYS_CREATE_RULESET,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            CREATE_RULESET_VERSION,
        )
    };
    if r < 0 {
        Err(Error::Unavailable(io::Error::last_os_error()))
    } else {
        Ok(r as u32)
    }
}

/// Every filesystem right this ABI knows. Rights the kernel does not know cannot be handled.
fn handled_fs(abi: u32) -> u64 {
    let mut handled = EXECUTE
        | WRITE_FILE
        | READ_FILE
        | READ_DIR
        | REMOVE_DIR
        | REMOVE_FILE
        | MAKE_CHAR
        | MAKE_DIR
        | MAKE_REG
        | MAKE_SOCK
        | MAKE_FIFO
        | MAKE_BLOCK
        | MAKE_SYM;
    if abi >= 2 {
        handled |= REFER;
    }
    if abi >= 3 {
        handled |= TRUNCATE;
    }
    if abi >= 5 {
        handled |= IOCTL_DEV;
    }
    handled
}

fn rights_bits(rights: Rights, handled: u64) -> u64 {
    (match rights {
        Rights::Read => READ_FILE | READ_DIR,
        Rights::ReadExec => EXECUTE | READ_FILE | READ_DIR,
        Rights::All => handled,
    }) & handled
}

fn ruleset_attr_size(abi: u32) -> usize {
    match abi {
        0..=3 => 8,
        4..=5 => 16,
        _ => 24,
    }
}

/// Restricts the calling thread, and everything it starts afterwards, to what `config` allows.
/// Sets `no_new_privs` first. Anything the rules do not mention under a handled right is denied.
pub fn restrict(config: &Config) -> Result<(), Error> {
    let abi = abi_version()?;
    if config.scope_abstract_unix_sockets && abi < 6 {
        return Err(Error::ScopingNeedsNewerAbi(abi));
    }
    let handled = handled_fs(abi);
    let attr = RulesetAttr {
        handled_access_fs: handled,
        handled_access_net: 0,
        scoped: if config.scope_abstract_unix_sockets {
            SCOPE_ABSTRACT_UNIX_SOCKET
        } else {
            0
        },
    };
    let ruleset = unsafe {
        libc::syscall(
            SYS_CREATE_RULESET,
            &attr as *const RulesetAttr,
            ruleset_attr_size(abi),
            0u32,
        )
    };
    if ruleset < 0 {
        return Err(Error::Enforce(io::Error::last_os_error()));
    }
    let ruleset = ruleset as i32;
    let result = add_rules_and_enforce(ruleset, config, handled);
    unsafe { libc::close(ruleset) };
    result
}

fn add_rules_and_enforce(ruleset: i32, config: &Config, handled: u64) -> Result<(), Error> {
    for (path, rights) in &config.rules {
        let rule_error = |source| Error::Rule {
            path: path.clone(),
            source,
        };
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| rule_error(io::Error::other(e)))?;
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(rule_error(io::Error::last_os_error()));
        }
        let mut allowed = rights_bits(*rights, handled);
        if !is_dir(path) {
            allowed &= FILE_RIGHTS;
        }
        let beneath = PathBeneath {
            allowed_access: allowed,
            parent_fd: fd,
        };
        let r = unsafe {
            libc::syscall(
                SYS_ADD_RULE,
                ruleset,
                RULE_PATH_BENEATH,
                &beneath as *const PathBeneath,
                0u32,
            )
        };
        let error = (r != 0).then(io::Error::last_os_error);
        unsafe { libc::close(fd) };
        if let Some(e) = error {
            return Err(rule_error(e));
        }
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(Error::Enforce(io::Error::last_os_error()));
    }
    if unsafe { libc::syscall(SYS_RESTRICT_SELF, ruleset, 0u32) } != 0 {
        return Err(Error::Enforce(io::Error::last_os_error()));
    }
    Ok(())
}

fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};

    use tempfile::TempDir;

    use super::*;
    use crate::testutil::{in_child, is_child};

    #[test]
    fn handled_rights_grow_with_the_abi() {
        assert_eq!(handled_fs(1).count_ones(), 13);
        assert!(handled_fs(2) & REFER != 0 && handled_fs(1) & REFER == 0);
        assert!(handled_fs(3) & TRUNCATE != 0 && handled_fs(2) & TRUNCATE == 0);
        assert!(handled_fs(5) & IOCTL_DEV != 0 && handled_fs(4) & IOCTL_DEV == 0);
        assert_eq!(ruleset_attr_size(3), 8);
        assert_eq!(ruleset_attr_size(5), 16);
        assert_eq!(ruleset_attr_size(6), 24);
        assert_eq!(
            rights_bits(Rights::Read, handled_fs(1)),
            READ_FILE | READ_DIR
        );
        assert_eq!(rights_bits(Rights::All, handled_fs(1)), handled_fs(1));
    }

    #[test]
    fn the_kernel_here_has_landlock() {
        assert!(abi_version().unwrap() >= 1);
    }

    #[test]
    fn rules_confine_the_process_to_its_paths() {
        in_child("sandbox::landlock::tests::rules_confine_the_process_to_its_paths");
        if !is_child() {
            return;
        }
        let root = TempDir::new().unwrap();
        let (rw, ro, other) = (
            root.path().join("rw"),
            root.path().join("ro"),
            root.path().join("other"),
        );
        for d in [&rw, &ro, &other] {
            fs::create_dir(d).unwrap();
        }
        fs::write(ro.join("file"), "x").unwrap();
        fs::write(other.join("file"), "secret").unwrap();
        // Directories the test needs after restricting: the libraries and the harness.
        let system = ["/usr", "/lib", "/lib64", "/proc"]
            .map(|p| (PathBuf::from(p), Rights::ReadExec))
            .to_vec();
        let mut rules = system;
        rules.push((rw.clone(), Rights::All));
        rules.push((ro.clone(), Rights::Read));
        restrict(&Config {
            rules,
            scope_abstract_unix_sockets: false,
        })
        .unwrap();

        // Writable: create, write, truncate, rename within, remove.
        fs::write(rw.join("a"), "1").unwrap();
        fs::write(rw.join("a"), "").unwrap();
        fs::create_dir(rw.join("d")).unwrap();
        fs::rename(rw.join("a"), rw.join("d/b")).unwrap();
        fs::remove_file(rw.join("d/b")).unwrap();
        // Read-only: readable, not writable, nothing can be added.
        assert_eq!(fs::read_to_string(ro.join("file")).unwrap(), "x");
        assert!(fs::write(ro.join("file"), "y").is_err());
        assert!(fs::write(ro.join("new"), "y").is_err());
        assert!(fs::remove_file(ro.join("file")).is_err());
        // Unlisted: not even readable.
        assert!(fs::read_to_string(other.join("file")).is_err());
        assert!(fs::read_dir(&other).is_err());
        assert!(fs::read_to_string("/etc/passwd").is_err());
        // Programs under /usr still run.
        assert!(
            std::process::Command::new("/usr/bin/true")
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn a_rule_on_a_single_file_works() {
        in_child("sandbox::landlock::tests::a_rule_on_a_single_file_works");
        if !is_child() {
            return;
        }
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("one"), "1").unwrap();
        fs::write(root.path().join("two"), "2").unwrap();
        let rules = vec![(root.path().join("one"), Rights::All)];
        restrict(&Config {
            rules,
            scope_abstract_unix_sockets: false,
        })
        .unwrap();
        fs::write(root.path().join("one"), "changed").unwrap();
        assert!(fs::read_to_string(root.path().join("two")).is_err());
    }

    #[test]
    fn a_missing_path_is_an_error_naming_it() {
        in_child("sandbox::landlock::tests::a_missing_path_is_an_error_naming_it");
        if !is_child() {
            return;
        }
        let rules = vec![(PathBuf::from("/does/not/exist"), Rights::Read)];
        let err = restrict(&Config {
            rules,
            scope_abstract_unix_sockets: false,
        })
        .unwrap_err();
        assert!(
            matches!(&err, Error::Rule { path, .. } if path == Path::new("/does/not/exist")),
            "{err}"
        );
    }

    #[test]
    fn scoping_blocks_abstract_sockets_made_outside_but_not_inside() {
        if abi_version().is_ok_and(|abi| abi < 6) {
            return;
        }
        in_child(
            "sandbox::landlock::tests::scoping_blocks_abstract_sockets_made_outside_but_not_inside",
        );
        if !is_child() {
            return;
        }
        let outside = format!("oma-test-{}", std::process::id());
        let _listener =
            UnixListener::bind_addr(&SocketAddr::from_abstract_name(outside.as_bytes()).unwrap())
                .unwrap();
        let connect = |name: &str| {
            UnixStream::connect_addr(&SocketAddr::from_abstract_name(name.as_bytes()).unwrap())
        };
        assert!(connect(&outside).is_ok(), "reachable before scoping");

        restrict(&Config {
            rules: vec![(PathBuf::from("/"), Rights::All)],
            scope_abstract_unix_sockets: true,
        })
        .unwrap();

        let err = connect(&outside).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::EPERM));
        let inside = format!("{outside}-inside");
        let _own =
            UnixListener::bind_addr(&SocketAddr::from_abstract_name(inside.as_bytes()).unwrap())
                .unwrap();
        assert!(connect(&inside).is_ok(), "its own sockets still work");
    }
}
