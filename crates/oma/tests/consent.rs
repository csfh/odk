//! The consent flows of `oma run`, `oma install` and `oma consent`, with a fake dialog helper
//! that records the request it receives and gives a chosen answer. The real helper's window
//! is checked by eye (`docs/checks.md`).

mod common;

use std::fs;

use common::*;

fn request(bin: &std::path::Path) -> String {
    fs::read_to_string(bin.join("request.toml")).unwrap_or_default()
}

#[test]
fn opening_a_bundle_from_outside_offers_install_run_once_and_cancel() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");
    let bin = w.oma_with_helper("cancel\n");
    let out = w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()]);
    assert!(!out.status.success());
    let req = request(&bin);
    assert!(
        req.contains("buttons = [\"install\", \"run-once\", \"cancel\"]"),
        "{req}"
    );
    assert!(
        req.contains("identifier = \"com.example.probe\"")
            && req.contains("Can play and record sound"),
        "{req}"
    );
    assert!(!w.grant_file().exists(), "Cancel records nothing");
    assert!(
        !w.home.join("Applications").exists(),
        "and installs nothing"
    );
    assert!(out.stdout.is_empty(), "and starts nothing");
}

#[test]
fn install_copies_records_and_runs_the_installed_copy() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");
    let bin = w.oma_with_helper("install\n");
    let out = w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(w.home.join("Applications/Probe.oma/manifest.toml").exists());
    assert!(w.grant_file().exists());
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("exe=/app/probe"),
        "the probe ran"
    );
    // The data directory exists because the installed application ran with one.
    assert!(w.data.exists());
}

#[test]
fn run_once_runs_with_a_private_data_directory_and_writes_nothing() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");
    let bin = w.oma_with_helper("run-once\n");
    let out = w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("write_home=ok"),
        "it could write to its private $HOME"
    );
    assert!(!w.grant_file().exists(), "no consent record");
    assert!(!w.home.join("Applications").exists(), "nothing installed");
    assert!(
        !w.data.exists(),
        "the real data directory was never created"
    );
}

#[test]
fn an_installed_application_without_a_record_is_asked_allow_or_cancel() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("audio = true");
    fs::remove_file(w.grant_file()).unwrap();
    let bin = w.oma_with_helper("allow\n");
    let out = w.oma_in(&bin, &["run", "--wait", ID]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        request(&bin).contains("buttons = [\"allow\", \"cancel\"]"),
        "{}",
        request(&bin)
    );
    assert!(
        w.grant_file().exists(),
        "Allow records exactly what was asked"
    );
    // Now nothing is missing, so the next launch does not ask.
    fs::remove_file(bin.join("request.toml")).unwrap();
    let again = w.oma_in(&bin, &["run", "--wait", ID]);
    assert!(again.status.success());
    assert!(
        !bin.join("request.toml").exists(),
        "no dialog the second time"
    );
}

#[test]
fn a_path_copy_of_an_installed_application_is_not_offered_install() {
    if !supported() {
        return;
    }
    let w = World::new();
    w.install("audio = true");
    fs::remove_file(w.grant_file()).unwrap();
    let bin = w.oma_with_helper("cancel\n");
    w.oma_in(
        &bin,
        &[
            "run",
            "--wait",
            w.home.join("out/Probe.oma").to_str().unwrap(),
        ],
    );
    assert!(
        request(&bin).contains("buttons = [\"allow\", \"cancel\"]"),
        "{}",
        request(&bin)
    );
}

#[test]
fn a_dialog_that_cannot_answer_fails_closed() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");
    for (answer, what) in [
        ("install\nextra\n", "two lines"),
        ("maybe\n", "junk"),
        ("allow\n", "an answer that was not offered"),
        ("", "nothing"),
    ] {
        let bin = w.oma_with_helper(answer);
        let out = w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()]);
        assert!(!out.status.success(), "{what}");
        assert!(
            !w.grant_file().exists() && !w.home.join("Applications").exists(),
            "{what} granted something"
        );
        assert!(out.stdout.is_empty(), "{what} started something");
        fs::remove_dir_all(w.home.join("bin")).unwrap();
    }
    // A helper that exits with an error.
    let bin = w.oma_with_helper("install\n");
    fs::write(bin.join("oma-consent"), "#!/bin/sh\necho install\nexit 3\n").unwrap();
    assert!(
        !w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()])
            .status
            .success()
    );
    // No helper at all.
    fs::remove_file(bin.join("oma-consent")).unwrap();
    let out = w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--accept-permissions"));
    assert!(!w.grant_file().exists());
}

#[test]
fn without_a_display_there_is_no_dialog_and_nothing_launches() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");
    let bin = w.oma_with_helper("install\n");
    let out = output_retrying(
        std::process::Command::new(bin.join("oma"))
            .args(["run", bundle.to_str().unwrap()])
            .env_clear()
            .env("HOME", &w.home)
            .env("XDG_RUNTIME_DIR", &w.runtime),
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("WAYLAND_DISPLAY"));
    assert!(
        !bin.join("request.toml").exists(),
        "the helper was never run"
    );
}

#[test]
fn a_dbus_bundle_never_reaches_the_dialog() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("[permissions.dbus]\ntalk = [\"org.freedesktop.Notifications\"]");
    let bin = w.oma_with_helper("install\n");
    for args in [
        vec!["run", "--wait", bundle.to_str().unwrap()],
        vec!["install", bundle.to_str().unwrap()],
        vec!["consent", bundle.to_str().unwrap()],
    ] {
        let out = w.oma_in(&bin, &args);
        assert!(!out.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("not supported"),
            "{args:?}"
        );
    }
    assert!(!bin.join("request.toml").exists());
    assert!(!w.grant_file().exists());
}

#[test]
fn install_without_the_flag_asks_and_cancel_installs_nothing() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");

    let bin = w.oma_with_helper("cancel\n");
    let out = w.oma_in(&bin, &["install", bundle.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(
        request(&bin).contains("buttons = [\"install\", \"cancel\"]"),
        "{}",
        request(&bin)
    );
    assert!(!w.home.join("Applications").exists() && !w.grant_file().exists());

    fs::write(bin.join("answer"), "install\n").unwrap();
    let out = w.oma_in(&bin, &["install", bundle.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(w.home.join("Applications/Probe.oma").exists() && w.grant_file().exists());
}

#[test]
fn the_consent_command_asks_records_and_does_not_run() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("audio = true");
    let bin = w.oma_with_helper("allow\n");
    let out = w.oma_in(&bin, &["consent", bundle.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(w.grant_file().exists());
    assert!(!w.data.exists(), "consent does not start the application");

    // Nothing missing now: it says so and opens no window.
    fs::remove_file(bin.join("request.toml")).unwrap();
    let out = w.oma_in(&bin, &["consent", bundle.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("nothing to ask"));
    assert!(!bin.join("request.toml").exists());
}

#[test]
fn the_dialog_is_told_about_folders_the_user_does_not_have() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bundle = w.bundle("[[permissions.filesystem]]\ndir = \"desktop\"\naccess = \"read\"");
    let bin = w.oma_with_helper("cancel\n");
    w.oma_in(&bin, &["run", "--wait", bundle.to_str().unwrap()]);
    assert!(
        request(&bin).contains("Desktop: you have no such folder, so this will not be available"),
        "{}",
        request(&bin)
    );
}
