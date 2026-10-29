//! The EROFS structures a Bundle's image is made of, read only as `docs/spec/oma-image.md` allows
//! them: the superblock, compact inodes, and directory blocks. Every number is little-endian, and
//! every offset counts from byte 0 of the file, since the prefix sits in the first 1024 bytes the
//! format leaves unused.

use std::path::{Path, PathBuf};

use super::Error;
use crate::trust::Sealed;

pub(super) const BLOCK: u64 = 4096;
const SUPER_OFFSET: u64 = 1024;
const SUPER_LEN: usize = 128;
/// The first byte after the superblock: nothing before it is the image's metadata or data.
pub(super) const META_START: u64 = SUPER_OFFSET + SUPER_LEN as u64;
const MAGIC: u32 = 0xE0F5_E1E2;
const BLOCK_BITS: u8 = 12;
/// The superblock checksum and the mtime field, which `mkfs.erofs` sets. Neither changes how a
/// file reads, and the signature already covers every byte, so both are allowed and ignored.
const COMPAT_ALLOWED: u32 = 0x1 | 0x2;
const INODE_LEN: u64 = 32;
const DIRENT_LEN: usize = 12;
const NAME_MAX: usize = 255;

const LAYOUT_PLAIN: u16 = 0;
const LAYOUT_COMPRESSED_LEGACY: u16 = 1;
const LAYOUT_INLINE: u16 = 2;
const LAYOUT_COMPRESSED: u16 = 3;

const S_IFMT: u16 = 0o170_000;
pub(super) const S_IFDIR: u16 = 0o040_000;
pub(super) const S_IFREG: u16 = 0o100_000;
const S_IFLNK: u16 = 0o120_000;
const S_IFCHR: u16 = 0o020_000;
const S_IFBLK: u16 = 0o060_000;
const S_IFIFO: u16 = 0o010_000;
const S_IFSOCK: u16 = 0o140_000;
const SET_ID: u16 = 0o6000;

pub(super) const FT_REG: u8 = 1;
pub(super) const FT_DIR: u8 = 2;

/// What the superblock says about the rest of the image.
#[derive(Debug, Clone, Copy)]
pub(super) struct Superblock {
    pub root_nid: u64,
    pub blocks: u64,
    meta_blkaddr: u64,
}

/// Reads the superblock at byte 1024 and holds every field that changes how the image reads to
/// the one value the spec allows. The image is exactly `blocks` blocks, which is the whole file.
pub(super) fn superblock(file: &Sealed) -> Result<Superblock, Error> {
    let mut b = [0u8; SUPER_LEN];
    if file.len() < META_START {
        return Err(Error::NotErofs);
    }
    file.read_exact_at(&mut b, SUPER_OFFSET)?;
    let u16_at = |at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let u64_at = |at: usize| u64::from_le_bytes(b[at..at + 8].try_into().unwrap());
    if u32_at(0) != MAGIC {
        return Err(Error::NotErofs);
    }
    let bad = |field, value: u64, expected| Error::Superblock {
        field,
        value,
        expected,
    };
    if u32_at(8) & !COMPAT_ALLOWED != 0 {
        return Err(bad(
            "feature_compat",
            u32_at(8).into(),
            "only the checksum and mtime flags",
        ));
    }
    let fixed = [
        (
            "blkszbits",
            u64::from(b[12]),
            u64::from(BLOCK_BITS),
            "4 KiB blocks",
        ),
        ("sb_extslots", u64::from(b[13]), 0, "no extension slots"),
        (
            "feature_incompat",
            u32_at(80).into(),
            0,
            "no incompatible features",
        ),
        (
            "available_compr_algs",
            u16_at(84).into(),
            0,
            "no compression",
        ),
        ("extra_devices", u16_at(86).into(), 0, "one device"),
        (
            "dirblkbits",
            u64::from(b[90]),
            0,
            "directory blocks of one block",
        ),
        (
            "xattr_prefix_count",
            u64::from(b[91]),
            0,
            "no extended attributes",
        ),
        ("packed_nid", u64_at(96), 0, "no packed inode"),
    ];
    for (field, value, wanted, expected) in fixed {
        if value != wanted {
            return Err(bad(field, value, expected));
        }
    }
    let blocks = u64::from(u32_at(36));
    if blocks.checked_mul(BLOCK) != Some(file.len()) {
        return Err(bad(
            "blocks",
            blocks,
            "exactly the file's length, which the header's payload length fixes",
        ));
    }
    let meta_blkaddr = u64::from(u32_at(40));
    if meta_blkaddr >= blocks {
        return Err(bad("meta_blkaddr", meta_blkaddr, "a block of the image"));
    }
    Ok(Superblock {
        root_nid: u64::from(u16_at(14)),
        blocks,
        meta_blkaddr,
    })
}

