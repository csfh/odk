//! `odk oma dev`'s project file (ADR 0051, named `oma.toml` by ADR 0073): how to build one
//! application and what goes into its Bundle. It is read from `oma.toml` beside the Manifest and is
//! never put in the Bundle; the Manifest, `manifest.toml`, is what ships.
//!
//! `oma` still compiles nothing of its own (ADR 0010, ADR 0032): the commands in `build` are the
//! project's, run as the user, exactly as `make` or `cargo build` would be.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::bundle::{self, Include};
use crate::fmt::Manifest;

/// The project file's name.
pub const FILE: &str = "oma.toml";

/// Its name before ADR 0073.
pub const OLD_FILE: &str = "dev.toml";

/// Keys of the Manifest, which ships in the Bundle, and so are never the project file's.
const MANIFEST_KEYS: &[&str] = &[
    "identifier",
    "name",
    "version",
    "description",
    "icon",
    "categories",
    "publisher",
    "permissions",
    "daemon",
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{} not found: `odk oma dev` needs a {FILE} beside the manifest to know how to build", .0.display())]
    NoFile(PathBuf),
    #[error("{}: the project file is {FILE} now (ADR 0073); rename it", .0.display())]
    OldName(PathBuf),
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
    #[error("`{command}` failed ({status}); nothing was installed or started")]
    Build { command: String, status: String },
    #[error(transparent)]
    Bundle(#[from] bundle::Error),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    /// Commands to run, in order, in the project's directory.
    #[serde(default)]
    build: Vec<Vec<String>>,
    /// The built executable, relative to the project's directory: `SOURCE`, or
    /// `SOURCE=DESTINATION` to say where it goes, which must be the manifest's `executable`.
    executable: String,
    /// The manifest, relative to the project's directory.
    #[serde(default = "manifest_name")]
    manifest: String,
    /// `FILE[=PATH]`, as `odk oma bundle --include` takes them, the files relative to the directory.
    #[serde(default)]
    include: Vec<String>,
}

fn manifest_name() -> String {
    "manifest.toml".to_owned()
}

/// A project to build, bundle, install and run.
#[derive(Debug)]
pub struct Project {
    dir: PathBuf,
    build: Vec<Vec<String>>,
    executable: PathBuf,
    manifest: PathBuf,
    includes: Vec<Include>,
}

impl Project {
    /// Reads `<dir>/oma.toml`. Every path in it is taken from `dir`, never from where `oma` was
    /// started.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let file = dir.join(FILE);
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let old = dir.join(OLD_FILE);
                return Err(if old.is_file() {
                    Error::OldName(old)
                } else {
                    Error::NoFile(file)
                });
            }
            Err(source) => return Err(Error::Io { path: file, source }),
        };
        let invalid = |message: String| Error::Invalid {
            path: file.clone(),
            message,
        };
        // A Manifest's key here is the Manifest in the wrong file: say where it belongs, not
        // only that it is unknown.
        if let Ok(table) = toml::from_str::<toml::Table>(&text)
            && let Some(key) = MANIFEST_KEYS.iter().find(|k| table.contains_key(**k))
        {
            return Err(invalid(format!(
                "`{key}` belongs in the manifest, which goes into the Bundle; {FILE} says how to build it and is never put there"
            )));
        }
        let raw: Raw = toml::from_str(&text).map_err(|e| invalid(e.to_string()))?;
        if let Some(at) = raw
            .build
            .iter()
            .position(|command| command.first().is_none_or(String::is_empty))
        {
            return Err(invalid(format!("build command {} is empty", at + 1)));
        }
        let includes = raw
            .include
            .iter()
            .map(|text| {
                let mut include = Include::parse(text)?;
                include.source = dir.join(&include.source);
                Ok(include)
            })
            .collect::<Result<Vec<_>, bundle::Error>>()?;
        // The program always goes to the manifest's `executable`. `SOURCE=DESTINATION` says so,
        // as an include would, and is refused, before anything is built, when it says otherwise.
        let executable = match raw.executable.split_once('=') {
            None => dir.join(&raw.executable),
            Some((source, destination)) => {
                if source.is_empty() {
                    return Err(invalid("`executable` names no file before `=`".to_owned()));
                }
                let into_bundle = plain(destination).ok_or_else(|| {
                    invalid(format!(
                        "`executable` goes to {destination:?}, which is not a path inside the Bundle"
                    ))
                })?;
                let manifest_path = dir.join(&raw.manifest);
                let text = fs::read_to_string(&manifest_path).map_err(|source| Error::Io {
                    path: manifest_path.clone(),
                    source,
                })?;
                let shipped = Manifest::parse(&text)
                    .map_err(|e| Error::Invalid {
                        path: manifest_path.clone(),
                        message: e.to_string(),
                    })?
                    .executable;
                if plain(&shipped).as_ref() != Some(&into_bundle) {
                    return Err(invalid(format!(
                        "`executable` goes to {destination:?}, and the manifest's `executable` is {shipped:?}: the program is copied to the manifest's path, so they must be the same"
                    )));
                }
                dir.join(source)
            }
        };
        Ok(Self {
            dir: dir.to_owned(),
            build: raw.build,
            executable,
            manifest: dir.join(raw.manifest),
            includes,
        })
    }

    /// Runs the build commands, one after the other, stopping at the first that fails. Their
    /// output goes where `oma`'s does.
    pub fn build(&self) -> Result<(), Error> {
        for command in &self.build {
            let shown = command.join(" ");
            let status = Command::new(&command[0])
                .args(&command[1..])
                .current_dir(&self.dir)
                .status()
                .map_err(|e| Error::Build {
                    command: shown.clone(),
                    status: e.to_string(),
                })?;
            if !status.success() {
                return Err(Error::Build {
                    command: shown,
                    status: status.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Assembles the Bundle in `out_dir` and returns its path.
    pub fn assemble(&self, out_dir: &Path) -> Result<PathBuf, Error> {
        Ok(bundle::assemble_with(
            &self.executable,
            &self.manifest,
            out_dir,
            &self.includes,
        )?)
    }

    /// The Manifest that ships in the Bundle.
    pub fn manifest(&self) -> Result<Manifest, Error> {
        let text = fs::read_to_string(&self.manifest).map_err(|source| Error::Io {
            path: self.manifest.clone(),
            source,
        })?;
        Manifest::parse(&text).map_err(|e| Error::Invalid {
            path: self.manifest.clone(),
            message: e.to_string(),
        })
    }

    /// Writes the Bundle as one file signed with `signing` in `out_dir`, as `odk oma bundle
    /// --sign` would, and returns its path.
    pub fn write_file(&self, out_dir: &Path, signing: &bundle::Signing) -> Result<PathBuf, Error> {
        Ok(bundle::write_file(
            &self.executable,
            &self.manifest,
            out_dir,
            &self.includes,
            signing,
        )?)
    }
}

/// A path inside the Bundle as its plain components, or `None` when it is empty, absolute or
/// climbs out with `..`.
fn plain(path: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(name) => out.push(name),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn project(text: &str) -> (TempDir, Result<Project, Error>) {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(FILE), text).unwrap();
        let loaded = Project::load(dir.path());
        (dir, loaded)
    }

    #[test]
    fn a_directory_without_the_file_is_told_which_file_it_lacks() {
        let dir = TempDir::new().unwrap();
        let e = Project::load(dir.path()).unwrap_err();
        assert!(matches!(e, Error::NoFile(_)));
        assert!(e.to_string().contains("oma.toml"), "{e}");
    }

    #[test]
    fn a_project_with_the_old_name_is_told_to_rename_it() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(OLD_FILE), "executable = \"app\"\n").unwrap();
        let e = Project::load(dir.path()).unwrap_err();
        assert!(matches!(e, Error::OldName(_)), "{e}");
        let said = e.to_string();
        assert!(
            said.contains("dev.toml") && said.contains("oma.toml"),
            "{said}"
        );
    }

    #[test]
    fn a_manifest_key_is_sent_to_the_manifest() {
        let (_dir, loaded) = project("executable = \"app\"\nidentifier = \"dev.example.app\"\n");
        let said = loaded.unwrap_err().to_string();
        assert!(
            said.contains("`identifier` belongs in the manifest"),
            "{said}"
        );
    }

    #[test]
    fn paths_are_the_directorys_and_never_the_current_one() {
        let (dir, loaded) =
            project("executable = \"target/app\"\ninclude = [\"../x/tool=bin/tool\", \"data\"]\n");
        let p = loaded.unwrap();
        assert_eq!(p.executable, dir.path().join("target/app"));
        assert_eq!(p.manifest, dir.path().join("manifest.toml"));
        assert_eq!(p.includes[0].source, dir.path().join("../x/tool"));
        assert_eq!(p.includes[0].destination, PathBuf::from("bin/tool"));
        assert_eq!(p.includes[1].source, dir.path().join("data"));
        assert_eq!(p.includes[1].destination, PathBuf::from("data"));
    }

    #[test]
    fn an_unknown_key_an_empty_command_and_a_bad_include_are_refused() {
        for text in [
            "executable = \"a\"\nbuilds = []\n",
            "executable = \"a\"\nbuild = [[]]\n",
            "executable = \"a\"\nbuild = [[\"\"]]\n",
            "executable = \"a\"\ninclude = [\"f=../out\"]\n",
            "build = []\n",
        ] {
            assert!(project(text).1.is_err(), "{text}");
        }
    }

    fn with_manifest(oma_toml: &str, executable: &str) -> (TempDir, Result<Project, Error>) {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(FILE), oma_toml).unwrap();
        fs::write(
            dir.path().join("manifest.toml"),
            format!(
                "manifest_version = 1\nidentifier = \"dev.example.app\"\nname = \"App\"\nversion = \"1.0.0\"\nexecutable = \"{executable}\"\n\n[permissions]\n"
            ),
        )
        .unwrap();
        let loaded = Project::load(dir.path());
        (dir, loaded)
    }

    #[test]
    fn the_executable_may_say_it_goes_to_the_manifests_path_and_nowhere_else() {
        for shipped in ["bin/app", "./bin/app"] {
            let (dir, loaded) = with_manifest("executable = \"build/app=bin/app\"\n", shipped);
            let p = loaded.unwrap();
            assert_eq!(p.executable, dir.path().join("build/app"), "{shipped}");
        }

        let (_dir, loaded) = with_manifest("executable = \"build/app=bin/app\"\n", "bin/other");
        let said = loaded.unwrap_err().to_string();
        assert!(
            said.contains("\"bin/app\"") && said.contains("\"bin/other\""),
            "{said}"
        );

        for bad in [
            "build/app=../app",
            "build/app=/bin/app",
            "build/app=",
            "=bin/app",
        ] {
            let (_dir, loaded) = with_manifest(&format!("executable = \"{bad}\"\n"), "bin/app");
            assert!(loaded.is_err(), "{bad}");
        }

        // A bare path still loads without reading the manifest, and is copied to its path.
        let (dir, loaded) = project("executable = \"build/app\"\n");
        assert_eq!(loaded.unwrap().executable, dir.path().join("build/app"));
    }

    #[test]
    fn a_mapped_executable_lands_at_the_manifests_path() {
        let (dir, loaded) = with_manifest("executable = \"build/app=bin/app\"\n", "bin/app");
        fs::create_dir_all(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build/app"), crate::testutil::elf()).unwrap();
        let out = TempDir::new().unwrap();
        let bundle = loaded.unwrap().assemble(out.path()).unwrap();
        assert_eq!(
            fs::read(bundle.join("bin/app")).unwrap(),
            crate::testutil::elf()
        );
    }

    #[test]
    fn a_failing_build_command_stops_the_rest() {
        let (dir, loaded) = project(
            "executable = \"a\"\nbuild = [[\"sh\", \"-c\", \"touch one\"], [\"sh\", \"-c\", \"exit 3\"], [\"sh\", \"-c\", \"touch two\"]]\n",
        );
        let e = loaded.unwrap().build().unwrap_err();
        assert!(matches!(e, Error::Build { .. }), "{e}");
        assert!(dir.path().join("one").exists(), "ran in the directory");
        assert!(!dir.path().join("two").exists(), "stopped at the failure");
    }
}
