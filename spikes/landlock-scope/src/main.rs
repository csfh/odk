//! Spike: does Landlock's abstract-unix-socket scoping (ABI 6) stop a process from
//! connecting to an abstract socket created outside its domain? (docs/checks.md)

use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::process::Command;

const SYS_LANDLOCK_CREATE_RULESET: libc::c_long = 444;
const SYS_LANDLOCK_RESTRICT_SELF: libc::c_long = 446;
const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
const LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET: u64 = 1;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

fn abi_version() -> io::Result<i64> {
    let r = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r)
    }
}

fn scope_abstract_sockets() -> io::Result<()> {
    let attr = RulesetAttr {
        handled_access_fs: 0,
        handled_access_net: 0,
        scoped: LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET,
    };
    let fd = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &attr as *const RulesetAttr,
            std::mem::size_of::<RulesetAttr>(),
            0u32,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::syscall(SYS_LANDLOCK_RESTRICT_SELF, fd as i32, 0u32) } != 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe { libc::close(fd as i32) };
    Ok(())
}

fn connect_abstract(name: &str) -> io::Result<UnixStream> {
    let addr = SocketAddr::from_abstract_name(name.as_bytes())?;
    UnixStream::connect_addr(&addr)
}

fn describe(r: &io::Result<UnixStream>) -> String {
    match r {
        Ok(_) => "connected".into(),
        Err(e) => format!("failed: {e} (errno {})", e.raw_os_error().unwrap_or(0)),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 {
        // Child role: `plain <name>`, `scoped <name>` or `scoped-own <name>`.
        let name = &args[2];
        match args[1].as_str() {
            "plain" => println!("  unscoped child -> {}", describe(&connect_abstract(name))),
            "scoped" => {
                scope_abstract_sockets().expect("restrict_self");
                println!("  scoped child   -> {}", describe(&connect_abstract(name)));
            }
            "scoped-own" => {
                scope_abstract_sockets().expect("restrict_self");
                let own = format!("{name}-own");
                let _l = UnixListener::bind_addr(
                    &SocketAddr::from_abstract_name(own.as_bytes()).unwrap(),
                )
                .expect("bind own");
                println!(
                    "  scoped child, its own socket -> {}",
                    describe(&connect_abstract(&own))
                );
            }
            other => panic!("unknown role {other}"),
        }
        return;
    }

    match abi_version() {
        Ok(v) => println!("Landlock ABI: {v} (scoping needs >= 6)"),
        Err(e) => {
            println!("Landlock unavailable: {e}");
            return;
        }
    }

    let name = format!("oma-spike-{}", std::process::id());
    let _listener =
        UnixListener::bind_addr(&SocketAddr::from_abstract_name(name.as_bytes()).unwrap())
            .expect("bind");
    println!("abstract socket @{name} created outside the child domains");
    let me = std::env::current_exe().unwrap();
    for role in ["plain", "scoped", "scoped-own"] {
        let out = Command::new(&me)
            .args([role, &name])
            .output()
            .expect("spawn");
        print!("{}", String::from_utf8_lossy(&out.stdout));
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
    }

    match std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        Ok(a) => println!("DBUS_SESSION_BUS_ADDRESS={a}"),
        Err(_) => println!("DBUS_SESSION_BUS_ADDRESS unset"),
    }
    let tcp = std::fs::read_to_string("/proc/net/unix").unwrap_or_default();
    let abstracts: Vec<&str> = tcp.lines().filter(|l| l.contains(" @")).collect();
    println!(
        "{} abstract unix sockets on the host; examples:",
        abstracts.len()
    );
    for l in abstracts.iter().take(8) {
        println!("  {}", l.split_whitespace().last().unwrap_or(""));
    }
}