/// A compact inode, already held to the spec: a directory or a regular file, stored plain or
/// inline, with no extended attributes and neither setuid nor setgid.
#[derive(Debug, Clone, Copy)]
pub(super) struct Inode {
    pub offset: u64,
    pub mode: u16,
    pub data: Data,
}

impl Inode {
    pub fn is_dir(&self) -> bool {
        self.mode & S_IFMT == S_IFDIR
    }
}

/// Where an inode's bytes are: whole blocks from `block`, then for an inline inode the tail
/// that follows the inode in its own block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Data {
    pub size: u64,
    /// The first byte of the inode's whole blocks, and how many there are.
    pub blocks: (u64, u64),
    /// The inline tail's first byte and its length.
    pub tail: (u64, u64),
}

impl Data {
    /// The byte ranges of the file's contents, in order.
    pub fn ranges(&self) -> [(u64, u64); 2] {
        let in_blocks = self.size - self.tail.1;
        [(self.blocks.0, in_blocks), self.tail]
    }
}

/// Reads the inode `nid` names; `path` is what an error calls it.
pub(super) fn inode(file: &Sealed, sb: &Superblock, nid: u64, path: &Path) -> Result<Inode, Error> {
    let past = || Error::PastPayload {
        path: path.to_owned(),
    };
    let offset = nid
        .checked_mul(INODE_LEN)
        .and_then(|o| o.checked_add(sb.meta_blkaddr * BLOCK))
        .ok_or_else(past)?;
    if offset < META_START {
        return Err(Error::Overlap {
            path: path.to_owned(),
        });
    }
    if offset
        .checked_add(INODE_LEN)
        .is_none_or(|end| end > file.len())
    {
        return Err(past());
    }
    let mut b = [0u8; INODE_LEN as usize];
    file.read_exact_at(&mut b, offset)?;
    let u16_at = |at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let format = u16_at(0);
    let path = path.to_owned();
    if format & 1 != 0 {
        return Err(Error::ExtendedInode { path });
    }
    let layout = (format >> 1) & 0x7;
    if format >> 4 != 0 {
        return Err(Error::Layout {
            path,
            layout: format,
        });
    }
    match layout {
        LAYOUT_PLAIN | LAYOUT_INLINE => {}
        LAYOUT_COMPRESSED_LEGACY | LAYOUT_COMPRESSED => return Err(Error::Compressed { path }),
        _ => return Err(Error::Layout { path, layout }),
    }
    if u16_at(2) != 0 {
        return Err(Error::Xattr { path });
    }
    let mode = u16_at(4);
    match mode & S_IFMT {
        S_IFDIR | S_IFREG => {}
        S_IFCHR | S_IFBLK => return Err(Error::Device { path }),
        S_IFIFO => return Err(Error::Fifo { path }),
        S_IFSOCK => return Err(Error::Socket { path }),
        S_IFLNK => return Err(Error::Symlink { path }),
        _ => return Err(Error::Mode { path, mode }),
    }
    if mode & SET_ID != 0 {
        return Err(Error::SetId { path });
    }
    let size = u64::from(u32_at(8));
    let raw_blkaddr = u64::from(u32_at(16));
    let count = size.div_ceil(BLOCK);
    let data = if layout == LAYOUT_INLINE {
        // As the kernel reads it: every block but the last is whole, and the last is the tail,
        // which has to fit in the inode's own block after the inode.
        let whole = count.saturating_sub(1);
        let tail_len = size - whole * BLOCK;
        let tail_at = offset + INODE_LEN;
        if offset % BLOCK + INODE_LEN + tail_len > BLOCK {
            return Err(Error::Inline { path });
        }
        Data {
            size,
            blocks: (raw_blkaddr * BLOCK, whole),
            tail: (tail_at, tail_len),
        }
    } else {
        Data {
            size,
            blocks: (raw_blkaddr * BLOCK, count),
            tail: (offset + INODE_LEN, 0),
        }
    };
    // Block 0 holds the prefix and the superblock, so no file's blocks start there.
    let count = data.blocks.1;
    if count > 0 && raw_blkaddr == 0 {
        return Err(Error::Overlap { path });
    }
    if count > 0 && raw_blkaddr + count > sb.blocks {
        return Err(Error::PastPayload { path });
    }
    Ok(Inode { offset, mode, data })
}

