//! Images built by hand for the reader's tests (ADR 0067). The layout is the one `mkfs.erofs`
//! 1.9.4 writes with `-b 4096 -T 0 --all-root -U clear -E noinline_data`: the root inode right
//! after the superblock, compact inodes in tree order, each directory inline after its inode,
//! and each file in whole blocks after the metadata. Nothing here shares code with the reader,
//! so a misreading of the format shows up as a disagreement, and `fsck.erofs` accepts what this
//! writes (`docs/checks.md`).

use std::collections::{BTreeMap, BTreeSet};

use crate::trust::envelope::tests::{bundle, release_key};

const BLOCK: usize = 4096;
const META: usize = 1152;

/// One entry of a tree: a directory, or a file with its bytes and permission bits. An inline
/// file is stored as its directory is, after its inode.
#[derive(Debug, Clone)]
pub(crate) enum Item {
    Dir,
    File {
        bytes: Vec<u8>,
        mode: u16,
        inline: bool,
    },
}

pub(crate) fn file(bytes: impl Into<Vec<u8>>, mode: u16) -> Item {
    Item::File {
        bytes: bytes.into(),
        mode,
        inline: false,
    }
}

/// An image, and where its parts are, for tests that change one of them.
pub(crate) struct Built {
    pub image: Vec<u8>,
    /// Each path's inode, the root's under "".
    pub inode: BTreeMap<String, usize>,
    /// Each path's entry in its parent's directory.
    pub dirent: BTreeMap<String, usize>,
    /// The first byte of each file's data.
    pub data: BTreeMap<String, usize>,
}

impl Built {
    pub fn put(&mut self, at: usize, bytes: &[u8]) {
        self.image[at..at + bytes.len()].copy_from_slice(bytes);
    }

    /// Sets the 16-bit field `offset` bytes into `path`'s inode.
    pub fn inode_u16(&mut self, path: &str, offset: usize, value: u16) {
        let at = self.inode[path] + offset;
        self.put(at, &value.to_le_bytes());
    }

    pub fn inode_u32(&mut self, path: &str, offset: usize, value: u32) {
        let at = self.inode[path] + offset;
        self.put(at, &value.to_le_bytes());
    }

    pub fn nid(&self, path: &str) -> u64 {
        (self.inode[path] / 32) as u64
    }

    /// The Bundle file: the image behind a prefix signed by the tests' release key for the
    /// sample signed manifest (`dev.csfh.notes`, `bin/notes`, `apps.csfh.dev`).
    pub fn signed(&self) -> Vec<u8> {
        signed(&self.image)
    }
}

pub(crate) fn signed(image: &[u8]) -> Vec<u8> {
    bundle(&image[1024..], &release_key(), |_| {})
}

pub(crate) const MANIFEST: &str = "manifest_version = 1\nidentifier = \"dev.csfh.notes\"\nname = \"Notes\"\nversion = \"1.0.0\"\nexecutable = \"bin/notes\"\nicon = \"share/icon.png\"\npublisher = \"apps.csfh.dev\"\n\n[permissions]\n";

/// The tree of the tests' Bundle, which matches the sample signed manifest. `share/big` takes
/// two blocks; `share/empty` none.
pub(crate) fn notes() -> Vec<(String, Item)> {
    let big: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    vec![
        ("manifest.toml".into(), file(MANIFEST, 0o644)),
        ("bin/notes".into(), file(crate::testutil::elf(), 0o755)),
        ("share/icon.png".into(), file(b"\x89PNG".to_vec(), 0o644)),
        ("share/empty".into(), file(Vec::new(), 0o644)),
        ("share/big".into(), file(big, 0o644)),
    ]
}

/// `notes()` with `path` replaced, added or, for `None`, taken out.
pub(crate) fn notes_with(path: &str, item: Option<Item>) -> Vec<(String, Item)> {
    let mut tree: Vec<_> = notes().into_iter().filter(|(p, _)| p != path).collect();
    if let Some(item) = item {
        tree.push((path.to_owned(), item));
    }
    tree
}

struct Node {
    item: Item,
    children: BTreeSet<String>,
}

