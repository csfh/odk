//! A mirror of the two Omarchy settings an application's text follows (ADR 0044): the user's
//! text size and the font they chose. An application cannot be given the files themselves:
//! `omarchy display text size` replaces `~/.config/omarchy/shell.toml` by renaming a new file
//! over it, so a bind mount of that one file would stay on the old one, and the directory it
//! sits in holds other settings an application has no business reading. So `oma` copies what
//! the application needs into a directory of its own, and the sandbox mounts that read-only
//! where the files would have been (`docs/spec/sandbox-baseline.md`). A copy is replaced by a
//! rename inside a directory that stays, which a mount and a watch on it both follow.
//!
//! Nothing here runs unless something calls [`refresh`]: `oma run` does, before every launch.

use std::fs;
use std::io;
use std::path::Path;

use crate::layout::Layout;

/// The most of `fonts.conf` that is copied. Omarchy's is under 400 bytes; a file past this is
/// not the one `omarchy font set` wrote.
const MAX_FONTS_CONF: u64 = 64 * 1024;
/// The largest base size copied. Omarchy keeps it to 9 to 20; the application keeps it there
/// too, so this only stops a number that is not a size from being written out.
const MAX_BASE_SIZE: f64 = 1000.0;

/// Copies the user's text size and font, and Hyprland's reduced-motion answer, into the mirror,
/// replacing what is there and removing what is no longer set. Both directories always exist,
/// empty for a user who has set nothing, so a sandbox always mounts them: the application
/// cannot put a file of its own where Omarchy's would be, and a watch on the directory is
/// there when a setting first appears. The mirror is [`Layout::omarchy_mirror`]:
/// `omarchy/shell.toml` holds only `[font] base-size` and, when Hyprland's animations are off,
/// `[motion] reduced = true` (ADR 0048). The user's own `[motion] reduced` is not copied.
/// `fontconfig/fonts.conf` is `omarchy font set`'s file as it is. Every copy is atomic, so a
/// reader sees the old file or the new, never half of one. Stops at the first error; a later
/// call tries again.
pub fn refresh(layout: &Layout) -> io::Result<()> {
    refresh_with(layout, crate::hyprland::animations_enabled())
}

/// [`refresh`], with what the compositor said about animating windows: the user asks for reduced
/// motion when it said they are off. When it said nothing, the mirror does not ask either. The
/// user's own `[motion] reduced` is never copied (ADR 0048).
pub fn refresh_with(layout: &Layout, animations_enabled: Option<bool>) -> io::Result<()> {
    let mirror = layout.omarchy_mirror();
    for name in ["omarchy", "fontconfig"] {
        private_dir(&mirror.join(name))?;
    }
    // A user without a `shell.toml` can still have a compositor that asks for reduced motion.
    let text = fs::read_to_string(layout.user_omarchy_shell()).unwrap_or_default();
    let shell = shell_file(&text, animations_enabled);
    publish(&mirror.join("omarchy"), "shell.toml", shell.as_deref())?;
    let fonts = fs::File::open(layout.user_fonts_conf())
        .ok()
        .filter(|f| {
            f.metadata()
                .is_ok_and(|m| m.is_file() && m.len() <= MAX_FONTS_CONF)
        })
        .and_then(|f| io::read_to_string(f).ok());
    publish(&mirror.join("fontconfig"), "fonts.conf", fonts.as_deref())
}

