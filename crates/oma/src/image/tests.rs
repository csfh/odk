use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::fixture::{self, Built, Item, build, file, notes, notes_with};
use super::*;
use crate::trust::{self, Sealed};

/// The same tree as [`fixture::notes`], made by `mkfs.erofs` 1.9.4 (erofs-utils, from Arch's
/// signed package, not installed) with
/// `mkfs.erofs -b 4096 -T 0 --all-root -U clear -L Notes -E noinline_data <out> <tree>`, the
/// tree's directories `0755`, `bin/notes` `0755` and the rest `0644`. `fsck.erofs` accepts it.
const MKFS: &[u8] = include_bytes!("testdata/notes-mkfs-1.9.4.erofs");

/// [`fixture::notes`] and the 300 long names of [`many`] as empty files, from `mkfs.erofs` 1.9.4
/// with the same options. Its `many` directory is inline: four whole blocks, then the tail after
/// its inode, which is a layout the writer never emits. `fsck.erofs` accepts it.
const MKFS_MANY: &[u8] = include_bytes!("testdata/many-mkfs-1.9.4.erofs");

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

// The writer.

/// Lays `entries` out under `root` on disk, with their modes.
fn make_tree(root: &Path, entries: &[(String, Item)]) {
    for (path, item) in entries {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        match item {
            Item::Dir => fs::create_dir_all(&full).unwrap(),
            Item::File { bytes, mode, .. } => {
                fs::write(&full, bytes).unwrap();
                fs::set_permissions(&full, fs::Permissions::from_mode(u32::from(*mode))).unwrap();
            }
        }
    }
}

/// The tests' Bundle, and around it what the reader's own fixtures never had: files of 0, 4096
/// and 4097 bytes, an empty and a nested directory, a setuid executable, and a directory of 300
/// long names that takes several blocks.
fn writer_tree() -> Vec<(String, Item)> {
    let mut tree = notes();
    tree.push(("sizes/4096".into(), file(vec![1u8; 4096], 0o644)));
    tree.push(("sizes/4097".into(), file(vec![2u8; 4097], 0o600)));
    tree.push(("sizes/setuid".into(), file(vec![3u8; 9000], 0o4755)));
    tree.push(("empty-dir".into(), Item::Dir));
    tree.push(("a/b/c/d/deep.txt".into(), file(b"deep".to_vec(), 0o644)));
    // Names that sort before `.` and `..`.
    tree.push(("share/-dash".into(), file(b"-".to_vec(), 0o644)));
    tree.push(("share/+plus".into(), file(b"+".to_vec(), 0o644)));
    tree.extend(many(300));
    tree
}

fn many(count: usize) -> Vec<(String, Item)> {
    (0..count)
        .map(|i| {
            let name = format!("many/entry-{i:04}-{}", "x".repeat(40));
            (name, file(i.to_le_bytes().to_vec(), 0o644))
        })
        .collect()
}

/// What `entries` should unpack to: every directory `0755`, and every file `0755` when any
/// executable bit is set and `0644` when not.
fn normalized(entries: &[(String, Item)]) -> Vec<(String, u32, Vec<u8>)> {
    let mut out = std::collections::BTreeMap::new();
    for (path, item) in entries {
        let mut parent = Path::new(path).parent();
        while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
            out.insert(p.display().to_string(), (0o755, Vec::new()));
            parent = p.parent();
        }
        let entry = match item {
            Item::Dir => (0o755, Vec::new()),
            Item::File { bytes, mode, .. } => {
                let mode = if mode & 0o111 != 0 { 0o755 } else { 0o644 };
                (mode, bytes.clone())
            }
        };
        out.insert(path.clone(), entry);
    }
    out.into_iter().map(|(p, (m, b))| (p, m, b)).collect()
}

fn written(root: &Path) -> Vec<u8> {
    let mut image = Vec::new();
    let len = write(root, "Notes", &mut image).unwrap();
    assert_eq!(len, image.len() as u64);
    assert_eq!(len % 4096, 0);
    image
}

