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
