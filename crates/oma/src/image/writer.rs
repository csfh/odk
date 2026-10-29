//! The writer of a Bundle's image (ADR 0067): a directory tree written as the EROFS image
//! `docs/spec/oma-image.md` says the writer emits. No program is run, and the same tree always
//! gives the same bytes: every inode is compact, owned by root, with mtime 0; the UUID is zero;
//! each directory is inline after its inode when it fits there and in whole blocks when it does
//! not; each file is in whole blocks. Directories are `0755`, and a file is `0755` when any
//! executable bit is set and `0644` when none is; no other mode bit is kept.
//!
//! A tree the reader would refuse is refused here first, against the same limits, so what this
//! writes `odk-oma` can read. A symlink to a regular file inside the tree is written as that
//! file; any other symlink, and anything but a directory or a regular file, is refused. Each
//! path is its own inode, so a hard link in the tree is written twice.
//!
//! The first 1024 bytes are zero: they are the prefix's (ADR 0060), which the signer writes.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::erofs::{
    BLOCK, BLOCK_BITS, DIRENT_LEN, FT_DIR, FT_REG, INODE_LEN, LAYOUT_INLINE, LAYOUT_PLAIN,
    MAGIC_BYTES, META_START, NAME_MAX, S_IFDIR, S_IFREG, SUPER_OFFSET,
};
use super::{MAX_DEPTH, MAX_DIRECTORY, MAX_NODES, MAX_PATH};

/// Why a tree was not written.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("{}: {source}", .path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{}: a {kind} is not put in a Bundle; only directories and regular files are", .path.display())]
    NotAllowed { path: PathBuf, kind: &'static str },
    #[error("{}: a symbolic link that leads out of the tree", .path.display())]
    SymlinkOutside { path: PathBuf },
    #[error("{}: a symbolic link to a directory; only one to a regular file is written, as that file", .path.display())]
    SymlinkToDirectory { path: PathBuf },
    #[error("{}: a symbolic link to nothing", .path.display())]
    Dangling { path: PathBuf },
    #[error("{}: {reason}", .path.display())]
    Limit { path: PathBuf, reason: &'static str },
    #[error("{}: it changed while the image was written", .path.display())]
    Changed { path: PathBuf },
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> WriteError + '_ {
    move |source| WriteError::Io {
        path: path.to_owned(),
        source,
    }
}

enum Kind {
    Dir {
        children: Vec<usize>,
    },
    /// `read` is the file whose bytes are written: the path itself, or a symlink's target.
    File {
        read: PathBuf,
        size: u64,
        executable: bool,
    },
}

struct Node {
    /// Relative to the tree's root, for errors.
    path: PathBuf,
    name: Vec<u8>,
    parent: usize,
    kind: Kind,
}

/// Writes the tree at `source` as an image to `out`, labelled `label` cut to 16 bytes, and
/// returns how many bytes it wrote: a whole number of 4096-byte blocks.
pub fn write(source: &Path, label: &str, out: &mut impl Write) -> Result<u64, WriteError> {
    let root = fs::canonicalize(source).map_err(io_error(source))?;
    let mut nodes = vec![Node {
        path: PathBuf::new(),
        name: Vec::new(),
        parent: 0,
        kind: Kind::Dir {
            children: Vec::new(),
        },
    }];
    scan(&root, &root, 0, 0, &mut nodes)?;
    let layout = Layout::new(&nodes)?;
    let meta = layout.metadata(&nodes, label);
    out.write_all(&meta).map_err(io_error(source))?;
    for (index, node) in nodes.iter().enumerate() {
        match &node.kind {
            Kind::Dir { .. } if !layout.inline[index] => {
                out.write_all(&layout.dir_data[index])
                    .map_err(io_error(&node.path))?;
                pad(out, layout.dir_data[index].len() as u64, &node.path)?;
            }
            Kind::File { read, size, .. } if *size > 0 => {
                copy(read, *size, out, &node.path)?;
                pad(out, *size, &node.path)?;
            }
            _ => {}
        }
    }
    Ok(layout.blocks * BLOCK)
}

/// Adds the entries of the directory `dir` (node `index`, `depth` deep), sorted by name, and
/// everything below them, each directory before what it holds.
fn scan(
    root: &Path,
    dir: &Path,
    index: usize,
    depth: usize,
    nodes: &mut Vec<Node>,
) -> Result<(), WriteError> {
    let parent_path = nodes[index].path.clone();
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(io_error(dir))? {
        let entry = entry.map_err(io_error(dir))?;
        entries.push((entry.file_name().as_bytes().to_vec(), entry.path()));
    }
    entries.sort();
    let limit = |path: &Path, reason| WriteError::Limit {
        path: path.to_owned(),
        reason,
    };
    for (name, full) in entries {
        let path = parent_path.join(std::ffi::OsStr::from_bytes(&name));
        if name.len() > NAME_MAX {
            return Err(limit(&path, "a name longer than 255 bytes"));
        }
        if path.as_os_str().len() > MAX_PATH {
            return Err(limit(&path, "a path longer than 4095 bytes"));
        }
        if nodes.len() >= MAX_NODES {
            return Err(limit(&path, "more than 65536 entries in the tree"));
        }
        if depth + 1 > MAX_DEPTH {
            return Err(limit(&path, "an entry nested more than 64 deep"));
        }
        let meta = fs::symlink_metadata(&full).map_err(io_error(&full))?;
        let kind = if meta.is_dir() {
            Kind::Dir {
                children: Vec::new(),
            }
        } else if meta.is_file() {
            file_kind(&full, &meta, &path)?
        } else if meta.file_type().is_symlink() {
            let target = match fs::canonicalize(&full) {
                Ok(target) => target,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return Err(WriteError::Dangling { path });
                }
                Err(e) => return Err(io_error(&full)(e)),
            };
            if !target.starts_with(root) {
                return Err(WriteError::SymlinkOutside { path });
            }
            let target_meta = fs::metadata(&target).map_err(io_error(&target))?;
            if target_meta.is_dir() {
                return Err(WriteError::SymlinkToDirectory { path });
            }
            if !target_meta.is_file() {
                return Err(WriteError::NotAllowed {
                    path,
                    kind: kind_of(&target_meta),
                });
            }
            file_kind(&target, &target_meta, &path)?
        } else {
            return Err(WriteError::NotAllowed {
                path,
                kind: kind_of(&meta),
            });
        };
        let is_dir = matches!(kind, Kind::Dir { .. });
        let child = nodes.len();
        nodes.push(Node {
            path,
            name,
            parent: index,
            kind,
        });
        if let Kind::Dir { children } = &mut nodes[index].kind {
            children.push(child);
        }
        if is_dir {
            scan(root, &full, child, depth + 1, nodes)?;
        }
    }
    Ok(())
}

