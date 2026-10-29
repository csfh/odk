use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::fixture::{self, Built, Item, build, file, notes, notes_with};
use super::*;
use crate::trust::{self, Sealed};

/// The same tree as [`fixture::notes`], made by `mkfs.erofs` 1.9.4 (erofs-utils, from Arch's
/// signed package, not installed) with
/// `mkfs.erofs -b 4096 -T 0 --all-root -U clear -L Notes -E noinline_data <out> <tree>`, the
/// tree's directories `0755`, `bin/notes` `0755` and the rest `0644`. `fsck.erofs` accepts it.
const MKFS: &[u8] = include_bytes!("testdata/notes-mkfs-1.9.4.erofs");

fn seal(dir: &Path, bytes: &[u8]) -> Sealed {
    let path = dir.join("Notes.oma");
    fs::write(&path, bytes).unwrap();
    Sealed::memfd(&path).unwrap()
}

/// What reading `file` gives: the tree's paths and kinds, or why not.
fn read(file: &[u8]) -> Result<Vec<(String, Kind)>, Error> {
    let dir = tempfile::tempdir().unwrap();
    let sealed = seal(dir.path(), file);
    let checked = trust::check(&sealed).expect("a file whose envelope holds");
    let image = Image::read(&checked)?;
    Ok(image
        .nodes()
        .iter()
        .map(|n| (n.path.display().to_string(), n.kind))
        .collect())
}

fn read_built(built: &Built) -> Result<Vec<(String, Kind)>, Error> {
    read(&built.signed())
}

fn file_kind(size: u64, executable: bool) -> Kind {
    Kind::File { executable, size }
}

fn expected_tree() -> Vec<(String, Kind)> {
    vec![
        (String::new(), Kind::Dir),
        ("bin".into(), Kind::Dir),
        ("bin/notes".into(), file_kind(64, true)),
        (
            "manifest.toml".into(),
            file_kind(fixture::MANIFEST.len() as u64, false),
        ),
        ("share".into(), Kind::Dir),
        ("share/big".into(), file_kind(5000, false)),
        ("share/empty".into(), file_kind(0, false)),
        ("share/icon.png".into(), file_kind(4, false)),
    ]
}

/// Every file under `dir`, with its mode's permission bits and its bytes.
fn listing(dir: &Path) -> Vec<(String, u32, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in walkdir(dir) {
        let meta = fs::symlink_metadata(&entry).unwrap();
        let rel = entry.strip_prefix(dir).unwrap().display().to_string();
        let bytes = if meta.is_file() {
            fs::read(&entry).unwrap()
        } else {
            assert!(meta.is_dir(), "{rel} is a directory or a file");
            Vec::new()
        };
        out.push((rel, meta.permissions().mode() & 0o7777, bytes));
    }
    out.sort();
    out
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        }
        out.push(path);
    }
    out
}

#[test]
fn a_hand_built_image_reads_back_as_its_tree_and_manifest() {
    let built = build(&notes());
    let dir = tempfile::tempdir().unwrap();
    let sealed = seal(dir.path(), &built.signed());
    let checked = trust::check(&sealed).unwrap();
    let image = Image::read(&checked).unwrap();
    let tree: Vec<_> = image
        .nodes()
        .iter()
        .map(|n| (n.path.display().to_string(), n.kind))
        .collect();
    assert_eq!(tree, expected_tree());
    assert_eq!(image.manifest().identifier.as_str(), "dev.csfh.notes");
    assert_eq!(image.manifest().executable, "bin/notes");
    assert_eq!(
        image.manifest().publisher.as_ref().map(|p| p.as_str()),
        Some("apps.csfh.dev")
    );
    // Three files in a page each, and `share/big` in two.
    assert_eq!(image.unpacked_size(), 5 * 4096);
}

