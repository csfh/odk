//! A Bundle file's image (ADR 0067): the EROFS tree behind the prefix, read by `odk-oma` itself.
//!
//! It is read only through a [`Checked`] file, so only after the prefix, the signature and both
//! digests have held (ADR 0060, section 6), and only from the sealed copy they held for (section
//! 5). It accepts only the layouts `docs/spec/oma-image.md` lists, and anything else fails closed
//! (section 9).
//!
//! [`Image::read`] checks the whole tree before it returns: every inode, every directory entry,
//! and that no two things claim the same bytes, so the files an image unpacks to never add up to
//! more than the file. Then it reads `manifest.toml`, holds it to the signed manifest (`app_id`,
//! `entry` and `publisher`, section 4), and checks its programs and icon as
//! [`crate::fmt::Bundle::open`] checks a directory's.

mod erofs;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;
mod writer;

pub use writer::{WriteError, write};

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use erofs::{BLOCK, Data, Dirent, FT_DIR, FT_REG, META_START};

use crate::fmt::{self, ELF_HEADER_CHECKED, Manifest, check_elf_header};
use crate::trust::{Checked, Sealed, SignedManifest};

// The limits of a Bundle's tree (`docs/spec/oma-image.md`, "The tree"). The writer holds a tree
// to the same ones, so `odk oma bundle` never makes a file `odk-oma` cannot read.
pub(crate) const MAX_NODES: usize = 65_536;
pub(crate) const MAX_DEPTH: usize = 64;
pub(crate) const MAX_PATH: usize = 4095;
pub(crate) const MAX_DIRECTORY: u64 = 4 << 20;
const MAX_MANIFEST: u64 = 64 << 10;
const MANIFEST_FILE: &str = "manifest.toml";

