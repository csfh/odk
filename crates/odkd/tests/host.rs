//! The real `odkd`, on a throwaway home and runtime directory, asked through the same client an
//! application uses.

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use odk_theme::{Host, HostState};

const COLORS: &str = "mode = \"dark\"\nbackground = \"#0D0C0B\"\nforeground = \"#E8E0D4\"\n";

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(home: &Path, socket: &Path) -> Daemon {
    let state = home.join(".local/state");
    let runtime = home.join("run");
    fs::create_dir_all(&runtime).unwrap();
    Daemon(
        Command::new(env!("CARGO_BIN_EXE_odkd"))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home)
            .env("XDG_STATE_HOME", &state)
            .env("XDG_RUNTIME_DIR", &runtime)
            .env("ODKD_SOCKET", socket)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn until(what: &str, host: &Host, done: impl Fn(&HostState) -> bool) -> HostState {
    let start = Instant::now();
    loop {
        if let Some(state) = host.state()
            && done(&state)
        {
            return state;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "{what}");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(start.elapsed() < Duration::from_secs(10), "no socket");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_palette_and_the_text_size_reach_a_client_and_a_change_follows() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let theme = home.join(".local/state/omarchy/current/theme");
    fs::create_dir_all(&theme).unwrap();
    fs::write(theme.join("colors.toml"), COLORS).unwrap();
    fs::create_dir_all(home.join(".config/omarchy")).unwrap();
    fs::write(
        home.join(".config/omarchy/shell.toml"),
        "[font]\nbase-size = 14\n",
    )
    .unwrap();

    let socket = home.join("odkd.sock");
    let _daemon = start(home, &socket);
    wait_for(&socket);
    let host = Host::connect(&socket, Duration::from_secs(10)).unwrap();

    let state = until("the first line", &host, |s| s.colors.is_some());
    assert_eq!(state.base_size, Some(14.0));
    assert_eq!(state.colors.as_ref().unwrap()["background"], "#0D0C0B");
    assert_eq!(state.reduced_motion, None, "no compositor is running");

    fs::write(
        home.join(".config/omarchy/shell.toml.new"),
        "[font]\nbase-size = 16\n",
    )
    .unwrap();
    fs::rename(
        home.join(".config/omarchy/shell.toml.new"),
        home.join(".config/omarchy/shell.toml"),
    )
    .unwrap();
    until("the new size", &host, |s| s.base_size == Some(16.0));

    // A theme switch: Omarchy replaces the theme directory.
    let next = home.join(".local/state/omarchy/current/theme.next");
    fs::create_dir_all(&next).unwrap();
    fs::write(next.join("colors.toml"), COLORS.replace("0D0C0B", "FAFAFA")).unwrap();
    fs::remove_dir_all(&theme).unwrap();
    fs::rename(&next, &theme).unwrap();
    let state = until("the new palette", &host, |s| {
        s.colors
            .as_ref()
            .is_some_and(|c| c["background"] == "#FAFAFA")
    });
    assert_eq!(state.base_size, Some(16.0));
}

#[test]
fn a_client_that_connects_later_is_told_the_state_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let theme = home.join(".local/state/omarchy/current/theme");
    fs::create_dir_all(&theme).unwrap();
    fs::write(theme.join("colors.toml"), COLORS).unwrap();
    let socket = home.join("odkd.sock");
    let _daemon = start(home, &socket);
    wait_for(&socket);
    for _ in 0..3 {
        let host = Host::connect(&socket, Duration::from_secs(10)).unwrap();
        assert!(host.state().is_some_and(|s| s.colors.is_some()));
    }
}