#[test]
fn an_image_unpacks_to_exactly_its_tree() {
    let built = build(&notes());
    let dir = tempfile::tempdir().unwrap();
    let sealed = seal(dir.path(), &built.signed());
    let checked = trust::check(&sealed).unwrap();
    let image = Image::read(&checked).unwrap();
    let out = dir.path().join("app");
    fs::create_dir(&out).unwrap();
    image.unpack(&out).unwrap();
    let mut wanted = Vec::new();
    for (path, item) in notes() {
        if let Item::File { bytes, mode, .. } = item {
            wanted.push((path, u32::from(mode), bytes));
        }
    }
    wanted.push(("bin".into(), 0o755, Vec::new()));
    wanted.push(("share".into(), 0o755, Vec::new()));
    wanted.sort();
    assert_eq!(listing(&out), wanted);
}

#[test]
fn the_same_tree_written_by_mkfs_erofs_reads_the_same() {
    assert_eq!(read(&fixture::signed(MKFS)).unwrap(), expected_tree());
    let dir = tempfile::tempdir().unwrap();
    let unpacked: Vec<_> = [MKFS.to_vec(), build(&notes()).image]
        .iter()
        .enumerate()
        .map(|(i, image)| {
            let sealed = seal(dir.path(), &fixture::signed(image));
            let checked = trust::check(&sealed).unwrap();
            let out = dir.path().join(i.to_string());
            fs::create_dir(&out).unwrap();
            Image::read(&checked).unwrap().unpack(&out).unwrap();
            listing(&out)
        })
        .collect();
    assert_eq!(unpacked[0], unpacked[1]);
}

#[test]
fn an_inline_file_reads_as_a_plain_one() {
    let inline = Item::File {
        bytes: b"hello".to_vec(),
        mode: 0o644,
        inline: true,
    };
    let tree = read_built(&build(&notes_with("share/inline", Some(inline)))).unwrap();
    assert!(tree.contains(&("share/inline".into(), file_kind(5, false))));
}

#[test]
fn the_bytes_read_are_the_sealed_copys_not_the_files_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Notes.oma");
    fs::write(&path, build(&notes()).signed()).unwrap();
    let sealed = Sealed::memfd(&path).unwrap();
    // The file on disk changes after the copy: another tree, still a valid Bundle file.
    let other = build(&notes_with("share/extra", Some(file(b"x".to_vec(), 0o644))));
    fs::write(&path, other.signed()).unwrap();
    let checked = trust::check(&sealed).unwrap();
    let image = Image::read(&checked).unwrap();
    assert!(image.nodes().iter().all(|n| !n.path.ends_with("extra")));
}

#[test]
fn a_file_whose_envelope_does_not_hold_never_reaches_the_reader() {
    // `Image::read` takes only what `trust::check` returns, so a changed byte of the image is
    // the digest's failure, and the image is never parsed.
    let mut file = build(&notes()).signed();
    let last = file.len() - 1;
    file[last] ^= 1;
    let dir = tempfile::tempdir().unwrap();
    let sealed = seal(dir.path(), &file);
    assert!(matches!(
        trust::check(&sealed),
        Err(trust::Error::Digest { what: "payload" })
    ));
}

fn refused(built: &Built) -> Error {
    read_built(built).expect_err("refused")
}

#[test]
fn a_superblock_the_spec_does_not_allow_is_refused() {
    let at = 1024;
    let cases: [(usize, &[u8], &str); 6] = [
        (at + 12, &[13], "blkszbits"),
        (at + 8, &4u32.to_le_bytes(), "feature_compat"),
        (at + 80, &1u32.to_le_bytes(), "feature_incompat"),
        (at + 84, &2u16.to_le_bytes(), "available_compr_algs"),
        (at + 91, &[1], "xattr_prefix_count"),
        (at + 36, &99u32.to_le_bytes(), "blocks"),
    ];
    for (offset, bytes, field) in cases {
        let mut built = build(&notes());
        built.put(offset, bytes);
        match refused(&built) {
            Error::Superblock { field: f, .. } => assert_eq!(f, field),
            e => panic!("{field}: {e}"),
        }
    }
    let mut built = build(&notes());
    built.put(at, &[0; 4]);
    assert!(matches!(refused(&built), Error::NotErofs));
    // The checksum and mtime flags `mkfs.erofs` sets are allowed.
    let mut built = build(&notes());
    built.put(at + 8, &3u32.to_le_bytes());
    assert!(read_built(&built).is_ok());
}

