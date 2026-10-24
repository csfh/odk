//! Asking the user for consent (ADR 0016, ADR 0025, ADR 0033): the request `oma` sends to the
//! `oma-consent` helper and the answer it accepts back. The format is in
//! `docs/spec/consent-dialog.md`.
//!
//! Anything but one offered answer from a helper that ran and exited cleanly is an error, and
//! the caller fails closed.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::fmt::{Bundle, Permissions, UserDir, permission_lines};
use crate::userdirs::Resolved;

/// An answer the dialog can give.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Install,
    RunOnce,
    Allow,
    Cancel,
}

impl Button {
    pub fn word(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::RunOnce => "run-once",
            Self::Allow => "allow",
            Self::Cancel => "cancel",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        [Self::Install, Self::RunOnce, Self::Allow, Self::Cancel]
            .into_iter()
            .find(|b| b.word() == word)
    }
}

/// What the helper is shown. Built from a Bundle and what is missing from its consent record.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Request {
    pub name: String,
    pub identifier: String,
    pub version: String,
    /// `"new"`; the upgrade screen is not built (`docs/spec/consent-dialog.md`).
    pub situation: &'static str,
    pub buttons: Vec<&'static str>,
    pub lines: Vec<String>,
    pub notes: Vec<String>,
}

impl Request {
    /// `missing` is the part of the Bundle's request its consent record does not cover.
    pub fn new(
        bundle: &Bundle,
        missing: &Permissions,
        buttons: &[Button],
        named_dirs: &Resolved,
    ) -> Self {
        let manifest = bundle.manifest();
        Self {
            name: manifest.name.clone(),
            identifier: manifest.identifier.to_string(),
            version: manifest.version.to_string(),
            situation: "new",
            buttons: buttons.iter().map(|b| b.word()).collect(),
            lines: permission_lines(missing),
            notes: missing
                .filesystem
                .iter()
                .filter(|g| named_dirs.get(g.dir).is_err())
                .map(|g| {
                    format!(
                        "{}: you have no such folder, so this will not be available",
                        folder_title(g.dir)
                    )
                })
                .collect(),
        }
    }
}

fn folder_title(dir: UserDir) -> &'static str {
    match dir {
        UserDir::Documents => "Documents",
        UserDir::Downloads => "Downloads",
        UserDir::Pictures => "Pictures",
        UserDir::Videos => "Videos",
        UserDir::Music => "Music",
        UserDir::Desktop => "Desktop",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("there is no display to show the consent dialog on (WAYLAND_DISPLAY is not set)")]
    NoDisplay,
    #[error("the consent dialog helper {} is missing", .0.display())]
    HelperMissing(PathBuf),
    #[error("cannot run the consent dialog helper: {0}")]
    Spawn(io::Error),
    #[error("the consent dialog helper failed ({0})")]
    Failed(String),
    #[error(
        "the consent dialog helper answered {0:?}, which is not one line naming an offered button"
    )]
    BadAnswer(String),
}

/// Where the helper is looked for: next to this executable (ADR 0025), and nowhere else, so
/// nothing in the environment can stand in for the dialog.
pub fn helper_path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("oma-consent"))
}

/// Shows the dialog with the real helper.
pub fn ask(request: &Request) -> Result<Button, Error> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none_or(|d| d.is_empty()) {
        return Err(Error::NoDisplay);
    }
    let helper = helper_path().ok_or_else(|| Error::HelperMissing("oma-consent".into()))?;
    ask_with(&helper, request)
}

