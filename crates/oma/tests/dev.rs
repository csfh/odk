//! `odk oma dev --no-run` installs the built Bundle and leaves it unstarted.

mod common;

use std::fs;
use std::process::Command;

use common::{World, probe_binary};

#[test]
fn dev_no_run_installs_the_bundle_and_leaves_it_unstarted() {
    let w = World::new();
    let project = w.home.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::copy(probe_binary(), project.join("probe")).unwrap();
    fs::write(
        project.join("manifest.toml"),
        "manifest_version = 1\nidentifier = \"com.example.norun\"\nname = \"Norun\"\nversion = \"1.0.0\"\nexecutable = \"probe\"\npublisher = \"apps.example.com\"\n\n[permissions]\n",
    )
    .unwrap();
    fs::write(
        project.join("oma.toml"),
        "build = []\nexecutable = \"probe\"\n",
    )
    .unwrap();

    let out = Command::new(&w.oma_bin)
        .args(["dev", "--no-run"])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &w.home)
        .env("XDG_RUNTIME_DIR", &w.runtime)
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env("LANG", "en_US.UTF-8")
        .env("PATH", "/usr/bin")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("installed at"), "{stderr}");
    assert!(!stdout.contains("pid="), "the program started: {stdout}");
    assert!(w.home.join("Applications/Norun.oma").is_file());
}
