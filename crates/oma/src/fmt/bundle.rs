use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::fmt::{Error, Manifest};

const MANIFEST_FILE: &str = "manifest.toml";
const ELF_MAGIC: [u8; 4] = *b"\x7fELF";
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;

/// A Bundle directory whose Manifest and files have passed validation (ADR 0022).
#[derive(Debug, Clone)]
pub struct Bundle {
    root: PathBuf,
    manifest: Manifest,
    executable: PathBuf,
}

impl Bundle {
    /// Reads and validates the Bundle at `dir`, following `docs/spec/manifest.md`.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let root = fs::canonicalize(dir).map_err(|e| io(dir, e))?;
        let manifest_path = root.join(MANIFEST_FILE);
        let text = fs::read_to_string(&manifest_path).map_err(|e| io(&manifest_path, e))?;
        let manifest = Manifest::parse(&text)?;

        let executable = contained_file(&root, "executable", &manifest.executable)?;
        let mode = fs::metadata(&executable)
            .map_err(|e| io(&executable, e))?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err(path_error(
                "executable",
                &manifest.executable,
                "does not have the executable bit set",
            ));
        }
        check_x86_64_elf(&executable, &manifest.executable)?;

        if let Some(icon) = &manifest.icon {
            let resolved = contained_file(&root, "icon", icon)?;
            let extension = resolved
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(extension.as_deref(), Some("png" | "svg")) {
                return Err(path_error("icon", icon, "must be a PNG or SVG file"));
            }
        }

        Ok(Self {
            root,
            manifest,
            executable,
        })
    }

    /// The Bundle directory, with symlinks resolved.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The real path of the executable, inside the Bundle.
    pub fn executable(&self) -> &Path {
        &self.executable
    }
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

fn path_error(field: &'static str, path: &str, reason: &'static str) -> Error {
    Error::Path {
        field,
        path: path.to_owned(),
        reason,
    }
}

/// Resolves `relative` under `root`, following symlinks, and requires a regular file that is
/// still inside `root`.
fn contained_file(root: &Path, field: &'static str, relative: &str) -> Result<PathBuf, Error> {
    let resolved = fs::canonicalize(root.join(relative))
        .map_err(|_| path_error(field, relative, "does not exist"))?;
    if !resolved.starts_with(root) {
        return Err(path_error(field, relative, "resolves outside the Bundle"));
    }
    let metadata = fs::metadata(&resolved).map_err(|e| io(&resolved, e))?;
    if !metadata.is_file() {
        return Err(path_error(field, relative, "is not a regular file"));
    }
    Ok(resolved)
}