fn file_kind(read: &Path, meta: &fs::Metadata, path: &Path) -> Result<Kind, WriteError> {
    if meta.len() > u64::from(u32::MAX) {
        return Err(WriteError::Limit {
            path: path.to_owned(),
            reason: "a file of 4 GiB or more",
        });
    }
    Ok(Kind::File {
        read: read.to_owned(),
        size: meta.len(),
        executable: meta.mode() & 0o111 != 0,
    })
}

fn kind_of(meta: &fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    let t = meta.file_type();
    if t.is_fifo() {
        "fifo"
    } else if t.is_socket() {
        "socket"
    } else if t.is_block_device() || t.is_char_device() {
        "device node"
    } else {
        "file of an unknown type"
    }
}

/// Where everything goes.
struct Layout {
    /// Each node's inode, from byte 0.
    offset: Vec<u64>,
    /// Whether each directory's entries are inline after its inode.
    inline: Vec<bool>,
    /// Each directory's entries, as written: whole blocks padded with zeros for a plain one.
    /// Filled once every inode has its place, since the entries hold their inodes' numbers.
    dir_data: Vec<Vec<u8>>,
    /// The size of each directory's data, which for a plain one ends at its last name.
    dir_size: Vec<u64>,
    /// Each node's first data block, when it has any.
    block: Vec<u64>,
    /// The first block after the metadata, and the image's length in blocks.
    first_data: u64,
    blocks: u64,
}