#[test]
fn an_image_longer_than_its_blocks_is_refused() {
    let mut built = build(&notes());
    built.image.extend_from_slice(&[0; 4096]);
    assert!(matches!(
        refused(&built),
        Error::Superblock {
            field: "blocks",
            ..
        }
    ));
}

#[test]
fn compression_another_layout_an_extended_inode_or_an_attribute_is_refused() {
    for (format, expected) in [
        (1u16 << 1, "compressed"),
        (3 << 1, "compressed"),
        (4 << 1, "layout"),
        (1, "extended"),
        (1 << 4, "layout"),
    ] {
        let mut built = build(&notes());
        built.inode_u16("share/big", 0, format);
        let e = refused(&built);
        let ok = match expected {
            "compressed" => matches!(e, Error::Compressed { .. }),
            "layout" => matches!(e, Error::Layout { .. }),
            _ => matches!(e, Error::ExtendedInode { .. }),
        };
        assert!(ok, "{format:#x}: {e}");
    }
    let mut built = build(&notes());
    built.inode_u16("share/big", 2, 1);
    assert!(matches!(refused(&built), Error::Xattr { .. }));
}

#[test]
fn a_device_a_fifo_a_socket_or_a_symlink_is_refused_by_mode_or_by_entry() {
    for (mode, file_type) in [
        (0o020_644u16, 3u8),
        (0o060_644, 4),
        (0o010_644, 5),
        (0o140_644, 6),
        (0o120_777, 7),
    ] {
        let check = |e: &Error| match file_type {
            3 | 4 => matches!(e, Error::Device { .. }),
            5 => matches!(e, Error::Fifo { .. }),
            6 => matches!(e, Error::Socket { .. }),
            _ => matches!(e, Error::Symlink { .. }),
        };
        // The inode says so, and the entry still says a regular file.
        let mut built = build(&notes());
        built.inode_u16("share/icon.png", 4, mode);
        let e = refused(&built);
        assert!(check(&e), "mode {mode:o}: {e}");
        // The entry says so.
        let mut built = build(&notes());
        let at = built.dirent["share/icon.png"] + 10;
        built.put(at, &[file_type]);
        let e = refused(&built);
        assert!(check(&e), "type {file_type}: {e}");
    }
}

#[test]
fn setuid_or_setgid_is_refused_on_a_file_or_a_directory() {
    for (path, mode) in [("bin/notes", 0o104_755u16), ("share", 0o042_755)] {
        let mut built = build(&notes());
        built.inode_u16(path, 4, mode);
        assert!(
            matches!(refused(&built), Error::SetId { .. }),
            "{path} {mode:o}"
        );
    }
}

#[test]
fn data_past_the_end_or_shared_with_anything_else_is_refused() {
    // `share/big` grows past the image's last block.
    let mut built = build(&notes());
    built.inode_u32("share/big", 8, 8 * 4096);
    assert!(matches!(refused(&built), Error::PastPayload { .. }));
    // Its blocks start where `bin/notes`'s are.
    let mut built = build(&notes());
    let notes_block = (built.data["bin/notes"] / 4096) as u32;
    built.inode_u32("share/big", 16, notes_block);
    assert!(matches!(refused(&built), Error::Overlap { .. }));
    // Or in block 0, which is the prefix's and the superblock's.
    let mut built = build(&notes());
    built.inode_u32("share/big", 16, 0);
    assert!(matches!(refused(&built), Error::Overlap { .. }));
    // An entry names an inode inside the superblock.
    let mut built = build(&notes());
    let at = built.dirent["share/big"];
    built.put(at, &33u64.to_le_bytes());
    assert!(matches!(refused(&built), Error::Overlap { .. }));
}