/// Reads and unpacks `image`, signed, and lists what it unpacked to.
fn unpacked(image: &[u8]) -> Vec<(String, u32, Vec<u8>)> {
    let dir = tempfile::tempdir().unwrap();
    let sealed = seal(dir.path(), &fixture::signed(image));
    let checked = trust::check(&sealed).unwrap();
    let out = dir.path().join("app");
    fs::create_dir(&out).unwrap();
    Image::read(&checked).unwrap().unpack(&out).unwrap();
    listing(&out)
}

#[test]
fn what_the_writer_writes_the_reader_reads_back_as_the_tree() {
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &writer_tree());
    let image = written(src.path());
    assert_eq!(unpacked(&image), normalized(&writer_tree()));
    // The label, the superblock's root and its size.
    assert_eq!(&image[1024 + 64..1024 + 69], b"Notes");
    assert_eq!(
        u32::from_le_bytes(image[1024 + 36..1024 + 40].try_into().unwrap()) as usize * 4096,
        image.len()
    );
}

#[test]
fn the_same_tree_gives_the_same_bytes_whatever_its_times_and_other_mode_bits() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    make_tree(a.path(), &writer_tree());
    // The other copy, made in another order, with other times and permission bits that do not
    // change what is written.
    let mut reversed = writer_tree();
    reversed.reverse();
    make_tree(b.path(), &reversed);
    let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    fs::File::open(b.path().join("manifest.toml"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    fs::set_permissions(
        b.path().join("bin/notes"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::set_permissions(b.path().join("share"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(written(a.path()), written(b.path()));
}

#[test]
fn a_symlink_to_a_file_in_the_tree_is_that_file_and_any_other_is_refused() {
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &notes());
    std::os::unix::fs::symlink("../manifest.toml", src.path().join("share/link")).unwrap();
    let image = written(src.path());
    let found = unpacked(&image);
    let link = found.iter().find(|(p, ..)| p == "share/link").unwrap();
    assert_eq!(link.2, fixture::MANIFEST.as_bytes());

    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"x").unwrap();
    for (target, expected) in [
        (outside.path().join("secret"), "outside"),
        (PathBuf::from("bin"), "directory"),
        (PathBuf::from("nothing"), "dangling"),
    ] {
        let src = tempfile::tempdir().unwrap();
        make_tree(src.path(), &notes());
        std::os::unix::fs::symlink(&target, src.path().join("link")).unwrap();
        let e = write(src.path(), "Notes", &mut Vec::new()).unwrap_err();
        let ok = match expected {
            "outside" => matches!(e, WriteError::SymlinkOutside { .. }),
            "directory" => matches!(e, WriteError::SymlinkToDirectory { .. }),
            _ => matches!(e, WriteError::Dangling { .. }),
        };
        assert!(ok, "{expected}: {e}");
    }
}

#[test]
fn a_fifo_is_refused() {
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &notes());
    let fifo =
        std::ffi::CString::new(src.path().join("fifo").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
    let e = write(src.path(), "Notes", &mut Vec::new()).unwrap_err();
    assert!(
        matches!(e, WriteError::NotAllowed { kind: "fifo", .. }),
        "{e}"
    );
}

#[test]
fn a_tree_past_the_readers_limits_is_refused_before_anything_is_written() {
    // A file 64 deep is written and read back; one 65 deep is refused, as the reader would.
    let deep = |dirs: usize| {
        let mut tree = notes();
        let path: PathBuf = (0..dirs).map(|_| "d").collect();
        tree.push((
            path.join("f").display().to_string(),
            file(b"f".to_vec(), 0o644),
        ));
        tree
    };
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &deep(63));
    assert_eq!(unpacked(&written(src.path())), normalized(&deep(63)));
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &deep(64));
    let e = write(src.path(), "Notes", &mut Vec::new()).unwrap_err();
    assert!(e.to_string().contains("64 deep"), "{e}");

    // A file of 4 GiB, sparse, so nothing is read.
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &notes());
    fs::File::create(src.path().join("huge"))
        .unwrap()
        .set_len(1 << 32)
        .unwrap();
    let mut out = Vec::new();
    let e = write(src.path(), "Notes", &mut out).unwrap_err();
    assert!(e.to_string().contains("4 GiB"), "{e}");
    assert!(out.is_empty(), "nothing written");

    // A directory past 4 MiB: 16 000 names of 255 bytes.
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &notes());
    let big = src.path().join("big");
    fs::create_dir(&big).unwrap();
    for i in 0..16_000 {
        let name = format!("{i:05}{}", "n".repeat(250));
        fs::File::create(big.join(name)).unwrap();
    }
    let e = write(src.path(), "Notes", &mut Vec::new()).unwrap_err();
    assert!(e.to_string().contains("4 MiB"), "{e}");
}