/// Why an image was not accepted. Every variant fails closed (ADR 0060, section 9).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the image is not EROFS: there is no superblock at byte 1024")]
    NotErofs,
    #[error("the image's {field} is {value:#x}, and a Bundle's image has {expected}")]
    Superblock {
        field: &'static str,
        value: u64,
        expected: &'static str,
    },
    #[error("{} is compressed; a Bundle's files are stored as they are", show(.path))]
    Compressed { path: PathBuf },
    #[error("{} is stored in layout {layout:#x}, which a Bundle's image does not use", show(.path))]
    Layout { path: PathBuf, layout: u16 },
    #[error("{} has an extended inode, which a Bundle's image does not use", show(.path))]
    ExtendedInode { path: PathBuf },
    #[error("{}'s inline data does not fit in its block", show(.path))]
    Inline { path: PathBuf },
    #[error("{} has extended attributes", show(.path))]
    Xattr { path: PathBuf },
    #[error("{} is a device node", show(.path))]
    Device { path: PathBuf },
    #[error("{} is a fifo", show(.path))]
    Fifo { path: PathBuf },
    #[error("{} is a socket", show(.path))]
    Socket { path: PathBuf },
    #[error("{} is a symbolic link", show(.path))]
    Symlink { path: PathBuf },
    #[error("{} is setuid or setgid", show(.path))]
    SetId { path: PathBuf },
    #[error("{} has mode {mode:o}, which is neither a directory nor a regular file", show(.path))]
    Mode { path: PathBuf, mode: u16 },
    #[error("{} reaches past the end of the image", show(.path))]
    PastPayload { path: PathBuf },
    #[error("{} shares bytes with the prefix or with another part of the image", show(.path))]
    Overlap { path: PathBuf },
    #[error("{} is in the tree twice", show(.path))]
    Linked { path: PathBuf },
    #[error("{}: {reason}", show(.path))]
    Directory { path: PathBuf, reason: &'static str },
    #[error("the tree has {0}")]
    Limit(&'static str),
    #[error("the image has no manifest.toml")]
    NoManifest,
    #[error("manifest.toml is not a UTF-8 file of at most 64 KiB")]
    ManifestFile,
    #[error(transparent)]
    Manifest(#[from] fmt::Error),
    #[error("manifest.toml's {field} is {image}, and the signed manifest's is {signed}")]
    Mismatch {
        field: &'static str,
        image: String,
        signed: String,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn show(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        "the root directory".to_owned()
    } else {
        path.display().to_string()
    }
}

/// One directory or regular file of an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Relative to the image's root, which is the empty path.
    pub path: PathBuf,
    pub kind: Kind,
    data: Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File { executable: bool, size: u64 },
}

/// An image whose whole tree and Manifest have been checked.
#[derive(Debug)]
pub struct Image<'a> {
    file: &'a Sealed,
    manifest: Manifest,
    nodes: Vec<Node>,
}

impl<'a> Image<'a> {
    /// Reads and checks the image of a file whose envelope has held.
    pub fn read(checked: &Checked<'a>) -> Result<Self, Error> {
        let file = checked.file();
        let nodes = walk(file)?;
        let manifest = Tree {
            file,
            nodes: &nodes,
        }
        .check_manifest(checked.signed().manifest())?;
        Ok(Self {
            file,
            manifest,
            nodes,
        })
    }

    /// The image's `manifest.toml`, which matches the signed manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Every directory and file, each directory before what it holds; the root is first.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The bytes a tmpfs needs for the image's files: each file's size rounded up to a page, and
    /// never zero, which would be a tmpfs without a limit.
    pub fn unpacked_size(&self) -> u64 {
        let pages: u64 = self
            .nodes
            .iter()
            .map(|n| match n.kind {
                Kind::File { size, .. } => size.div_ceil(BLOCK) * BLOCK,
                Kind::Dir => 0,
            })
            .sum();
        pages.max(BLOCK)
    }

    /// Writes the tree into `dir`, an empty directory: directories `0755`, files `0755` when the
    /// image marks them executable and `0644` when not. Nothing it writes may already exist.
    pub fn unpack(&self, dir: &Path) -> io::Result<()> {
        for node in &self.nodes[1..] {
            let target = dir.join(&node.path);
            match node.kind {
                Kind::Dir => {
                    fs::DirBuilder::new().mode(0o755).create(&target)?;
                    fs::set_permissions(&target, Permissions::from_mode(0o755))?;
                }
                Kind::File { executable, .. } => {
                    let mode = if executable { 0o755 } else { 0o644 };
                    let mut out = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(mode)
                        .open(&target)?;
                    out.set_permissions(Permissions::from_mode(mode))?;
                    contents(self.file, &node.data, |bytes| out.write_all(bytes))?;
                }
            }
        }
        Ok(())
    }
}

/// The checked tree, before its Manifest is.
struct Tree<'t> {
    file: &'t Sealed,
    nodes: &'t [Node],
}

