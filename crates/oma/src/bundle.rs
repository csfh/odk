//! Assembling a Bundle directory from a built executable and a Manifest (ADR 0032).
//!
//! This compiles nothing and knows nothing about how the executable was built.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::fmt::{self, Manifest};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Invalid(#[from] fmt::Error),
    #[error("{} already exists; remove it first", .0.display())]
    Exists(PathBuf),
    #[error("the manifest names the icon {0:?}, but there is no such file beside it")]
    MissingIcon(String),
    #[error("cannot include {0}")]
    BadInclude(String),
}

/// A file to put in the Bundle besides the executable, the manifest and the icon (ADR 0050).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    pub source: PathBuf,
    /// Where it goes, relative to the Bundle.
    pub destination: PathBuf,
}

impl Include {
    /// `SOURCE` or `SOURCE=DESTINATION`. Without a destination the file goes to the Bundle's root
    /// under its own name.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let (source, destination) = match text.split_once('=') {
            Some((source, destination)) => (PathBuf::from(source), PathBuf::from(destination)),
            None => {
                let source = PathBuf::from(text);
                let name = source
                    .file_name()
                    .ok_or_else(|| Error::BadInclude(format!("{text:?}: it names no file")))?;
                let destination = PathBuf::from(name);
                (source, destination)
            }
        };
        let relative_and_plain = !destination.as_os_str().is_empty()
            && destination
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !relative_and_plain {
            return Err(Error::BadInclude(format!(
                "{text:?}: the destination must be a path inside the Bundle, with no `..`"
            )));
        }
        Ok(Self {
            source,
            destination,
        })
    }
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// Creates `<out_dir>/<Name>.oma/` from `manifest_path` and the built `executable`, and returns
/// its path. The Manifest is copied as is, the executable goes to the path the Manifest names
/// and gets the executable bit, and the icon, if declared, is copied from beside the Manifest.
///
/// The result is validated like any Bundle ([`fmt::Bundle::open`]). It is built under a
/// temporary name and moved into place only once valid, so a failure leaves nothing behind.
/// An existing `<Name>.oma` is never replaced.
pub fn assemble(executable: &Path, manifest_path: &Path, out_dir: &Path) -> Result<PathBuf, Error> {
    assemble_with(executable, manifest_path, out_dir, &[])
}

/// [`assemble`], with files to include besides (ADR 0050): each a regular file, not a symlink,
/// copied to its destination with the executable bit kept, never over the manifest, the
/// executable, the icon or another include.
pub fn assemble_with(
    executable: &Path,
    manifest_path: &Path,
    out_dir: &Path,
    includes: &[Include],
) -> Result<PathBuf, Error> {
    let manifest_text = fs::read_to_string(manifest_path).map_err(|e| io(manifest_path, e))?;
    let manifest = Manifest::parse(&manifest_text)?;

    fs::create_dir_all(out_dir).map_err(|e| io(out_dir, e))?;
    let name = format!("{}.oma", manifest.name.replace('/', "-"));
    let target = out_dir.join(&name);
    if target.exists() {
        return Err(Error::Exists(target));
    }
    // Named by process so concurrent runs cannot touch each other's directory; `create_dir`
    // fails rather than reuse one that exists, and only a directory this run made is removed.
    let staging = out_dir.join(format!(".{name}.partial-{}", std::process::id()));
    fs::create_dir(&staging).map_err(|e| io(&staging, e))?;

    let result = fill(
        &staging,
        executable,
        manifest_path,
        &manifest_text,
        &manifest,
        includes,
    )
    .and_then(|()| fmt::Bundle::open(&staging).map(drop).map_err(Error::from))
    .and_then(|()| fs::rename(&staging, &target).map_err(|e| io(&target, e)));
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    Ok(target)
}