#[test]
fn a_file_in_the_tree_twice_is_refused() {
    let mut built = build(&notes());
    let nid = built.nid("share/icon.png");
    let at = built.dirent["share/empty"];
    built.put(at, &nid.to_le_bytes());
    assert!(matches!(refused(&built), Error::Linked { .. }));
    // A directory that holds itself is the same refusal, not a loop.
    let mut built = build(&notes());
    let nid = built.nid("");
    let at = built.dirent["share"];
    built.put(at, &nid.to_le_bytes());
    assert!(matches!(refused(&built), Error::Linked { .. }));
}

#[test]
fn an_entry_whose_type_is_not_its_inodes_is_refused() {
    for (path, file_type) in [("share/icon.png", 2u8), ("share", 1)] {
        let mut built = build(&notes());
        let at = built.dirent[path] + 10;
        built.put(at, &[file_type]);
        assert!(
            matches!(
                refused(&built),
                Error::Directory {
                    reason: "an entry's type is not its inode's",
                    ..
                }
            ),
            "{path}"
        );
    }
}

#[test]
fn names_out_of_order_or_a_wrong_dot_entry_are_refused() {
    let tree = vec![
        ("a1".to_owned(), file(b"1".to_vec(), 0o644)),
        ("a2".to_owned(), file(b"2".to_vec(), 0o644)),
    ];
    let mut built = build(&tree);
    // Swap the last bytes of the two names, so `a2` comes before `a1`.
    let root = built.inode[""] + 32;
    let names = root + 12 * 4 + 1 + 2;
    assert_eq!(&built.image[names..names + 4], b"a1a2");
    built.put(names, b"a2a1");
    assert!(matches!(
        refused(&built),
        Error::Directory {
            reason: "its names are not in order",
            ..
        }
    ));

    let mut built = build(&notes());
    let share = built.inode["share"] + 32;
    let other = built.nid("bin");
    built.put(share, &other.to_le_bytes());
    assert!(matches!(refused(&built), Error::Directory { .. }));
}

#[test]
fn the_manifest_must_be_there_and_match_the_signed_one() {
    assert!(matches!(
        refused(&build(&notes_with("manifest.toml", None))),
        Error::NoManifest
    ));
    for (from, to, field) in [
        ("dev.csfh.notes", "dev.csfh.other", "identifier"),
        (
            "executable = \"bin/notes\"",
            "executable = \"bin/other\"",
            "executable",
        ),
        ("apps.csfh.dev", "other.csfh.dev", "publisher"),
        ("publisher = \"apps.csfh.dev\"\n", "", "publisher"),
    ] {
        let text = fixture::MANIFEST.replace(from, to);
        let mut tree = notes_with("manifest.toml", Some(file(text, 0o644)));
        tree.push(("bin/other".into(), file(crate::testutil::elf(), 0o755)));
        match refused(&build(&tree)) {
            Error::Mismatch { field: f, .. } => assert_eq!(f, field),
            e => panic!("{field}: {e}"),
        }
    }
}

#[test]
fn the_programs_and_the_icon_are_held_to_a_directory_bundles_checks() {
    let cases = [
        (
            "bin/notes",
            file(crate::testutil::elf(), 0o644),
            "executable bit",
        ),
        (
            "bin/notes",
            file(b"#!/bin/sh\n".to_vec(), 0o755),
            "not an ELF",
        ),
        ("bin/notes", Item::Dir, "not a regular file"),
        ("share/icon.png", Item::Dir, "not a regular file"),
    ];
    for (path, item, reason) in cases {
        let mut tree = notes_with(path, None);
        if matches!(item, Item::Dir) {
            tree.push((format!("{path}/inside"), file(b"x".to_vec(), 0o644)));
        } else {
            tree.push((path.into(), item));
        }
        let e = refused(&build(&tree));
        assert!(
            matches!(e, Error::Manifest(_)) && e.to_string().contains(reason),
            "{path}: {e}"
        );
    }
    let e = refused(&build(&notes_with("share/icon.png", None)));
    assert!(e.to_string().contains("does not exist"), "{e}");
}