fn ask_with(helper: &Path, request: &Request) -> Result<Button, Error> {
    if !helper.exists() {
        return Err(Error::HelperMissing(helper.to_owned()));
    }
    let text = toml::to_string(request).expect("a request serialises");
    let mut child = Command::new(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(Error::Spawn)?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    // A helper that exits without reading is reported by its status below, not here.
    let _ = stdin.write_all(text.as_bytes());
    drop(stdin);
    let output = child.wait_with_output().map_err(Error::Spawn)?;
    if !output.status.success() {
        return Err(Error::Failed(output.status.to_string()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let word = stdout.strip_suffix('\n').unwrap_or(&stdout);
    match Button::parse(word) {
        Some(button) if !word.contains('\n') && request.buttons.contains(&button.word()) => {
            Ok(button)
        }
        _ => Err(Error::BadAnswer(stdout)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;
    use crate::fmt::{Access, FsGrant};
    use crate::testutil::bundle;
    use crate::userdirs;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("helper");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn request(buttons: &[Button]) -> Request {
        let dir = TempDir::new().unwrap();
        let b = Bundle::open(&bundle(
            dir.path(),
            "Tee",
            "com.example.t",
            "1.2.3",
            "gpu = true",
        ))
        .unwrap();
        let named = userdirs::resolve(dir.path(), &dir.path().join("none"));
        Request::new(&b, &b.manifest().permissions, buttons, &named)
    }

    #[test]
    fn the_request_carries_what_the_spec_lists() {
        let dir = TempDir::new().unwrap();
        let b = Bundle::open(&bundle(
            dir.path(),
            "Tee",
            "com.example.t",
            "1.2.3",
            "gpu = true\n[[permissions.filesystem]]\ndir = \"desktop\"\naccess = \"read\"",
        ))
        .unwrap();
        let named = userdirs::resolve(dir.path(), &dir.path().join("none"));
        let r = Request::new(
            &b,
            &b.manifest().permissions,
            &[Button::Install, Button::RunOnce, Button::Cancel],
            &named,
        );
        assert_eq!(
            (
                r.name.as_str(),
                r.identifier.as_str(),
                r.version.as_str(),
                r.situation
            ),
            ("Tee", "com.example.t", "1.2.3", "new")
        );
        assert_eq!(r.buttons, ["install", "run-once", "cancel"]);
        assert_eq!(
            r.lines,
            [
                "Can use the graphics card directly",
                "Can read files in your Desktop folder"
            ]
        );
        assert_eq!(
            r.notes,
            ["Desktop: you have no such folder, so this will not be available"]
        );
        let text = toml::to_string(&r).unwrap();
        assert!(
            text.contains("situation = \"new\"")
                && text.contains("buttons = [\"install\", \"run-once\", \"cancel\"]"),
            "{text}"
        );
    }

    #[test]
    fn an_offered_one_line_answer_is_accepted() {
        let dir = TempDir::new().unwrap();
        let helper = script(dir.path(), "cat > /dev/null; echo allow");
        assert_eq!(
            ask_with(&helper, &request(&[Button::Allow, Button::Cancel])).unwrap(),
            Button::Allow
        );
        let helper = script(dir.path(), "cat > /dev/null; printf cancel");
        assert_eq!(
            ask_with(&helper, &request(&[Button::Allow, Button::Cancel])).unwrap(),
            Button::Cancel
        );
    }

    #[test]
    fn the_helper_gets_the_request_on_stdin() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("log");
        let helper = script(dir.path(), &format!("cat > {}; echo cancel", log.display()));
        ask_with(&helper, &request(&[Button::Allow, Button::Cancel])).unwrap();
        let seen = fs::read_to_string(log).unwrap();
        assert!(
            seen.contains("identifier = \"com.example.t\"")
                && seen.contains("Can use the graphics card directly"),
            "{seen}"
        );
    }

    #[test]
    fn everything_else_is_an_error() {
        let dir = TempDir::new().unwrap();
        let req = request(&[Button::Allow, Button::Cancel]);
        for (body, what) in [
            (
                "cat > /dev/null; echo install",
                "an answer that was not offered",
            ),
            ("cat > /dev/null; echo allow; echo allow", "two lines"),
            ("cat > /dev/null; echo ALLOW", "junk"),
            ("cat > /dev/null", "no output"),
            ("cat > /dev/null; echo allow; exit 3", "a non-zero exit"),
            ("cat > /dev/null; echo 'allow '", "trailing space"),
        ] {
            let helper = script(dir.path(), body);
            assert!(ask_with(&helper, &req).is_err(), "{what}");
        }
        assert!(matches!(
            ask_with(&dir.path().join("nope"), &req),
            Err(Error::HelperMissing(_))
        ));
    }

    #[test]
    fn notes_only_for_folders_that_are_unavailable() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("Documents")).unwrap();
        fs::write(
            dir.path().join("user-dirs.dirs"),
            "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\n",
        )
        .unwrap();
        let named = userdirs::resolve(dir.path(), &dir.path().join("user-dirs.dirs"));
        let b = Bundle::open(&bundle(dir.path(), "Tee", "com.example.t", "1.0.0", "")).unwrap();
        let missing = Permissions {
            filesystem: vec![
                FsGrant {
                    dir: UserDir::Documents,
                    access: Access::Write,
                },
                FsGrant {
                    dir: UserDir::Music,
                    access: Access::Read,
                },
            ],
            ..Default::default()
        };
        let r = Request::new(&b, &missing, &[Button::Allow, Button::Cancel], &named);
        assert_eq!(
            r.notes,
            ["Music: you have no such folder, so this will not be available"]
        );
    }
}
