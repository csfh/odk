//! `odk oma dev`'s project file (ADR 0051): how to build one application and what goes into its
//! Bundle. It is read from `dev.toml` beside the Manifest and is never put in the Bundle.
//!
//! `oma` still compiles nothing of its own (ADR 0010, ADR 0032): the commands in `build` are the
//! project's, run as the user, exactly as `make` or `cargo build` would be.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::bundle::{self, Include};

/// The project file's name.
pub const FILE: &str = "dev.toml";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{} not found: `odk oma dev` needs a {FILE} beside the manifest to know how to build", .0.display())]
    NoFile(PathBuf),
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
    /// The built executable, relative to the project's directory.
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
    /// Reads `<dir>/dev.toml`. Every path in it is taken from `dir`, never from where `oma` was
    /// started.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let file = dir.join(FILE);
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::NoFile(file)),
            Err(source) => return Err(Error::Io { path: file, source }),
        };
        let invalid = |message: String| Error::Invalid {
            path: file.clone(),
            message,
        };
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
        Ok(Self {
            dir: dir.to_owned(),
            build: raw.build,
            executable: dir.join(raw.executable),
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
        assert!(e.to_string().contains("dev.toml"), "{e}");
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
