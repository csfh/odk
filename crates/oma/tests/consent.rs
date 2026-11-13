//! The consent flows of `odk oma run` and `odk oma consent` for an installed Bundle file, with a
//! fake dialog helper that records the request it receives and gives a chosen answer. Opening a
//! file that is not installed (Install, Run once or Cancel) and `install` need a publisher's key
//! set, which no test fetches; `flow`'s unit tests cover them with a fake fetch. The real
//! helper's window is checked by eye (`docs/checks.md`).

mod common;

use std::fs;

use common::*;

fn request(bin: &std::path::Path) -> String {
    fs::read_to_string(bin.join("request.toml")).unwrap_or_default()
}

/// The probe installed, asking for `permissions`, with a pair that allows none of it yet, as
/// after an upgrade that asks for more (ADR 0015).
fn installed_asking(w: &World, permissions: &str) {
    w.install(permissions);
    w.set_consent("");
}

#[test]
fn an_installed_file_that_asks_for_more_is_asked_allow_or_cancel() {
    if !supported() {
        return;
    }
    let w = World::new();
    installed_asking(&w, "audio = true");
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
    assert_eq!(
        w.allowed(),
        ["Can play and record sound"],
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
fn a_dialog_that_cannot_answer_fails_closed() {
    if !supported() {
        return;
    }
    let w = World::new();
    installed_asking(&w, "audio = true");
    for (answer, what) in [
        ("allow\nextra\n", "two lines"),
        ("maybe\n", "junk"),
        ("install\n", "an answer that was not offered"),
        ("", "nothing"),
    ] {
        let bin = w.oma_with_helper(answer);
        let out = w.oma_in(&bin, &["run", "--wait", ID]);
        assert!(!out.status.success(), "{what}");
        assert!(w.allowed().is_empty(), "{what} granted something");
        assert!(out.stdout.is_empty(), "{what} started something");
        fs::remove_dir_all(w.home.join("bin")).unwrap();
    }
    // A helper that exits with an error.
    let bin = w.oma_with_helper("allow\n");
    fs::write(bin.join("oma-consent"), "#!/bin/sh\necho allow\nexit 3\n").unwrap();
    assert!(!w.oma_in(&bin, &["run", "--wait", ID]).status.success());
    // No helper at all.
    fs::remove_file(bin.join("oma-consent")).unwrap();
    let out = w.oma_in(&bin, &["run", "--wait", ID]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no consent was recorded"));
    assert!(w.allowed().is_empty());
}

#[test]
fn without_a_display_there_is_no_dialog_and_nothing_launches() {
    if !supported() {
        return;
    }
    let w = World::new();
    installed_asking(&w, "audio = true");
    let bin = w.oma_with_helper("allow\n");
    let out = output_retrying(
        std::process::Command::new(bin.join("odk-oma"))
            .args(["run", ID])
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
    installed_asking(
        &w,
        "[permissions.dbus]\ntalk = [\"org.freedesktop.Notifications\"]",
    );
    let bin = w.oma_with_helper("allow\n");
    for args in [vec!["run", "--wait", ID], vec!["consent", ID]] {
        let out = w.oma_in(&bin, &args);
        assert!(!out.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("not supported"),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(!bin.join("request.toml").exists());
    assert!(w.allowed().is_empty());
}

#[test]
fn the_consent_command_asks_records_and_does_not_run() {
    if !supported() {
        return;
    }
    let w = World::new();
    installed_asking(&w, "audio = true");
    let bin = w.oma_with_helper("allow\n");
    let out = w.oma_in(&bin, &["consent", ID]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(w.allowed(), ["Can play and record sound"]);
    assert!(!w.data.exists(), "consent does not start the application");

    // Nothing missing now: it says so and opens no window, by Identifier or by the file's path.
    fs::remove_file(bin.join("request.toml")).unwrap();
    let file = w.home.join("Applications/Probe.oma");
    for target in [ID, file.to_str().unwrap()] {
        let out = w.oma_in(&bin, &["consent", target]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("nothing to ask"));
    }
    assert!(!bin.join("request.toml").exists());
}

#[test]
fn consent_is_refused_for_a_directory_and_for_what_is_not_installed() {
    if !supported() {
        return;
    }
    let w = World::new();
    let bin = w.oma_with_helper("allow\n");
    let out = w.oma_in(&bin, &["consent", ID]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no installed application is called"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dir = w.home.join("Probe.oma");
    fs::create_dir_all(&dir).unwrap();
    let out = w.oma_in(&bin, &["consent", dir.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("is a directory; a Bundle is a signed .oma file"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!bin.join("request.toml").exists());
}

#[test]
fn the_dialog_is_told_about_folders_the_user_does_not_have() {
    if !supported() {
        return;
    }
    let w = World::new();
    installed_asking(
        &w,
        "[[permissions.filesystem]]\ndir = \"desktop\"\naccess = \"read\"",
    );
    let bin = w.oma_with_helper("cancel\n");
    w.oma_in(&bin, &["run", "--wait", ID]);
    assert!(
        request(&bin).contains("Desktop: you have no such folder, so this will not be available"),
        "{}",
        request(&bin)
    );
}
