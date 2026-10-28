//! The `odk` command (ADR 0056): one short umbrella that finds a separate program, `odk-<name>`,
//! and replaces itself with it, as `git` and `cargo` do. It uses only the standard library, links
//! neither `oma` nor `odk`, and has no Bundle commands of its own.
//!
//! This library is the parsing and the lookup, so both can be tested without starting a
//! process; `main.rs` is the exec.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The built-in name that is resolved only by the rule in [`Layout::oma`], never from `PATH`.
pub const OMA: &str = "oma";

/// What the arguments ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Dispatcher help on standard output.
    Help,
    /// `odk --version`.
    Version,
    /// `odk --list`.
    List,
    /// Run `odk-<name>` with these arguments (`odk help <name>` is `--help`).
    Run { name: String, args: Vec<OsString> },
    /// A usage error: the line for standard error, after `odk: `.
    Usage(String),
}

/// Reads the arguments after the program name. The first one decides; nothing else is parsed.
pub fn parse(args: &[OsString]) -> Action {
    let Some(first) = args.first() else {
        return Action::Help;
    };
    let rest = &args[1..];
    let usage = |what: &str| Action::Usage(what.to_owned());
    match first.to_str() {
        Some("help") => match rest {
            [] => Action::Help,
            [name] => match name.to_str() {
                Some(name) if valid_name(name) => Action::Run {
                    name: name.to_owned(),
                    args: vec!["--help".into()],
                },
                _ => bad_name(name),
            },
            _ => usage("help takes at most one command name"),
        },
        Some("--help" | "-h") if rest.is_empty() => Action::Help,
        Some("--version" | "-V") if rest.is_empty() => Action::Version,
        Some("--list") if rest.is_empty() => Action::List,
        Some("--help" | "-h" | "--version" | "-V" | "--list") => {
            usage(&format!("{} takes no arguments", first.to_string_lossy()))
        }
        _ if first.as_bytes().first() == Some(&b'-') => usage(&format!(
            "unknown option '{}'; flags of a command go after its name",
            first.to_string_lossy()
        )),
        Some(name) if valid_name(name) => Action::Run {
            name: name.to_owned(),
            args: rest.to_vec(),
        },
        _ => bad_name(first),
    }
}

fn bad_name(name: &OsStr) -> Action {
    Action::Usage(format!(
        "'{}' is not a command name",
        name.to_string_lossy()
    ))
}

/// An ASCII lowercase letter, then up to 63 of `a-z`, `0-9` and `-`.
pub fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Where programs are looked for: the directory of the `odk` file, and `PATH`.
#[derive(Debug, Clone)]
pub struct Layout {
    /// The directory containing the `odk` executable, when it is known.
    pub exe_dir: Option<PathBuf>,
    /// The environment's `PATH`.
    pub path: Option<OsString>,
}

impl Layout {
    /// The private directory: `../lib/odk` of a `bin` directory (`/usr/bin/odk` has
    /// `/usr/lib/odk`, `~/.local/bin/odk` has `~/.local/lib/odk`), else the directory of `odk`
    /// itself, which is a Cargo target directory.
    pub fn private_dir(&self) -> Option<PathBuf> {
        let dir = self.exe_dir.as_ref()?;
        if dir.file_name() == Some(OsStr::new("bin")) {
            Some(dir.parent()?.join("lib/odk"))
        } else {
            Some(dir.clone())
        }
    }

    /// The one place `odk-oma` is looked for. `PATH` is not consulted.
    pub fn oma(&self) -> Option<PathBuf> {
        Some(self.private_dir()?.join("odk-oma"))
    }

    /// The program for `name`: `oma` from its one place, any other name from the private
    /// directory first and then each absolute entry of `PATH`. The first regular file wins.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        if name == OMA {
            return self.oma().filter(|p| is_regular(p));
        }
        let file = format!("odk-{name}");
        self.search_dirs()
            .into_iter()
            .map(|dir| dir.join(&file))
            .find(|p| is_regular(p))
    }

    /// The private directory, then the absolute `PATH` entries, none twice. An empty, `.` or
    /// relative entry is skipped: it would mean the current directory.
    fn search_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = self.private_dir().into_iter().collect();
        if let Some(path) = &self.path {
            for entry in std::env::split_paths(path) {
                if entry.is_absolute() && entry != Path::new(".") && !dirs.contains(&entry) {
                    dirs.push(entry);
                }
            }
        }
        dirs
    }

    /// Every external command found, by name, with the file selected: the first directory that
    /// has a name hides the same name later. `oma` is the private file only.
    pub fn list(&self) -> BTreeMap<String, PathBuf> {
        let mut found = BTreeMap::new();
        if let Some(path) = self.oma().filter(|p| is_regular(p)) {
            found.insert(OMA.to_owned(), path);
        }
        for dir in self.search_dirs() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let file = entry.file_name();
                let Some(name) = file.to_str().and_then(|f| f.strip_prefix("odk-")) else {
                    continue;
                };
                if valid_name(name) && name != OMA && is_regular(&entry.path()) {
                    found.entry(name.to_owned()).or_insert_with(|| entry.path());
                }
            }
        }
        found
    }
}