/// Lays `entries` out as an image. Parent directories are added as needed.
pub(crate) fn build(entries: &[(String, Item)]) -> Built {
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    nodes.insert(
        String::new(),
        Node {
            item: Item::Dir,
            children: BTreeSet::new(),
        },
    );
    for (path, item) in entries {
        let mut parent = String::new();
        let parts: Vec<&str> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            let here = if parent.is_empty() {
                (*part).to_owned()
            } else {
                format!("{parent}/{part}")
            };
            nodes
                .get_mut(&parent)
                .unwrap()
                .children
                .insert((*part).to_owned());
            let last = i + 1 == parts.len();
            nodes.entry(here.clone()).or_insert_with(|| Node {
                item: if last { item.clone() } else { Item::Dir },
                children: BTreeSet::new(),
            });
            parent = here;
        }
    }

    // Tree order: each directory, then what it holds, by name.
    let mut order = Vec::new();
    let mut stack = vec![String::new()];
    while let Some(path) = stack.pop() {
        let children: Vec<String> = nodes[&path]
            .children
            .iter()
            .map(|c| join(&path, c))
            .collect();
        order.push(path);
        stack.extend(children.into_iter().rev());
    }

    let dir_names = |path: &str| -> Vec<String> {
        let mut names: Vec<String> = vec![".".into(), "..".into()];
        names.extend(nodes[path].children.iter().cloned());
        names.sort();
        names
    };
    let inline_len = |path: &str| -> usize {
        match &nodes[path].item {
            Item::Dir => dir_names(path).iter().map(|n| 12 + n.len()).sum(),
            Item::File {
                bytes,
                inline: true,
                ..
            } => bytes.len(),
            Item::File { .. } => 0,
        }
    };

    let mut inode = BTreeMap::new();
    let mut cursor = META;
    for path in &order {
        let need = 32 + inline_len(path);
        if cursor % BLOCK + need > BLOCK {
            cursor = cursor.next_multiple_of(BLOCK);
        }
        inode.insert(path.clone(), cursor);
        cursor = (cursor + need).next_multiple_of(32);
    }
    let mut next_block = cursor.div_ceil(BLOCK);
    let mut data = BTreeMap::new();
    for path in &order {
        if let Item::File {
            bytes,
            inline: false,
            ..
        } = &nodes[path].item
            && !bytes.is_empty()
        {
            data.insert(path.clone(), next_block * BLOCK);
            next_block += bytes.len().div_ceil(BLOCK);
        }
    }

    let blocks = next_block;
    let mut image = vec![0u8; blocks * BLOCK];
    let mut put = |at: usize, bytes: &[u8]| image[at..at + bytes.len()].copy_from_slice(bytes);

    // The superblock.
    put(1024, &0xE0F5_E1E2u32.to_le_bytes());
    put(1024 + 12, &[12]);
    put(1024 + 14, &((META / 32) as u16).to_le_bytes());
    put(1024 + 16, &(order.len() as u64).to_le_bytes());
    put(1024 + 36, &(blocks as u32).to_le_bytes());
    put(1024 + 64, b"Notes");

    let mut dirent = BTreeMap::new();
    for (ino, path) in order.iter().enumerate() {
        let at = inode[path];
        let node = &nodes[path];
        let (format, mode, nlink, size, raw) = match &node.item {
            Item::Dir => {
                let subdirs = node
                    .children
                    .iter()
                    .filter(|c| matches!(nodes[&join(path, c)].item, Item::Dir))
                    .count();
                let len = inline_len(path);
                (2u16 << 1, 0o040_755u16, 2 + subdirs as u16, len, u32::MAX)
            }
            Item::File {
                bytes,
                mode,
                inline: true,
            } => (2 << 1, 0o100_000 | mode, 1, bytes.len(), u32::MAX),
            Item::File { bytes, mode, .. } => {
                let raw = data.get(path).map_or(0, |b| (b / BLOCK) as u32);
                (0, 0o100_000 | mode, 1, bytes.len(), raw)
            }
        };
        put(at, &format.to_le_bytes());
        put(at + 4, &mode.to_le_bytes());
        put(at + 6, &nlink.to_le_bytes());
        put(at + 8, &(size as u32).to_le_bytes());
        put(at + 16, &raw.to_le_bytes());
        put(at + 20, &(ino as u32 + 1).to_le_bytes());

        match &node.item {
            Item::Dir => {
                let names = dir_names(path);
                let parent = path.rsplit_once('/').map_or("", |(p, _)| p);
                let mut name_at = 12 * names.len();
                for (i, name) in names.iter().enumerate() {
                    let (target, file_type) = match name.as_str() {
                        "." => (path.clone(), 2u8),
                        ".." => (parent.to_owned(), 2),
                        _ => {
                            let child = join(path, name);
                            let ft = if matches!(nodes[&child].item, Item::Dir) {
                                2
                            } else {
                                1
                            };
                            dirent.insert(child.clone(), at + 32 + 12 * i);
                            (child, ft)
                        }
                    };
                    let entry = at + 32 + 12 * i;
                    put(entry, &((inode[&target] / 32) as u64).to_le_bytes());
                    put(entry + 8, &(name_at as u16).to_le_bytes());
                    put(entry + 10, &[file_type]);
                    put(at + 32 + name_at, name.as_bytes());
                    name_at += name.len();
                }
            }
            Item::File {
                bytes,
                inline: true,
                ..
            } => put(at + 32, bytes),
            Item::File { bytes, .. } => {
                if let Some(&start) = data.get(path) {
                    put(start, bytes);
                }
            }
        }
    }
    Built {
        image,
        inode,
        dirent,
        data,
    }
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}
