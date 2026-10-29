# A Bundle file's image

Normative. This spec fills in what ADR 0067 leaves open: the exact EROFS layouts `odk-oma`'s reader accepts (`crates/oma/src/image/`), which are the only layouts its writer may emit. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

## Where the image is

A Bundle file is ADR 0060's 1024-byte prefix followed by the payload, and the payload is the rest of an EROFS image whose first 1024 bytes the prefix takes. The superblock is at byte 1024 of the file, and every block address counts from byte 0 of the file, so the kernel mounts the file as it is (ADR 0065). The image is exactly `blocks` × 4096 bytes, which is the whole file: `1024 + payload length`.

The image is read only from the sealed copy (ADR 0060, section 5), only after the prefix, the signature and both digests hold (section 6), and only as below. Anything else is a parser error and fails closed (section 9).

## Superblock

| Field | Accepted |
|---|---|
| `magic` | `0xE0F5E1E2` |
| `feature_compat` | only the superblock checksum (`0x1`) and mtime (`0x2`) flags, which are ignored: the signature covers every byte |
| `blkszbits` | 12 (4 KiB blocks) |
| `sb_extslots` | 0 |
| `feature_incompat` | 0 |
| `available_compr_algs` | 0 |
| `extra_devices` | 0 |
| `dirblkbits` | 0 |
| `xattr_prefix_count` | 0 |
| `packed_nid` | 0 |
| `blocks` | the file's length divided by 4096, exactly |
| `meta_blkaddr` | a block of the image |

`root_nid` names the root directory. The checksum, `inos`, the build time, the UUID and the volume label are not read. ADR 0067's writer writes a zero UUID and the application's name as the label.

## Inodes

- Compact (32-byte) inodes only. An extended inode is refused.
- Data layout plain (`0`) or inline (`2`). Plain holds the data in whole blocks from `raw_blkaddr`. Inline holds every block but the last there, and the last, its tail, right after the inode in the inode's own block. A tail that does not fit in that block is refused. A compressed layout (`1`, `3`) is refused as compressed, and any other layout as one a Bundle does not use. The format's other bits are 0.
- `i_xattr_icount` is 0.
- A directory or a regular file. A device node, a fifo, a socket and a symlink are refused by name, whether the inode or its directory entry says so. Neither setuid nor setgid, on a file or a directory.
- Ownership, link counts, `i_ino` and times are not read.

## Directories

- Each block's entries are 12 bytes each, followed by their names. The first entry's name offset says how many entries the block holds. Each name runs to the next one's offset, and the last runs to the end of the directory's data in that block or to a zero byte, after which there are only zeros.
- Names are 1 to 255 bytes, with no `/` and no zero byte. They are in strictly ascending byte order across the whole directory, the order the kernel's lookup assumes.
- `.` names the directory and `..` its parent; the root is its own parent.
- An entry's file type is its inode's: 1 for a regular file and 2 for a directory.

## The tree

- Each inode is in the tree once. There are no hard links, and a directory that holds itself or an ancestor is refused as the same.
- Nothing claims a byte anything else claims. The prefix and the superblock (bytes 0 to 1151), each inode with its inline tail, and each inode's whole blocks are all distinct. So the files an image unpacks to never add up to more than the file, and no inode sits inside another file's data.
- At most 65 536 entries, 64 directories deep, 4095 bytes in a path, and 4 MiB in one directory.

## The Manifest

`manifest.toml` is a regular file at the root of at most 64 KiB of UTF-8, and a valid Manifest (`docs/spec/manifest.md`). Its `identifier`, `executable` and `publisher` equal the signed manifest's `app_id`, `entry` and `publisher` (ADR 0060, section 4); a missing `publisher` is a mismatch. Its programs and icon pass the checks a directory Bundle's do: `executable`, and `daemon.executable` when there is one, is a regular file with an executable bit and an x86_64 ELF executable (ADR 0002), and `icon` is a regular file ending in `.png` or `.svg`.

## Unpacking

A launch writes the tree into a tmpfs at `/app` in the application's mount namespace (ADR 0067). The tmpfs's size is each file's size rounded up to 4096 bytes, added up, and at least 4096. It has one inode more than the tree's entries. Directories are `0755`. A file is `0755` when any executable bit is set in the image and `0644` when none is. Then the mount is made read-only, `nosuid` and `nodev`, and execution stays allowed.

## What the writer emits

ADR 0067's writer (`docs/oma-trust-plan.md`, phase 7) emits a subset of the above. It does not use the checksum or mtime flags. It writes compact inodes with mtime 0, owned by root. Each directory is inline when its entries fit after its inode, and every file is plain. `mkfs.erofs` 1.9.4 with `-b 4096 -T 0 --all-root -U clear -E noinline_data` writes an image the reader accepts, and the reader's tests keep one (`crates/oma/src/image/testdata/`).
