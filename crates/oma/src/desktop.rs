//! The launcher's entry for an installed application (ADR 0014, ADR 0056): one `.desktop` file per
//! Identifier, written from the Manifest, whose `Exec` runs the Identifier through the absolute
//! `odk` facade, never the program, so the sandbox always applies. `install`, `dev`, `run` of an
//! installed copy and `--register` all write it with [`write`], so a run and a scan give one file.
//! Run once writes nothing (ADR 0018).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::fmt::{Identifier, Manifest};
use crate::layout::Layout;

/// The line that marks an entry as one `odk-oma` wrote, and names its Identifier. Entries without
/// it are never changed or removed.
pub const MARK: &str = "X-Oma-Identifier=";

/// The facade an entry names (ADR 0056): `$prefix/bin/odk` when this program is
/// `$prefix/lib/odk/odk-oma`, and `odk` when it is a build run in place.
pub fn facade() -> PathBuf {
    let installed = || -> Option<PathBuf> {
        let exe = fs::canonicalize(std::env::current_exe().ok()?).ok()?;
        let lib = exe.parent()?;
        if !lib.ends_with("lib/odk") {
            return None;
        }
        Some(lib.parent()?.parent()?.join("bin/odk"))
    };
    installed().unwrap_or_else(|| PathBuf::from("odk"))
}

/// The entry for `id`: its name, description and categories from `manifest`, its icon when it has
/// one on disk, and `Exec` the facade `odk` running it.
pub fn text(id: &Identifier, manifest: &Manifest, icon: Option<&Path>, odk: &Path) -> String {
    let mut text = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={} oma run {id}\n",
        escape(&manifest.name),
        exec_word(&odk.to_string_lossy()),
    );
    if let Some(description) = &manifest.description {
        text.push_str(&format!("Comment={}\n", escape(description)));
    }
    if !manifest.categories.is_empty() {
        text.push_str(&format!("Categories={};\n", manifest.categories.join(";")));
    }
    if let Some(icon) = icon {
        text.push_str(&format!("Icon={}\n", escape(&icon.to_string_lossy())));
    }
    text.push_str(&format!("{MARK}{id}\n"));
    text
}

/// Where `id`'s entry is.
pub fn path(layout: &Layout, id: &Identifier) -> PathBuf {
    layout.desktop_entries().join(format!("{id}.desktop"))
}

/// Writes `id`'s entry, replacing it in one step and only when it would change.
pub fn write(layout: &Layout, id: &Identifier, text: &str) -> io::Result<()> {
    let entry = path(layout, id);
    if fs::read_to_string(&entry).ok().as_deref() == Some(text) {
        return Ok(());
    }
    let dir = layout.desktop_entries();
    fs::create_dir_all(&dir)?;
    let partial = dir.join(format!(".{id}.desktop.partial"));
    fs::write(&partial, text)?;
    fs::rename(&partial, &entry)
}

/// Removes every entry `odk-oma` wrote that is not `keep`'s, under its own name: those of
/// applications no longer installed, and any written under another name.
pub fn remove_others(layout: &Layout, keep: &[Identifier]) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(layout.desktop_entries()) else {
        return Ok(());
    };
    for entry in entries.filter_map(Result::ok).map(|e| e.path()) {
        if entry.extension().is_none_or(|e| e != "desktop") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&entry) else {
            continue;
        };
        let Some(id) = text.lines().find_map(|l| l.strip_prefix(MARK)) else {
            continue;
        };
        let wanted = keep
            .iter()
            .find(|k| k.as_str() == id)
            .is_some_and(|k| path(layout, k) == entry);
        if !wanted {
            fs::remove_file(&entry)?;
        }
    }
    Ok(())
}

/// A `.desktop` string value: `\` is the one escape a Manifest's text can need, since it holds no
/// control characters (`docs/spec/manifest.md`).
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
}

/// One word of an `Exec` line: quoted, with the characters the specification reserves escaped,
/// unless it holds only characters that need neither.
fn exec_word(word: &str) -> String {
    if word
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    {
        return word.to_owned();
    }
    let mut quoted = String::from("\"");
    for c in word.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    // Then the escaping of every string value, which doubles each backslash.
    escape(&quoted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(extra: &str) -> Manifest {
        Manifest::parse(&format!(
            "manifest_version = 1\nidentifier = \"dev.csfh.notes\"\nname = \"Notes\"\nversion = \"1.0.0\"\nexecutable = \"bin/notes\"\n{extra}\n[permissions]\n"
        ))
        .unwrap()
    }

    #[test]
    fn an_entry_runs_the_identifier_through_the_absolute_facade() {
        let id: Identifier = "dev.csfh.notes".parse().unwrap();
        let odk = Path::new("/home/me/.local/bin/odk");
        assert_eq!(
            text(&id, &manifest(""), None, odk),
            "[Desktop Entry]\nType=Application\nName=Notes\nExec=/home/me/.local/bin/odk oma run dev.csfh.notes\nX-Oma-Identifier=dev.csfh.notes\n"
        );
        let full = manifest(
            "description = \"Write things down\"\ncategories = [\"Office\", \"Utility\"]\nicon = \"share/notes.svg\"",
        );
        let icon = Path::new("/home/me/Applications/Notes.oma/share/notes.svg");
        assert_eq!(
            text(&id, &full, Some(icon), odk),
            "[Desktop Entry]\nType=Application\nName=Notes\nExec=/home/me/.local/bin/odk oma run dev.csfh.notes\nComment=Write things down\nCategories=Office;Utility;\nIcon=/home/me/Applications/Notes.oma/share/notes.svg\nX-Oma-Identifier=dev.csfh.notes\n"
        );
        // A home with a space and a dollar sign is quoted as the specification says.
        let odd = Path::new("/home/a b$/.local/bin/odk");
        assert!(
            text(&id, &manifest(""), None, odd)
                .contains("Exec=\"/home/a b\\\\$/.local/bin/odk\" oma run dev.csfh.notes\n")
        );
    }

    #[test]
    fn only_entries_odk_oma_wrote_and_no_longer_wants_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path().join("home"), dir.path().join("data"));
        let keep: Identifier = "dev.csfh.keep".parse().unwrap();
        let gone: Identifier = "dev.csfh.gone".parse().unwrap();
        for id in [&keep, &gone] {
            write(
                &layout,
                id,
                &text(id, &manifest(""), None, Path::new("odk")),
            )
            .unwrap();
        }
        let entries = layout.desktop_entries();
        let legacy = entries.join("oma-dev.csfh.keep.desktop");
        fs::write(&legacy, format!("[Desktop Entry]\n{MARK}dev.csfh.keep\n")).unwrap();
        let foreign = entries.join("dev.csfh.gone.other.desktop");
        fs::write(&foreign, "[Desktop Entry]\nName=Someone else's\n").unwrap();

        remove_others(&layout, std::slice::from_ref(&keep)).unwrap();
        assert!(path(&layout, &keep).exists());
        assert!(!path(&layout, &gone).exists());
        assert!(
            !legacy.exists(),
            "an entry under another name is replaced by the one name"
        );
        assert!(foreign.exists());
    }
}
