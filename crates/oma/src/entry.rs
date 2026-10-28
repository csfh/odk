//! The entry the kernel uses for a `.oma` file (ADR 0060, section 1; ADR 0056): `binfmt_misc`
//! runs the private program with the Bundle's path as its one argument.
//!
//! A `.oma` file is inert: the program never executes bytes from it. Until the phases that check
//! its publisher, read its image and ask for consent exist (issues #12 to #14), the entry seals
//! a copy, checks the envelope against itself (`trust::check`, issue #10) and refuses the file
//! whatever that check says (issue #11). A directory Bundle, and every subcommand, go the way
//! they always did.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

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

/// The refusal, after `odk oma: `. The file is sealed and its envelope checked first, so the
/// line says which refusal it is. Neither runs anything.
pub fn refusal(path: &Path) -> String {
    let checked = Sealed::memfd(path).and_then(|sealed| crate::trust::check(&sealed));
    match checked {
        Err(e) => format!(
            "{}: {e}; a file that does not verify is never run",
            path.display()
        ),
        Ok(file) => format!(
            "{}: {} {} is signed by release key {}, but a single-file Bundle is not run yet: its publisher's root is not checked and its image is not read",
            path.display(),
            file.manifest().app_id,
            file.manifest().version,
            file.manifest().key_id,
        ),
    }
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
    fn a_subcommand_or_a_flag_is_never_the_entry_whatever_files_there_are() {
        for name in ["run", "install", "help", "-h", "--version", "-"] {
            assert_eq!(single_file(args(&[name]), &subs()), None, "{name}");
        }
    }
}