/// `shell.toml` holding only the `[font] base-size` of `text`, when it has one that is a
/// number, and `[motion] reduced = true` when the compositor said its animations are off
/// (ADR 0048). The user's own key is not read. `None` when there is neither a size nor a
/// request.
fn shell_file(text: &str, animations_enabled: Option<bool>) -> Option<String> {
    let table: toml::Table = toml::from_str(text).unwrap_or_default();
    let size = || {
        let value = table.get("font")?.as_table()?.get("base-size")?;
        let size = match value {
            toml::Value::Integer(i) => *i as f64,
            toml::Value::Float(f) => *f,
            _ => return None,
        };
        (size.is_finite() && size > 0.0 && size <= MAX_BASE_SIZE)
            .then(|| format!("[font]\nbase-size = {value}\n"))
    };
    let reduced =
        (animations_enabled == Some(false)).then(|| "[motion]\nreduced = true\n".to_owned());
    let parts: Vec<String> = [size(), reduced].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// Makes `dir/name` hold `content`, or not exist when there is none. The directory is there
/// already ([`private_dir`]). Nothing is written when the file already holds it.
fn publish(dir: &Path, name: &str, content: Option<&str>) -> io::Result<()> {
    let path = dir.join(name);
    let Some(content) = content else {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    };
    if fs::read_to_string(&path).is_ok_and(|now| now == content) {
        return Ok(());
    }
    let temporary = dir.join(format!(".{name}.{}", std::process::id()));
    fs::write(&temporary, content)?;
    fs::rename(&temporary, &path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

/// A directory only the user can enter, which stays.
fn private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    struct World {
        _dir: TempDir,
        layout: Layout,
    }

    fn world() -> World {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join(".config/omarchy")).unwrap();
        fs::create_dir_all(home.join(".config/fontconfig")).unwrap();
        let layout = Layout::new(&home, home.join(".local/share"));
        World { _dir: dir, layout }
    }

    fn mirrored(w: &World, file: &str) -> Option<String> {
        fs::read_to_string(w.layout.omarchy_mirror().join(file)).ok()
    }

    const FONTS: &str = "<?xml version=\"1.0\"?>\n<fontconfig><match/></fontconfig>\n";

    /// The user's own `[motion] reduced` is not a source (ADR 0048). Hyprland's answer is.
    #[test]
    fn the_users_reduced_motion_key_is_not_copied() {
        let w = world();
        let write = |text: &str| fs::write(w.layout.user_omarchy_shell(), text).unwrap();
        for text in [
            "[motion]\nreduced = true\nspeed = 3\n\n[bar]\ntext = \"#fff\"\n",
            "[motion]\nreduced = true\n",
            "[motion]\nreduced = false\n",
            "[motion]\nreduced = \"yes\"\n",
            "[font]\nbase-size = 14\n\n[motion]\nreduced = true\n",
        ] {
            write(text);
            refresh_with(&w.layout, None).unwrap();
            let mirrored = mirrored(&w, "omarchy/shell.toml");
            assert!(
                mirrored.as_deref().is_none_or(|s| !s.contains("reduced")),
                "the user's key must not reach the copy: {text:?} -> {mirrored:?}"
            );
        }
        // A text size beside the key is still copied, and the key is not.
        write("[font]\nbase-size = 14\n\n[motion]\nreduced = true\n");
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(
            mirrored(&w, "omarchy/shell.toml").as_deref(),
            Some("[font]\nbase-size = 14\n")
        );
    }

    #[test]
    fn only_the_base_size_is_copied_from_the_users_shell_toml() {
        let w = world();
        fs::write(
            w.layout.user_omarchy_shell(),
            "[bar]\ntext = \"#ffffff\"\n\n[font]\nbase-size = 14\nbody = 20\n",
        )
        .unwrap();
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(
            mirrored(&w, "omarchy/shell.toml").as_deref(),
            Some("[font]\nbase-size = 14\n"),
            "nothing else of the file reaches the application"
        );
    }

    #[test]
    fn a_float_is_copied_as_it_is_and_what_is_not_a_size_is_not() {
        let w = world();
        let write = |text: &str| fs::write(w.layout.user_omarchy_shell(), text).unwrap();
        write("[font]\nbase-size = 13.5\n");
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(
            mirrored(&w, "omarchy/shell.toml").as_deref(),
            Some("[font]\nbase-size = 13.5\n")
        );
        for bad in [
            "[font]\nbase-size = \"big\"\n",
            "[font]\nbase-size = nan\n",
            "[font]\nbase-size = 0\n",
            "[font]\nbase-size = -4\n",
            "[font]\nbase-size = 99999\n",
            "[font]\nbase-size = 1e300\n",
            "[font]\nbody = 20\n",
            "base-size = 14\n",
            "not toml [",
        ] {
            write(bad);
            refresh_with(&w.layout, None).unwrap();
            assert_eq!(mirrored(&w, "omarchy/shell.toml"), None, "{bad:?}");
        }
    }

    #[test]
    fn a_setting_the_user_takes_away_is_taken_away_from_the_mirror() {
        let w = world();
        fs::write(w.layout.user_omarchy_shell(), "[font]\nbase-size = 16\n").unwrap();
        fs::write(w.layout.user_fonts_conf(), FONTS).unwrap();
        refresh_with(&w.layout, None).unwrap();
        assert!(mirrored(&w, "omarchy/shell.toml").is_some());
        assert_eq!(
            mirrored(&w, "fontconfig/fonts.conf").as_deref(),
            Some(FONTS)
        );

        fs::remove_file(w.layout.user_omarchy_shell()).unwrap();
        fs::remove_file(w.layout.user_fonts_conf()).unwrap();
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(mirrored(&w, "omarchy/shell.toml"), None);
        assert_eq!(mirrored(&w, "fontconfig/fonts.conf"), None);
        // The directories stay, which is what a mount of them needs.
        assert!(w.layout.omarchy_mirror().join("omarchy").is_dir());
    }

    #[test]
    fn a_new_size_replaces_the_old_by_a_rename_inside_the_same_directory() {
        use std::os::unix::fs::MetadataExt;
        let w = world();
        let dir = w.layout.omarchy_mirror().join("omarchy");
        fs::write(w.layout.user_omarchy_shell(), "[font]\nbase-size = 14\n").unwrap();
        refresh_with(&w.layout, None).unwrap();
        let (dir_inode, file_inode) = (
            fs::metadata(&dir).unwrap().ino(),
            fs::metadata(dir.join("shell.toml")).unwrap().ino(),
        );

        // The same content writes nothing.
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(
            fs::metadata(dir.join("shell.toml")).unwrap().ino(),
            file_inode
        );

        fs::write(w.layout.user_omarchy_shell(), "[font]\nbase-size = 18\n").unwrap();
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(
            mirrored(&w, "omarchy/shell.toml").as_deref(),
            Some("[font]\nbase-size = 18\n")
        );
        assert_eq!(
            fs::metadata(&dir).unwrap().ino(),
            dir_inode,
            "the directory a sandbox has mounted is the same one"
        );
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, ["shell.toml"], "no temporary file is left");
    }

    #[test]
    fn a_fonts_conf_that_is_not_a_small_file_is_not_copied() {
        let w = world();
        fs::write(
            w.layout.user_fonts_conf(),
            vec![b' '; MAX_FONTS_CONF as usize + 1],
        )
        .unwrap();
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(mirrored(&w, "fontconfig/fonts.conf"), None);

        fs::remove_file(w.layout.user_fonts_conf()).unwrap();
        fs::create_dir(w.layout.user_fonts_conf()).unwrap();
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(mirrored(&w, "fontconfig/fonts.conf"), None);
    }

    #[test]
    fn a_user_with_no_omarchy_settings_gets_an_empty_mirror_and_no_error() {
        let dir = TempDir::new().unwrap();
        let layout = Layout::new(dir.path().join("home"), dir.path().join("data"));
        refresh(&layout).unwrap();
        let mirror = layout.omarchy_mirror();
        assert!(!mirror.join("omarchy/shell.toml").exists());
        assert!(!mirror.join("fontconfig/fonts.conf").exists());
        // But the directories are there, for a sandbox to mount and for a watch to start on.
        assert!(mirror.join("omarchy").is_dir() && mirror.join("fontconfig").is_dir());
    }

    #[test]
    fn the_compositors_word_is_copied_and_silence_does_not_ask() {
        let w = world();
        let mirrored_motion = |w: &World| mirrored(w, "omarchy/shell.toml");
        let write = |text: &str| fs::write(w.layout.user_omarchy_shell(), text).unwrap();
        const ASKED: Option<&str> = Some("[motion]\nreduced = true\n");

        // Animations off: the request, even from a user with no `shell.toml` at all.
        let _ = fs::remove_file(w.layout.user_omarchy_shell());
        refresh_with(&w.layout, Some(false)).unwrap();
        assert_eq!(mirrored_motion(&w).as_deref(), ASKED);

        // Animations on: no request, whatever the key says.
        write("[motion]\nreduced = true\n");
        refresh_with(&w.layout, Some(true)).unwrap();
        assert_eq!(mirrored_motion(&w), None);

        // And off beside a text size: both are written, and a key that says no does not undo it.
        write("[font]\nbase-size = 14\n[motion]\nreduced = false\n");
        refresh_with(&w.layout, Some(false)).unwrap();
        assert_eq!(
            mirrored_motion(&w).as_deref(),
            Some("[font]\nbase-size = 14\n\n[motion]\nreduced = true\n")
        );

        // The compositor says nothing: the user's key is ignored, and nothing is asked.
        write("[motion]\nreduced = true\n");
        refresh_with(&w.layout, None).unwrap();
        assert_eq!(mirrored_motion(&w), None);
    }
}