fn fill(
    staging: &Path,
    executable: &Path,
    manifest_path: &Path,
    manifest_text: &str,
    manifest: &Manifest,
    includes: &[Include],
) -> Result<(), Error> {
    let manifest_out = staging.join("manifest.toml");
    fs::write(&manifest_out, manifest_text).map_err(|e| io(&manifest_out, e))?;

    let exe_out = staging.join(&manifest.executable);
    copy_into(executable, &exe_out)?;
    fs::set_permissions(&exe_out, fs::Permissions::from_mode(0o755))
        .map_err(|e| io(&exe_out, e))?;

    if let Some(icon) = &manifest.icon {
        let source = manifest_path.parent().unwrap_or(Path::new(".")).join(icon);
        if !source.is_file() {
            return Err(Error::MissingIcon(icon.clone()));
        }
        copy_into(&source, &staging.join(icon))?;
    }

    for include in includes {
        let name = include.destination.display();
        let meta = fs::symlink_metadata(&include.source).map_err(|e| io(&include.source, e))?;
        if !meta.is_file() {
            return Err(Error::BadInclude(format!(
                "{}: it is not a regular file (a symlink or a directory is not copied)",
                include.source.display()
            )));
        }
        let out = staging.join(&include.destination);
        if out.symlink_metadata().is_ok() {
            return Err(Error::BadInclude(format!(
                "{name}: the Bundle already has a file there"
            )));
        }
        copy_into(&include.source, &out)?;
        let mode = if meta.permissions().mode() & 0o111 != 0 {
            0o755
        } else {
            0o644
        };
        fs::set_permissions(&out, fs::Permissions::from_mode(mode)).map_err(|e| io(&out, e))?;
    }
    Ok(())
}