impl Tree<'_> {
    /// The node at a Manifest path such as `bin/app` or `./bin/app`.
    fn find(&self, relative: &str) -> Option<&Node> {
        let wanted: PathBuf = Path::new(relative)
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .collect();
        self.nodes.iter().find(|n| n.path == wanted)
    }

    fn check_manifest(&self, signed: &SignedManifest) -> Result<Manifest, Error> {
        let node = self.find(MANIFEST_FILE).ok_or(Error::NoManifest)?;
        let Kind::File { size, .. } = node.kind else {
            return Err(Error::ManifestFile);
        };
        if size > MAX_MANIFEST {
            return Err(Error::ManifestFile);
        }
        let mut bytes = Vec::with_capacity(size as usize);
        contents(self.file, &node.data, |b| {
            bytes.extend_from_slice(b);
            Ok(())
        })?;
        let text = String::from_utf8(bytes).map_err(|_| Error::ManifestFile)?;
        let manifest = Manifest::parse(&text)?;

        let mismatch = |field, image: &dyn std::fmt::Display, signed: &dyn std::fmt::Display| {
            Error::Mismatch {
                field,
                image: image.to_string(),
                signed: signed.to_string(),
            }
        };
        if manifest.identifier != signed.app_id {
            return Err(mismatch("identifier", &manifest.identifier, &signed.app_id));
        }
        if manifest.executable != signed.entry {
            return Err(mismatch("executable", &manifest.executable, &signed.entry));
        }
        match &manifest.publisher {
            Some(publisher) if *publisher == signed.publisher => {}
            Some(publisher) => return Err(mismatch("publisher", publisher, &signed.publisher)),
            None => return Err(mismatch("publisher", &"missing", &signed.publisher)),
        }

        self.check_program("executable", &manifest.executable)?;
        if let Some(daemon) = &manifest.permissions.daemon {
            self.check_program("daemon.executable", &daemon.executable)?;
        }
        if let Some(icon) = &manifest.icon {
            self.regular_file("icon", icon)?;
            let extension = Path::new(icon)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(extension.as_deref(), Some("png" | "svg")) {
                return Err(path_error("icon", icon, "must be a PNG or SVG file"));
            }
        }
        Ok(manifest)
    }

    fn regular_file(&self, field: &'static str, relative: &str) -> Result<&Node, Error> {
        let node = self
            .find(relative)
            .ok_or_else(|| path_error(field, relative, "does not exist"))?;
        match node.kind {
            Kind::File { .. } => Ok(node),
            Kind::Dir => Err(path_error(field, relative, "is not a regular file")),
        }
    }

    /// The checks a directory Bundle's programs pass (`fmt::Bundle::open`): a regular file with
    /// the executable bit, and an x86_64 ELF executable.
    fn check_program(&self, field: &'static str, relative: &str) -> Result<(), Error> {
        let node = self.regular_file(field, relative)?;
        let Kind::File { executable, size } = node.kind else {
            unreachable!("regular_file returns files");
        };
        if !executable {
            return Err(path_error(
                field,
                relative,
                "does not have the executable bit set",
            ));
        }
        if size < ELF_HEADER_CHECKED as u64 {
            return Err(path_error(field, relative, "is not an ELF file"));
        }
        let mut header = [0u8; ELF_HEADER_CHECKED];
        let mut filled = 0;
        contents(self.file, &node.data, |b| {
            let n = b.len().min(header.len() - filled);
            header[filled..filled + n].copy_from_slice(&b[..n]);
            filled += n;
            Ok(())
        })?;
        check_elf_header(&header, field, relative).map_err(Error::Manifest)
    }
}

fn path_error(field: &'static str, path: &str, reason: &'static str) -> Error {
    Error::Manifest(fmt::Error::Path {
        field,
        path: path.to_owned(),
        reason,
    })
}

/// Calls `each` with the bytes of a file or directory, in order, a piece at a time.
fn contents(
    file: &Sealed,
    data: &Data,
    mut each: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let mut buf = vec![0u8; (data.size as usize).clamp(1, 1 << 16)];
    for (mut at, mut len) in data.ranges() {
        while len > 0 {
            let n = len.min(buf.len() as u64) as usize;
            file.read_exact_at(&mut buf[..n], at)?;
            each(&buf[..n])?;
            at += n as u64;
            len -= n as u64;
        }
    }
    Ok(())
}

/// The byte ranges of the image taken so far, by first byte. Each inode's are taken as soon as
/// the inode is read and before any of its data is, so a range that meets one already taken is
/// refused before it is read, and the walk reads no byte twice.
struct Claims(BTreeMap<u64, u64>);

impl Claims {
    /// The prefix and the superblock are taken from the start.
    fn new() -> Self {
        Self(BTreeMap::from([(0, META_START)]))
    }

    fn take(&mut self, inode: &erofs::Inode, path: &Path) -> Result<(), Error> {
        for (start, len) in erofs::claims(inode) {
            if len == 0 {
                continue;
            }
            let end = start + len;
            let before = self.0.range(..=start).next_back();
            let after = self.0.range(start..).next();
            if before.is_some_and(|(_, &e)| e > start) || after.is_some_and(|(&s, _)| s < end) {
                return Err(Error::Overlap {
                    path: path.to_owned(),
                });
            }
            self.0.insert(start, end);
        }
        Ok(())
    }
}