#[test]
fn the_label_is_cut_to_16_bytes_at_a_character() {
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &notes());
    let mut image = Vec::new();
    // One byte, then two-byte letters: the 16th byte would be the first half of the ninth
    // letter, so the label is the first 15 bytes.
    write(src.path(), "aÆØÅÆØÅÆØÅ", &mut image).unwrap();
    assert_eq!(&image[1024 + 64..1024 + 80], "aÆØÅÆØÅÆ\0".as_bytes());
}

/// ADR 0067: the kernel mounts what the writer writes. It needs root, a loop device and the
/// kernel's EROFS, so only CI runs it (`.github/workflows/tests.yml`), and only with
/// `ODK_KERNEL_MOUNT=1`; never on the development machine, where nothing is mounted.
#[test]
#[ignore = "needs root, a loop device and the kernel's EROFS: CI runs it"]
fn the_kernel_mounts_what_the_writer_writes() {
    if std::env::var("ODK_KERNEL_MOUNT").as_deref() != Ok("1") || unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: only CI runs this, as root, with ODK_KERNEL_MOUNT=1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    make_tree(&src, &writer_tree());
    let file = dir.path().join("Notes.oma");
    fs::write(&file, fixture::signed(&written(&src))).unwrap();
    let mnt = dir.path().join("mnt");
    fs::create_dir(&mnt).unwrap();

    struct Mounted<'a>(&'a Path);
    impl Drop for Mounted<'_> {
        fn drop(&mut self) {
            let _ = std::process::Command::new("umount").arg(self.0).status();
        }
    }
    let status = std::process::Command::new("mount")
        .args(["-t", "erofs", "-o", "loop,ro"])
        .arg(&file)
        .arg(&mnt)
        .status()
        .unwrap();
    assert!(status.success(), "mount -t erofs");
    let _mounted = Mounted(&mnt);

    // Opened by name, as the kernel looks names up: a binary search over the first name of
    // each directory block, so names at the blocks' edges are the ones to try.
    for (path, item) in writer_tree() {
        if let Item::File { bytes, .. } = item {
            assert_eq!(fs::read(mnt.join(&path)).unwrap(), bytes, "{path}");
        }
    }
    assert!(mnt.join("empty-dir").is_dir());
    assert!(fs::read(mnt.join("many/entry-0300-x")).is_err());
    // And walked, as `readdir` reads it.
    assert_eq!(listing(&mnt), normalized(&writer_tree()));
    eprintln!("the kernel mounted and read the writer's image");
}

#[test]
fn a_large_inline_directory_from_mkfs_erofs_reads_as_the_writers_plain_one() {
    let mut tree = notes();
    tree.extend(
        many(300)
            .into_iter()
            .map(|(path, _)| (path, file(Vec::new(), 0o644))),
    );
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &tree);
    let ours = unpacked(&written(src.path()));
    assert_eq!(ours, normalized(&tree));
    assert_eq!(unpacked(MKFS_MANY), ours);
}

#[test]
fn a_root_too_large_for_the_first_block_moves_to_the_next_and_the_superblock_says_so() {
    // About 3 KiB of entries: inline after the inode, but not after the superblock in block 0.
    let mut tree = notes();
    for i in 0..50 {
        tree.push((
            format!("root-{i:02}-{}", "r".repeat(40)),
            file(Vec::new(), 0o644),
        ));
    }
    let src = tempfile::tempdir().unwrap();
    make_tree(src.path(), &tree);
    let image = written(src.path());
    let root_nid = u16::from_le_bytes([image[1024 + 14], image[1024 + 15]]);
    assert_eq!(root_nid, 4096 / 32, "the root's inode starts block 1");
    assert_eq!(unpacked(&image), normalized(&tree));
}