/// Checks the ELF header of a 64-bit little-endian x86-64 executable or position-independent
/// executable (ADR 0002). Nothing else about the file is inspected.
fn check_x86_64_elf(path: &Path, relative: &str) -> Result<(), Error> {
    let mut header = [0u8; 20];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|_| path_error("executable", relative, "is not an ELF file"))?;
    if header[..4] != ELF_MAGIC {
        return Err(path_error("executable", relative, "is not an ELF file"));
    }
    if header[4] != ELFCLASS64 || header[5] != ELFDATA2LSB {
        return Err(path_error(
            "executable",
            relative,
            "is not a 64-bit little-endian ELF file",
        ));
    }
    let file_type = u16::from_le_bytes([header[16], header[17]]);
    let machine = u16::from_le_bytes([header[18], header[19]]);
    if machine != EM_X86_64 {
        return Err(path_error(
            "executable",
            relative,
            "is not built for x86_64",
        ));
    }
    if file_type != ET_EXEC && file_type != ET_DYN {
        return Err(path_error(
            "executable",
            relative,
            "is not an executable ELF file",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    use super::*;

    fn elf(machine: u16) -> Vec<u8> {
        let mut h = vec![0u8; 64];
        h[..4].copy_from_slice(&ELF_MAGIC);
        h[4] = ELFCLASS64;
        h[5] = ELFDATA2LSB;
        h[16..18].copy_from_slice(&ET_DYN.to_le_bytes());
        h[18..20].copy_from_slice(&machine.to_le_bytes());
        h
    }

    fn manifest(extra: &str) -> String {
        format!(
            "manifest_version = 1\nidentifier = \"com.example.t\"\nname = \"T\"\nversion = \"1.0.0\"\nexecutable = \"bin/t\"\n{extra}\n[permissions]\n"
        )
    }

    /// A valid Bundle in a fresh temporary directory; tests then break one thing.
    fn valid() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(MANIFEST_FILE), manifest("")).unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();
        write_exe(&dir.path().join("bin/t"), &elf(EM_X86_64), 0o755);
        dir
    }

    fn write_exe(path: &Path, bytes: &[u8], mode: u32) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn rejects(dir: &TempDir, needle: &str) {
        let err = Bundle::open(dir.path()).unwrap_err().to_string();
        assert!(err.contains(needle), "{err:?} does not contain {needle:?}");
    }

    #[test]
    fn opens_a_valid_bundle() {
        let dir = valid();
        let bundle = Bundle::open(dir.path()).unwrap();
        assert_eq!(bundle.manifest().name, "T");
        assert!(bundle.executable().ends_with("bin/t"));
    }

    #[test]
    fn opens_through_a_symlink_to_the_bundle() {
        let dir = valid();
        let other = TempDir::new().unwrap();
        let link = other.path().join("T.oma");
        symlink(dir.path(), &link).unwrap();
        assert!(Bundle::open(&link).is_ok());
    }

    #[test]
    fn rejects_a_missing_manifest() {
        let dir = valid();
        fs::remove_file(dir.path().join(MANIFEST_FILE)).unwrap();
        rejects(&dir, "manifest.toml");
    }

    #[test]
    fn rejects_a_missing_executable() {
        let dir = valid();
        fs::remove_file(dir.path().join("bin/t")).unwrap();
        rejects(&dir, "does not exist");
    }

    #[test]
    fn rejects_an_executable_without_the_executable_bit() {
        let dir = valid();
        write_exe(&dir.path().join("bin/t"), &elf(EM_X86_64), 0o644);
        rejects(&dir, "executable bit");
    }

    #[test]
    fn rejects_a_directory_as_executable() {
        let dir = valid();
        fs::remove_file(dir.path().join("bin/t")).unwrap();
        fs::create_dir(dir.path().join("bin/t")).unwrap();
        rejects(&dir, "not a regular file");
    }

    #[test]
    fn rejects_files_that_are_not_x86_64_elf() {
        for (bytes, needle) in [
            (b"#!/bin/sh\nexit 0\n".to_vec(), "not an ELF"),
            (b"\x7fE".to_vec(), "not an ELF"),
            (elf(183), "not built for x86_64"),
            (
                {
                    let mut h = elf(EM_X86_64);
                    h[4] = 1;
                    h
                },
                "64-bit",
            ),
            (
                {
                    let mut h = elf(EM_X86_64);
                    h[5] = 2;
                    h
                },
                "little-endian",
            ),
            (
                {
                    let mut h = elf(EM_X86_64);
                    h[16] = 1;
                    h
                },
                "not an executable ELF",
            ),
        ] {
            let dir = valid();
            write_exe(&dir.path().join("bin/t"), &bytes, 0o755);
            rejects(&dir, needle);
        }
    }

    #[test]
    fn rejects_a_symlink_that_leaves_the_bundle() {
        let dir = valid();
        let outside = TempDir::new().unwrap();
        write_exe(&outside.path().join("real"), &elf(EM_X86_64), 0o755);
        fs::remove_file(dir.path().join("bin/t")).unwrap();
        symlink(outside.path().join("real"), dir.path().join("bin/t")).unwrap();
        rejects(&dir, "outside the Bundle");
    }

    #[test]
    fn accepts_a_symlink_that_stays_inside() {
        let dir = valid();
        fs::rename(dir.path().join("bin/t"), dir.path().join("bin/real")).unwrap();
        symlink("real", dir.path().join("bin/t")).unwrap();
        assert!(Bundle::open(dir.path()).is_ok());
    }

    #[test]
    fn checks_the_icon() {
        let dir = valid();
        fs::write(dir.path().join(MANIFEST_FILE), manifest("icon = \"t.svg\"")).unwrap();
        rejects(&dir, "does not exist");
        fs::write(dir.path().join("t.svg"), "<svg/>").unwrap();
        assert!(Bundle::open(dir.path()).is_ok());

        fs::write(dir.path().join(MANIFEST_FILE), manifest("icon = \"t.txt\"")).unwrap();
        fs::write(dir.path().join("t.txt"), "x").unwrap();
        rejects(&dir, "PNG or SVG");

        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("o.png"), "x").unwrap();
        symlink(outside.path().join("o.png"), dir.path().join("o.png")).unwrap();
        fs::write(dir.path().join(MANIFEST_FILE), manifest("icon = \"o.png\"")).unwrap();
        rejects(&dir, "outside the Bundle");
    }
}