/// Every node of the tree, each directory before what it holds, after checking each inode and
/// directory entry and that nothing in the image overlaps anything else.
fn walk(file: &Sealed) -> Result<Vec<Node>, Error> {
    let sb = erofs::superblock(file)?;
    let root_path = PathBuf::new();
    let root = erofs::inode(file, &sb, sb.root_nid, &root_path)?;
    if !root.is_dir() {
        return Err(Error::Directory {
            path: root_path,
            reason: "the root is not a directory",
        });
    }
    let mut seen = HashSet::from([sb.root_nid]);
    let mut claims = Claims::new();
    claims.take(&root, &root_path)?;
    let mut nodes = Vec::new();
    // (nid, its parent's nid, inode, path, depth)
    let mut stack = vec![(sb.root_nid, sb.root_nid, root, root_path, 0usize)];
    while let Some((nid, parent, inode, path, depth)) = stack.pop() {
        if !inode.is_dir() {
            nodes.push(Node {
                path,
                kind: Kind::File {
                    executable: inode.mode & 0o111 != 0,
                    size: inode.data.size,
                },
                data: inode.data,
            });
            continue;
        }
        let entries = directory(file, &inode.data, &path)?;
        check_dots(&entries, nid, parent, &path)?;
        let mut children = Vec::new();
        for entry in entries {
            if entry.name == b"." || entry.name == b".." {
                continue;
            }
            let child = erofs::child(&path, &entry.name);
            if depth + 1 > MAX_DEPTH {
                return Err(Error::Limit("an entry nested more than 64 deep"));
            }
            if child.as_os_str().len() > MAX_PATH {
                return Err(Error::Limit("a path longer than 4095 bytes"));
            }
            if !matches!(entry.file_type, FT_REG | FT_DIR) {
                return Err(erofs::file_type_error(entry.file_type, child));
            }
            if !seen.insert(entry.nid) {
                return Err(Error::Linked { path: child });
            }
            let child_inode = erofs::inode(file, &sb, entry.nid, &child)?;
            claims.take(&child_inode, &child)?;
            if child_inode.is_dir() != (entry.file_type == FT_DIR) {
                return Err(Error::Directory {
                    path: child,
                    reason: "an entry's type is not its inode's",
                });
            }
            children.push((entry.nid, nid, child_inode, child, depth + 1));
        }
        nodes.push(Node {
            path,
            kind: Kind::Dir,
            data: inode.data,
        });
        if nodes.len() + stack.len() + children.len() > MAX_NODES {
            return Err(Error::Limit("more than 65536 entries"));
        }
        stack.extend(children.into_iter().rev());
    }
    Ok(nodes)
}

/// A directory's entries, block by block, in the strictly ascending byte order the kernel's
/// lookup assumes.
fn directory(file: &Sealed, data: &Data, path: &Path) -> Result<Vec<Dirent>, Error> {
    if data.size > MAX_DIRECTORY {
        return Err(Error::Limit("a directory larger than 4 MiB"));
    }
    let mut bytes = Vec::with_capacity(data.size as usize);
    contents(file, data, |b| {
        bytes.extend_from_slice(b);
        Ok(())
    })?;
    if bytes.is_empty() {
        return Err(Error::Directory {
            path: path.to_owned(),
            reason: "it is empty, without even '.' and '..'",
        });
    }
    let mut entries = Vec::new();
    for chunk in bytes.chunks(BLOCK as usize) {
        entries.extend(erofs::dirents(chunk, path)?);
    }
    if entries.windows(2).any(|w| w[0].name >= w[1].name) {
        return Err(Error::Directory {
            path: path.to_owned(),
            reason: "its names are not in order",
        });
    }
    Ok(entries)
}

/// `.` is the directory itself and `..` its parent; the root is its own parent.
fn check_dots(entries: &[Dirent], nid: u64, parent: u64, path: &Path) -> Result<(), Error> {
    for (name, wanted) in [(&b"."[..], nid), (&b".."[..], parent)] {
        let found = entries.iter().find(|e| e.name == name);
        if !found.is_some_and(|e| e.nid == wanted && e.file_type == FT_DIR) {
            return Err(Error::Directory {
                path: path.to_owned(),
                reason: "'.' or '..' is missing or names the wrong directory",
            });
        }
    }
    Ok(())
}