impl Layout {
    fn new(nodes: &[Node]) -> Result<Self, WriteError> {
        let count = nodes.len();
        let mut layout = Self {
            offset: vec![0; count],
            inline: vec![false; count],
            dir_data: vec![Vec::new(); count],
            dir_size: vec![0; count],
            block: vec![0; count],
            first_data: 0,
            blocks: 0,
        };
        // The entries' sizes do not depend on the inode numbers, so a directory's size, and
        // whether it fits after its inode, are known before anything has a place.
        for (index, node) in nodes.iter().enumerate() {
            if let Kind::Dir { children } = &node.kind {
                // Every block but the last is whole; the last ends at its last name.
                let blocks = dir_blocks(&names(nodes, index, children));
                let last: usize = blocks
                    .last()
                    .map_or(0, |b| b.iter().map(|e| DIRENT_LEN + e.0.len()).sum());
                let size = (blocks.len() as u64 - 1) * BLOCK + last as u64;
                if size > MAX_DIRECTORY {
                    return Err(WriteError::Limit {
                        path: node.path.clone(),
                        reason: "a directory larger than 4 MiB",
                    });
                }
                layout.dir_size[index] = size;
                layout.inline[index] = size <= BLOCK - INODE_LEN;
            }
        }
        let mut cursor = META_START;
        for index in 0..count {
            let tail = if layout.inline[index] {
                layout.dir_size[index]
            } else {
                0
            };
            let need = INODE_LEN + tail;
            if cursor % BLOCK + need > BLOCK {
                cursor = cursor.next_multiple_of(BLOCK);
            }
            layout.offset[index] = cursor;
            cursor = (cursor + need).next_multiple_of(INODE_LEN);
        }
        layout.first_data = cursor.div_ceil(BLOCK);
        let mut next = layout.first_data;
        for (index, node) in nodes.iter().enumerate() {
            let used = match &node.kind {
                Kind::Dir { .. } if !layout.inline[index] => layout.dir_size[index],
                Kind::File { size, .. } => *size,
                Kind::Dir { .. } => 0,
            };
            if used > 0 {
                layout.block[index] = next;
                next += used.div_ceil(BLOCK);
            }
        }
        if next > u64::from(u32::MAX) {
            return Err(WriteError::Limit {
                path: PathBuf::new(),
                reason: "an image of more than 2^32 blocks",
            });
        }
        layout.blocks = next;
        for (index, node) in nodes.iter().enumerate() {
            if let Kind::Dir { children } = &node.kind {
                let blocks = dir_blocks(&names(nodes, index, children));
                let nid = |i: usize| layout.offset[i] / INODE_LEN;
                let mut data = Vec::new();
                for (i, block) in blocks.iter().enumerate() {
                    let start = data.len();
                    let mut name_at = DIRENT_LEN * block.len();
                    for (name, target, file_type) in block {
                        data.extend_from_slice(&nid(*target).to_le_bytes());
                        data.extend_from_slice(&(name_at as u16).to_le_bytes());
                        data.extend_from_slice(&[*file_type, 0]);
                        name_at += name.len();
                    }
                    for (name, ..) in block {
                        data.extend_from_slice(name);
                    }
                    if i + 1 < blocks.len() {
                        data.resize(start + BLOCK as usize, 0);
                    }
                }
                debug_assert_eq!(data.len() as u64, layout.dir_size[index]);
                layout.dir_data[index] = data;
            }
        }
        Ok(layout)
    }

