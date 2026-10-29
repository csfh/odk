//! The entry the kernel uses for a `.oma` file (ADR 0060, section 1; ADR 0056): `binfmt_misc`
//! runs the private program with the Bundle's path as its one argument.
//!
//! A `.oma` file is inert: the program never executes bytes from it. Until the phases that check
//! its publisher and ask for consent exist (issues #13 and #14), the entry seals a copy, checks
//! the envelope against itself (`trust::check`, issue #10), reads its image (`image`, issue
//! #12), and refuses the file whatever those say (issue #11). A directory Bundle, and every
//! subcommand, go the way they always did.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use std::fmt::Display;

use crate::image::Image;
use crate::trust::Sealed;

/// The path, when the arguments are the entry: exactly one argument, which is not a
/// subcommand, does not start with `-`, and names a regular file. The file is not opened: its
/// metadata is read, and that is all.
pub fn single_file(
    mut args: impl Iterator<Item = OsString>,
    subcommands: &[String],
) -> Option<PathBuf> {
    let arg = args.next()?;
    if args.next().is_some() || arg.as_bytes().first() == Some(&b'-') {
        return None;
    }
    if arg
        .to_str()
        .is_some_and(|a| subcommands.iter().any(|s| s == a))
    {
        return None;
    }
    let path = PathBuf::from(arg);
    std::fs::metadata(&path).ok()?.is_file().then_some(path)
}

/// The refusal, after `odk oma: `. The file is sealed, its envelope checked, and only then its
/// image read (ADR 0060, section 6), so the line says which refusal it is. None of it runs
/// anything.
pub fn refusal(path: &Path) -> String {
    let unverified = |e: &dyn Display| {
        format!(
            "{}: {e}; a file that does not verify is never run",
            path.display()
        )
    };
    let sealed = match Sealed::memfd(path) {
        Ok(sealed) => sealed,
        Err(e) => return unverified(&e),
    };
    let checked = match crate::trust::check(&sealed) {
        Ok(checked) => checked,
        Err(e) => return unverified(&e),
    };
    if let Err(e) = Image::read(&checked) {
        return unverified(&e);
    }
    let manifest = checked.signed().manifest();
    format!(
        "{}: {} {} is signed by release key {} and its image matches it, but a single-file Bundle is not run yet: its publisher's root is not checked",
        path.display(),
        manifest.app_id,
        manifest.version,
        manifest.key_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn args(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn subs() -> Vec<String> {
        ["run", "install", "help"].map(String::from).to_vec()
    }

    #[test]
    fn one_argument_naming_a_regular_file_is_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("App.oma");
        fs::write(&file, b"OMA1").unwrap();
        let path = file.to_str().unwrap();
        assert_eq!(single_file(args(&[path]), &subs()), Some(file.clone()));
        assert_eq!(
            single_file(args(&[path, "x"]), &subs()),
            None,
            "two arguments"
        );
        assert_eq!(single_file(args(&[]), &subs()), None);
        let missing = dir.path().join("none.oma");
        assert_eq!(
            single_file(args(&[missing.to_str().unwrap()]), &subs()),
            None
        );
        assert_eq!(
            single_file(args(&[dir.path().to_str().unwrap()]), &subs()),
            None,
            "a directory"
        );
    }

    #[test]
    fn the_refusal_says_whether_the_envelope_or_the_image_failed_or_neither() {
        use crate::image::fixture::{build, notes, notes_with};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Notes.oma");
        let refusal_of = |bytes: &[u8]| {
            fs::write(&path, bytes).unwrap();
            refusal(&path)
        };

        let said = refusal_of(&build(&notes()).signed());
        assert!(
            said.contains("dev.csfh.notes 7 is signed")
                && said.contains("its image matches it")
                && said.contains("not run yet"),
            "{said}"
        );

        let said = refusal_of(&build(&notes_with("manifest.toml", None)).signed());
        assert!(
            said.contains("no manifest.toml") && said.contains("never run"),
            "{said}"
        );

        let mut tampered = build(&notes()).signed();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        let said = refusal_of(&tampered);
        assert!(
            said.contains("payload digest") && said.contains("never run"),
            "{said}"
        );
    }

    #[test]
    fn a_subcommand_or_a_flag_is_never_the_entry_whatever_files_there_are() {
        for name in ["run", "install", "help", "-h", "--version", "-"] {
            assert_eq!(single_file(args(&[name]), &subs()), None, "{name}");
        }
    }
}