/// A regular file, after following links. A directory is a miss.
fn is_regular(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// Up to three discovered names within a distance of 2 of `name`, closest first.
pub fn suggestions(name: &str, known: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut near: Vec<(usize, String)> = known
        .into_iter()
        .map(|k| (distance(name, &k), k))
        .filter(|(d, _)| *d <= 2)
        .collect();
    near.sort();
    near.into_iter().take(3).map(|(_, k)| k).collect()
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The dispatcher's help text.
pub fn help(layout: &Layout) -> String {
    let mut text = String::from(
        "usage: odk <command> [arguments...]\n\
         \n\
         Runs the program odk-<command>. Bundle commands are `odk oma <command>`; the program\n\
         odk-oma is private and is not on PATH.\n\
         \n\
         built-ins:\n  \
         help [command]   show this text, or run `odk <command> --help`\n  \
         --version, -V    print the version of odk\n  \
         --list           list the commands found, with their files\n  \
         --help, -h       show this text\n\
         \n\
         commands found:\n",
    );
    let found = layout.list();
    if found.is_empty() {
        text.push_str("  (none)\n");
    }
    for name in found.keys() {
        text.push_str(&format!("  {name}\n"));
    }
    text
}

/// The `odk --list` output: a name, a tab, the file, one per line, in byte order of the name.
pub fn list_lines(layout: &Layout) -> String {
    layout
        .list()
        .into_iter()
        .map(|(name, path)| format!("{name}\t{}\n", path.display()))
        .collect()
}

/// Why `name` was not found, for standard error.
pub fn not_found(layout: &Layout, name: &str) -> String {
    let mut text = format!("odk: no such command: '{name}'\n");
    if name == OMA {
        match layout.oma() {
            Some(path) => text.push_str(&format!(
                "odk: looked for {} only; a copy on PATH is not used\n",
                path.display()
            )),
            None => {
                text.push_str("odk: cannot tell where odk is installed, so odk-oma is not found\n")
            }
        }
    } else {
        let found = layout.list();
        let near = suggestions(name, found.keys().cloned());
        for n in near {
            text.push_str(&format!("Did you mean: odk {n}?\n"));
        }
        if found.contains_key(OMA) {
            text.push_str("Bundle commands are `odk oma <command>`.\n");
        }
    }
    text.push_str("`odk --list` shows the commands found.\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_first_argument_decides() {
        assert_eq!(parse(&os(&[])), Action::Help);
        for help in [&["help"][..], &["--help"], &["-h"]] {
            assert_eq!(parse(&os(help)), Action::Help);
        }
        assert_eq!(parse(&os(&["--version"])), Action::Version);
        assert_eq!(parse(&os(&["-V"])), Action::Version);
        assert_eq!(parse(&os(&["--list"])), Action::List);
        assert_eq!(
            parse(&os(&["oma", "--help"])),
            Action::Run {
                name: "oma".into(),
                args: os(&["--help"])
            },
            "passthrough, not a built-in flag"
        );
        assert_eq!(
            parse(&os(&["help", "oma"])),
            Action::Run {
                name: "oma".into(),
                args: os(&["--help"])
            }
        );
        assert_eq!(
            parse(&os(&["oma", "run", "--", "", "x"])),
            Action::Run {
                name: "oma".into(),
                args: os(&["run", "--", "", "x"])
            },
            "arguments pass through unchanged, `--` and empty ones included"
        );
    }

    #[test]
    fn a_flag_that_is_not_a_built_in_and_a_built_in_with_company_are_usage_errors() {
        for bad in [
            &["--nope"][..],
            &["-x", "oma"],
            &["--version", "oma"],
            &["--list", "x"],
            &["-h", "oma"],
            &["help", "a", "b"],
            &["--"],
        ] {
            assert!(matches!(parse(&os(bad)), Action::Usage(_)), "{bad:?}");
        }
    }

    #[test]
    fn a_name_is_one_lowercase_token_and_never_a_path() {
        for good in ["oma", "a", "a-b-9", &format!("a{}", "b".repeat(63))] {
            assert!(valid_name(good), "{good}");
        }
        let long = format!("a{}", "b".repeat(64));
        for bad in [
            "",
            "Oma",
            "o_ma",
            "o.ma",
            "o ma",
            "9a",
            "-a",
            "ö",
            "../odk-oma",
            "a/b",
            "a\\b",
            ".",
            "..",
            "a\0b",
            &long,
        ] {
            assert!(!valid_name(bad), "{bad:?}");
            assert!(matches!(parse(&os(&[bad])), Action::Usage(_)) || bad.is_empty());
        }
        assert!(matches!(
            parse(&os(&["/usr/lib/odk/odk-oma"])),
            Action::Usage(_)
        ));
    }

    #[test]
    fn a_name_that_is_not_utf8_is_not_a_name() {
        use std::os::unix::ffi::OsStringExt;
        let odd = vec![OsString::from_vec(vec![b'a', 0xff])];
        assert!(matches!(parse(&odd), Action::Usage(_)));
    }

    fn layout(exe_dir: &str, path: Option<&str>) -> Layout {
        Layout {
            exe_dir: Some(exe_dir.into()),
            path: path.map(Into::into),
        }
    }

    #[test]
    fn the_private_directory_follows_the_bin_directory_it_was_run_from() {
        assert_eq!(
            layout("/usr/bin", None).oma(),
            Some("/usr/lib/odk/odk-oma".into())
        );
        assert_eq!(
            layout("/home/u/.local/bin", None).oma(),
            Some("/home/u/.local/lib/odk/odk-oma".into())
        );
        assert_eq!(
            layout("/work/target/debug", None).oma(),
            Some("/work/target/debug/odk-oma".into()),
            "a Cargo directory, where the sibling is the program"
        );
        assert_eq!(
            Layout {
                exe_dir: None,
                path: None
            }
            .oma(),
            None
        );
    }

    #[test]
    fn near_names_are_suggested_closest_first_and_at_most_three() {
        let known = ["oma", "foo", "fob", "fooo", "food", "zzz"].map(String::from);
        assert_eq!(suggestions("foo", known.clone()), ["foo", "fob", "food"]);
        assert_eq!(suggestions("completely", known), Vec::<String>::new());
    }
}