    /// The image up to its first data block: the superblock, every inode and every inline
    /// directory.
    fn metadata(&self, nodes: &[Node], label: &str) -> Vec<u8> {
        let mut m = vec![0u8; (self.first_data * BLOCK) as usize];
        let mut put = |at: u64, bytes: &[u8]| {
            m[at as usize..at as usize + bytes.len()].copy_from_slice(bytes);
        };
        let sb = SUPER_OFFSET;
        put(sb, &MAGIC_BYTES);
        put(sb + 12, &[BLOCK_BITS]);
        put(
            sb + 14,
            &((self.offset[0] / INODE_LEN) as u16).to_le_bytes(),
        );
        put(sb + 16, &(nodes.len() as u64).to_le_bytes());
        put(sb + 36, &(self.blocks as u32).to_le_bytes());
        put(sb + 64, cut(label, 16).as_bytes());

        for (index, node) in nodes.iter().enumerate() {
            let at = self.offset[index];
            let (layout, mode, nlink, size, raw) = match &node.kind {
                Kind::Dir { children } => {
                    let subdirs = children
                        .iter()
                        .filter(|&&c| matches!(nodes[c].kind, Kind::Dir { .. }))
                        .count();
                    let (layout, raw) = if self.inline[index] {
                        (LAYOUT_INLINE, u32::MAX)
                    } else {
                        (LAYOUT_PLAIN, self.block[index] as u32)
                    };
                    let nlink = u16::try_from(2 + subdirs).unwrap_or(u16::MAX);
                    (layout, S_IFDIR | 0o755, nlink, self.dir_size[index], raw)
                }
                Kind::File {
                    size, executable, ..
                } => {
                    let mode = if *executable { 0o755 } else { 0o644 };
                    (
                        LAYOUT_PLAIN,
                        S_IFREG | mode,
                        1,
                        *size,
                        self.block[index] as u32,
                    )
                }
            };
            put(at, &(layout << 1).to_le_bytes());
            put(at + 4, &mode.to_le_bytes());
            put(at + 6, &nlink.to_le_bytes());
            put(at + 8, &(size as u32).to_le_bytes());
            put(at + 16, &raw.to_le_bytes());
            put(at + 20, &(index as u32 + 1).to_le_bytes());
            if self.inline[index] {
                put(at + INODE_LEN, &self.dir_data[index]);
            }
        }
        m
    }
}

/// A directory's entries, `.` and `..` among them, in byte order: each name, the node it
/// names, and its file type.
fn names(nodes: &[Node], index: usize, children: &[usize]) -> Vec<(Vec<u8>, usize, u8)> {
    let mut entries = vec![
        (b".".to_vec(), index, FT_DIR),
        (b"..".to_vec(), nodes[index].parent, FT_DIR),
    ];
    for &child in children {
        let file_type = match nodes[child].kind {
            Kind::Dir { .. } => FT_DIR,
            Kind::File { .. } => FT_REG,
        };
        entries.push((nodes[child].name.clone(), child, file_type));
    }
    entries.sort();
    entries
}

/// The entries split into directory blocks: as many as fit, with their names, in each.
fn dir_blocks(entries: &[(Vec<u8>, usize, u8)]) -> Vec<Vec<(Vec<u8>, usize, u8)>> {
    let mut blocks = vec![Vec::new()];
    let mut used = 0;
    for entry in entries {
        let need = DIRENT_LEN + entry.0.len();
        if used + need > BLOCK as usize {
            blocks.push(Vec::new());
            used = 0;
        }
        blocks.last_mut().unwrap().push(entry.clone());
        used += need;
    }
    blocks
}

/// `text` cut to at most `max` bytes, at a character boundary.
fn cut(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Writes exactly `size` bytes of `read`, which must still be the regular file of that size it
/// was when the tree was scanned.
fn copy(read: &Path, size: u64, out: &mut impl Write, path: &Path) -> Result<(), WriteError> {
    let mut file = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(read)
        .map_err(io_error(read))?;
    let meta = file.metadata().map_err(io_error(read))?;
    let changed = || WriteError::Changed {
        path: path.to_owned(),
    };
    if !meta.is_file() || meta.len() != size {
        return Err(changed());
    }
    let copied = io::copy(&mut (&mut file).take(size), out).map_err(io_error(read))?;
    let mut more = [0u8; 1];
    if copied != size || file.read(&mut more).map_err(io_error(read))? != 0 {
        return Err(changed());
    }
    Ok(())
}

/// Zeros up to the end of the block `written` bytes reached.
fn pad(out: &mut impl Write, written: u64, path: &Path) -> Result<(), WriteError> {
    let zeros = written.next_multiple_of(BLOCK) - written;
    out.write_all(&vec![0u8; zeros as usize])
        .map_err(io_error(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_changed_since_the_scan_or_became_a_symlink_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        fs::write(&file, [7u8; 10]).unwrap();
        let path = Path::new("f");
        copy(&file, 10, &mut Vec::new(), path).unwrap();
        for scanned in [5, 20] {
            assert!(matches!(
                copy(&file, scanned, &mut Vec::new(), path),
                Err(WriteError::Changed { .. })
            ));
        }
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(matches!(
            copy(&link, 10, &mut Vec::new(), path),
            Err(WriteError::Io { .. })
        ));
    }
}
