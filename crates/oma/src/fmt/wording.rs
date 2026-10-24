//! The plain-language permission lines of `docs/spec/consent-dialog.md`.

use crate::fmt::{Access, Permissions, UserDir};

/// One line per requested permission, stating the consequence rather than the name.
pub fn permission_lines(p: &Permissions) -> Vec<String> {
    let mut lines = Vec::new();
    if p.network {
        lines.push("Can use the network, including services running on this computer".into());
    }
    if p.gpu {
        lines.push("Can use the graphics card directly".into());
    }
    if p.audio {
        lines.push("Can play and record sound".into());
    }
    if p.updates {
        lines
            .push("Can update itself; you'll be asked again if an update wants more access".into());
    }
    for grant in &p.filesystem {
        let what = match grant.access {
            Access::Read => "read files in",
            Access::Write => "read and change files in",
        };
        lines.push(format!("Can {what} your {} folder", folder_name(grant.dir)));
    }
    if !p.dbus_talk.is_empty() {
        lines.push(format!(
            "Can talk to these desktop services: {}",
            p.dbus_talk.join(", ")
        ));
    }
    lines
}

fn folder_name(dir: UserDir) -> &'static str {
    match dir {
        UserDir::Documents => "Documents",
        UserDir::Downloads => "Downloads",
        UserDir::Pictures => "Pictures",
        UserDir::Videos => "Videos",
        UserDir::Music => "Music",
        UserDir::Desktop => "Desktop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::FsGrant;

    #[test]
    fn nothing_requested_gives_no_lines() {
        assert!(permission_lines(&Permissions::default()).is_empty());
    }

    #[test]
    fn every_permission_has_its_line_in_order() {
        let p = Permissions {
            network: true,
            gpu: true,
            audio: true,
            updates: true,
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
            dbus_talk: vec!["org.a".into(), "org.b".into()],
        };
        assert_eq!(
            permission_lines(&p),
            [
                "Can use the network, including services running on this computer",
                "Can use the graphics card directly",
                "Can play and record sound",
                "Can update itself; you'll be asked again if an update wants more access",
                "Can read and change files in your Documents folder",
                "Can read files in your Music folder",
                "Can talk to these desktop services: org.a, org.b",
            ]
        );
    }
}
