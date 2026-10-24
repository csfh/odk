//! End-to-end tests of `oma omarchy` (ADR 0044): the copy of the user's text size and font, and
//! the `systemd --user` units the package ships to keep it current. Everything is under a
//! temporary home; no test touches the real user manager.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

use common::*;

fn oma_omarchy(w: &World, args: &[&str]) -> Output {
    output_retrying(
        Command::new(&w.oma_bin)
            .arg("omarchy")
            .args(args)
            .env_clear()
            .env("HOME", &w.home)
            .env("PATH", "/usr/bin"),
    )
}

fn mirror(w: &World) -> PathBuf {
    w.home.join(".local/share/oma/omarchy")
}

#[test]
fn sync_copies_the_text_size_and_font_and_follows_the_user() {
    let w = World::new();
    let (omarchy, fontconfig) = (
        w.home.join(".config/omarchy"),
        w.home.join(".config/fontconfig"),
    );
    fs::create_dir_all(&omarchy).unwrap();
    fs::create_dir_all(&fontconfig).unwrap();
    fs::write(
        omarchy.join("shell.toml"),
        "[bar]\ntext = \"#fff\"\n[font]\nbase-size = 15\n",
    )
    .unwrap();
    fs::write(fontconfig.join("fonts.conf"), "<fontconfig/>").unwrap();

    let out = oma_omarchy(&w, &["sync"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(mirror(&w).join("omarchy/shell.toml")).unwrap(),
        "[font]\nbase-size = 15\n"
    );
    assert_eq!(
        fs::read_to_string(mirror(&w).join("fontconfig/fonts.conf")).unwrap(),
        "<fontconfig/>"
    );

    fs::remove_file(omarchy.join("shell.toml")).unwrap();
    assert!(oma_omarchy(&w, &["sync"]).status.success());
    assert!(!mirror(&w).join("omarchy/shell.toml").exists());
}

/// ADR 0048: `sync` asks the compositor whether it animates windows and tells the sandbox what
/// it said, as the user's request for reduced motion. The compositor here is a socket of the
/// test's own, in the test's runtime directory; the machine's is never asked.
#[test]
fn sync_copies_the_compositors_request_for_reduced_motion() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let w = World::new();
    let instance = w.runtime.join("hypr/test");
    fs::create_dir_all(&instance).unwrap();
    let listener = UnixListener::bind(instance.join(".socket.sock")).unwrap();
    let animated = Arc::new(AtomicBool::new(false));
    let now = animated.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut question = [0u8; 128];
            let n = stream.read(&mut question).unwrap_or(0);
            assert_eq!(&question[..n], b"j/getoption animations:enabled");
            let reply = format!(
                "{{\"option\": \"animations:enabled\", \"bool\": {}, \"set\": true }}",
                now.load(Ordering::SeqCst)
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    let sync = || {
        output_retrying(
            Command::new(&w.oma_bin)
                .args(["omarchy", "sync"])
                .env_clear()
                .env("HOME", &w.home)
                .env("PATH", "/usr/bin")
                .env("XDG_RUNTIME_DIR", &w.runtime),
        )
    };
    let shell = || fs::read_to_string(mirror(&w).join("omarchy/shell.toml")).ok();

    // Animations off, and the user has no `shell.toml` at all: the application is told.
    assert!(sync().status.success());
    assert_eq!(shell().as_deref(), Some("[motion]\nreduced = true\n"));

    // Animations on: nothing is asked for, whatever the user's own file says.
    animated.store(true, Ordering::SeqCst);
    let omarchy = w.home.join(".config/omarchy");
    fs::create_dir_all(&omarchy).unwrap();
    fs::write(omarchy.join("shell.toml"), "[motion]\nreduced = true\n").unwrap();
    assert!(sync().status.success());
    assert_eq!(shell(), None);
}

/// What the package installs (`packaging/`, ADR 0044): a path unit on the two files Omarchy
/// writes, which starts a one-shot service that runs `oma omarchy sync`, and both wanted by
/// `default.target`, for the copy at login.
#[test]
fn the_shipped_units_watch_both_files_and_run_sync() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/systemd/user");
    let lines = |name: &str| -> Vec<String> {
        fs::read_to_string(dir.join(name))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    };
    let (path, service) = (lines("oma-omarchy.path"), lines("oma-omarchy.service"));
    for line in [
        "PathChanged=%h/.config/omarchy/shell.toml",
        "PathChanged=%h/.config/fontconfig/fonts.conf",
        "Unit=oma-omarchy.service",
        "WantedBy=default.target",
    ] {
        assert!(path.iter().any(|l| l == line), "{line}");
    }
    for line in [
        "Type=oneshot",
        "ExecStart=/usr/bin/oma omarchy sync",
        "WantedBy=default.target",
    ] {
        assert!(service.iter().any(|l| l == line), "{line}");
    }
    // The files are the ones `Layout` copies from, which Omarchy keeps under `$HOME/.config`.
    let layout = oma::layout::Layout::new("/home/u", "/home/u/.local/share");
    assert_eq!(
        layout.user_omarchy_shell(),
        Path::new("/home/u/.config/omarchy/shell.toml")
    );
    assert_eq!(
        layout.user_fonts_conf(),
        Path::new("/home/u/.config/fontconfig/fonts.conf")
    );
}