/// The byte ranges an inode takes up: the inode with its inline tail, and its whole blocks.
pub(super) fn claims(inode: &Inode) -> [(u64, u64); 2] {
    let (start, count) = inode.data.blocks;
    [
        (inode.offset, INODE_LEN + inode.data.tail.1),
        (start, count * BLOCK),
    ]
}

/// One directory entry.
#[derive(Debug, Clone)]
pub(super) struct Dirent {
    pub nid: u64,
    pub name: Vec<u8>,
    pub file_type: u8,
}

/// The entries of one directory block, `chunk`, as the kernel reads them: the first entry's name
/// offset says how many entries the block holds, each name runs to the next one's offset, and
/// the last runs to the end of the block or the first zero byte, after which there are only
/// zeros.
pub(super) fn dirents(chunk: &[u8], path: &Path) -> Result<Vec<Dirent>, Error> {
    let bad = |reason| Error::Directory {
        path: path.to_owned(),
        reason,
    };
    if chunk.len() < DIRENT_LEN {
        return Err(bad("a block is too short for an entry"));
    }
    let nameoff =
        |i: usize| usize::from(u16::from_le_bytes([chunk[i * 12 + 8], chunk[i * 12 + 9]]));
    let first = nameoff(0);
    if first % DIRENT_LEN != 0 || first == 0 || first > chunk.len() {
        return Err(bad("an entry's name is not where the entries end"));
    }
    let count = first / DIRENT_LEN;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let start = nameoff(i);
        let end = if i + 1 < count {
            nameoff(i + 1)
        } else {
            chunk.len()
        };
        if start < first || end <= start || end > chunk.len() {
            return Err(bad("an entry's name is out of place"));
        }
        let mut name = &chunk[start..end];
        if i + 1 == count
            && let Some(nul) = name.iter().position(|&b| b == 0)
        {
            if name[nul..].iter().any(|&b| b != 0) {
                return Err(bad("there are bytes after the last name"));
            }
            name = &name[..nul];
        }
        if name.is_empty() || name.len() > NAME_MAX {
            return Err(bad("a name is empty or longer than 255 bytes"));
        }
        if name.iter().any(|&b| b == b'/' || b == 0) {
            return Err(bad("a name holds '/' or a zero byte"));
        }
        let at = i * DIRENT_LEN;
        out.push(Dirent {
            nid: u64::from_le_bytes(chunk[at..at + 8].try_into().unwrap()),
            name: name.to_vec(),
            file_type: chunk[at + 10],
        });
    }
    Ok(out)
}

/// The path of `name` in the directory at `parent`.
pub(super) fn child(parent: &Path, name: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    parent.join(std::ffi::OsStr::from_bytes(name))
}

/// The error for an entry whose type is not a directory's or a regular file's.
pub(super) fn file_type_error(file_type: u8, path: PathBuf) -> Error {
    match file_type {
        3 | 4 => Error::Device { path },
        5 => Error::Fifo { path },
        6 => Error::Socket { path },
        7 => Error::Symlink { path },
        _ => Error::Directory {
            path,
            reason: "an entry has an unknown file type",
        },
    }
}