fn copy_into(source: &Path, destination: &Path) -> Result<(), Error> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    fs::copy(source, destination).map_err(|e| io(source, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    use crate::testutil::elf;

    fn manifest(extra: &str, executable: &str) -> String {
        format!(
            "manifest_version = 1\nidentifier = \"com.example.t\"\nname = \"Tee\"\nversion = \"1.0.0\"\nexecutable = \"{executable}\"\n{extra}\n[permissions]\n"
        )
    }

    /// A project directory holding a manifest and a built executable.
    struct Project {
        dir: TempDir,
    }

    impl Project {
        fn new(extra: &str, executable: &str) -> Self {
            let dir = TempDir::new().unwrap();
            fs::write(
                dir.path().join("manifest.toml"),
                manifest(extra, executable),
            )
            .unwrap();
            fs::write(dir.path().join("built"), elf()).unwrap();
            Self { dir }
        }

        fn assemble(&self, out: &Path) -> Result<PathBuf, Error> {
            assemble(
                &self.dir.path().join("built"),
                &self.dir.path().join("manifest.toml"),
                out,
            )
        }
    }

    #[test]
    fn assembles_a_valid_bundle_named_after_the_manifest() {
        let project = Project::new("icon = \"share/t.svg\"", "bin/tee");
        fs::create_dir(project.dir.path().join("share")).unwrap();
        fs::write(project.dir.path().join("share/t.svg"), "<svg/>").unwrap();
        let out = TempDir::new().unwrap();

        let bundle = project.assemble(out.path()).unwrap();

        assert_eq!(bundle, out.path().join("Tee.oma"));
        let opened = fmt::Bundle::open(&bundle).unwrap();
        assert!(opened.executable().ends_with("bin/tee"));
        assert_eq!(
            fs::metadata(opened.executable())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert!(bundle.join("share/t.svg").is_file());
        assert_eq!(
            fs::read_dir(out.path()).unwrap().count(),
            1,
            "no staging directory left"
        );
    }

    #[test]
    fn creates_the_output_directory_and_replaces_slashes_in_the_name() {
        let project = Project::new("", "tee");
        let text = manifest("", "tee").replace("name = \"Tee\"", "name = \"a/b\"");
        fs::write(project.dir.path().join("manifest.toml"), text).unwrap();
        let out = TempDir::new().unwrap();
        let bundle = project.assemble(&out.path().join("new/dir")).unwrap();
        assert_eq!(bundle, out.path().join("new/dir/a-b.oma"));
    }

    #[test]
    fn never_replaces_an_existing_bundle() {
        let project = Project::new("", "tee");
        let out = TempDir::new().unwrap();
        project.assemble(out.path()).unwrap();
        assert!(matches!(
            project.assemble(out.path()),
            Err(Error::Exists(_))
        ));
    }

    #[test]
    fn a_failure_leaves_nothing_behind() {
        let out = TempDir::new().unwrap();

        let not_elf = Project::new("", "tee");
        fs::write(not_elf.dir.path().join("built"), "#!/bin/sh\n").unwrap();
        assert!(matches!(
            not_elf.assemble(out.path()),
            Err(Error::Invalid(_))
        ));

        let no_icon = Project::new("icon = \"missing.png\"", "tee");
        assert!(matches!(
            no_icon.assemble(out.path()),
            Err(Error::MissingIcon(_))
        ));

        let overwrites_manifest = Project::new("", "manifest.toml");
        assert!(overwrites_manifest.assemble(out.path()).is_err());

        let bad_manifest = Project::new("", "tee");
        fs::write(
            bad_manifest.dir.path().join("manifest.toml"),
            "manifest_version = 2\n",
        )
        .unwrap();
        assert!(matches!(
            bad_manifest.assemble(out.path()),
            Err(Error::Invalid(_))
        ));

        assert_eq!(
            fs::read_dir(out.path()).unwrap().count(),
            0,
            "{:?}",
            fs::read_dir(out.path()).unwrap().collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_include_parses_with_or_without_a_destination() {
        assert_eq!(
            Include::parse("target/release/ratmos=bin/ratmos").unwrap(),
            Include {
                source: "target/release/ratmos".into(),
                destination: "bin/ratmos".into()
            }
        );
        assert_eq!(
            Include::parse("a/b/tool").unwrap().destination,
            PathBuf::from("tool")
        );
        for bad in ["x=../up", "x=/abs", "x=", "x=a/../b", "/"] {
            assert!(
                matches!(Include::parse(bad), Err(Error::BadInclude(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn included_files_keep_their_executable_bit_and_nothing_else() {
        let project = Project::new("", "bin/tee");
        let helper = project.dir.path().join("helper");
        fs::write(&helper, elf()).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let data = project.dir.path().join("data.txt");
        fs::write(&data, "x").unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o666)).unwrap();
        let out = TempDir::new().unwrap();
        let bundle = assemble_with(
            &project.dir.path().join("built"),
            &project.dir.path().join("manifest.toml"),
            out.path(),
            &[
                Include {
                    source: helper,
                    destination: "bin/helper".into(),
                },
                Include {
                    source: data,
                    destination: "share/data.txt".into(),
                },
            ],
        )
        .unwrap();
        let mode = |p: &str| fs::metadata(bundle.join(p)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("bin/helper"), 0o755);
        assert_eq!(mode("share/data.txt"), 0o644);
    }

    #[test]
    fn an_include_never_replaces_a_file_the_bundle_has_and_is_never_a_symlink() {
        let project = Project::new("", "bin/tee");
        let out = TempDir::new().unwrap();
        let build = |includes: &[Include]| {
            assemble_with(
                &project.dir.path().join("built"),
                &project.dir.path().join("manifest.toml"),
                out.path(),
                includes,
            )
        };
        let file = project.dir.path().join("f");
        fs::write(&file, "x").unwrap();
        let at = |destination: &str| Include {
            source: file.clone(),
            destination: destination.into(),
        };
        for taken in ["manifest.toml", "bin/tee"] {
            assert!(
                matches!(build(&[at(taken)]), Err(Error::BadInclude(_))),
                "{taken}"
            );
        }
        assert!(matches!(
            build(&[at("same"), at("same")]),
            Err(Error::BadInclude(_))
        ));

        let link = project.dir.path().join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(matches!(
            build(&[Include {
                source: link,
                destination: "l".into()
            }]),
            Err(Error::BadInclude(_))
        ));
        assert!(matches!(
            build(&[Include {
                source: project.dir.path().to_owned(),
                destination: "d".into()
            }]),
            Err(Error::BadInclude(_))
        ));
        assert_eq!(
            fs::read_dir(out.path()).unwrap().count(),
            0,
            "a refused include leaves nothing behind"
        );
    }

    #[test]
    fn a_missing_executable_is_an_error() {
        let project = Project::new("", "tee");
        let out = TempDir::new().unwrap();
        let result = assemble(
            &project.dir.path().join("nope"),
            &project.dir.path().join("manifest.toml"),
            out.path(),
        );
        assert!(matches!(result, Err(Error::Io { .. })));
        assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0);
    }
}
