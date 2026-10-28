---
status: accepted
---

# odk-oma writes and reads a Bundle's image itself

ADR 0052 made a Bundle one EROFS file, written by `mkfs.erofs` and mounted for every launch by `erofsfuse`. Both come from `erofs-utils`. On 2026-10-27 the owner ruled that ODK depends on nothing a stock Omarchy does not already install. A stock Omarchy has the kernel's EROFS driver, `udisks2` and `fuse3`, and not `erofs-utils` or `fuse2` (ADR 0065, `docs/checks.md`). So `odk-oma` writes and reads the image itself, in Rust. The owner left the mechanism to this record.

## Decision

**The format is unchanged.** A Bundle is ADR 0052's EROFS image behind ADR 0060's 1024-byte prefix, and the superblock is at byte 1024. A file manager still opens it as a disk image, through the kernel and udisks (ADR 0065).

**`odk oma bundle` writes the image.** No program is run. The writer is deterministic. The same tree and the same key give the same bytes, as ADR 0052's `-T 0 --all-root -U clear` did:

- 4 KiB blocks;
- every inode owned by root, with mtime 0;
- a zero UUID;
- the volume label is the application's name, cut to 16 bytes (ADR 0065);
- directories and regular files only, and a symlink inside the tree is written as the file it names;
- the executable bit kept, and no extended attributes.

This first writer stores files uncompressed, which the kernel and the reader below both read. Compression is a later ADR. Omarchy's kernel reads zstd, but the reader would have to as well.

**`odk-oma` reads the image.** It reads only the sealed object (ADR 0060 section 5), only after the prefix verifies (section 6), and only the layouts this writer emits. Anything else is a parser error and fails closed (section 9): another layout, a compressed file, an extended attribute, a device node, a fifo, a socket, a symlink, a setuid or setgid file, or a size past the header's payload length. `inspect`, `--register` and consent read `manifest.toml` this way, with no mount. ADR 0052 wanted exactly this reader.

**A launch unpacks the image into the sandbox.** No image is mounted:
- While the sandbox's mounts are made (`plan.rs`, `mounts.rs`), `odk-oma` mounts a tmpfs at `/app` in the application's own mount namespace, sized to the image's files.
- It writes the verified tree into that tmpfs, then makes it read-only, `nosuid` and `nodev`. Execution stays allowed.
- The sealed object is closed once the copy is made.

The tmpfs belongs to that namespace and goes when the namespace does. There is no session mount, no `$XDG_RUNTIME_DIR/oma/mnt/` and no unmount helper. ADR 0024's init gains no job, and ADR 0034's intermediate process stays. Run once is the same. Nothing reaches a host filesystem.

## What this costs

- A running Bundle holds its unpacked files in memory, once per instance. tmpfs can be swapped.
- A launch reads and writes the whole tree. How long that takes for a representative Bundle is a measurement for `docs/checks.md`.
- Bundles are not compressed yet, so they are bigger to download and to keep.
- ODK keeps an EROFS writer and reader of its own, for the layouts it emits. ADR 0052 set that aside as "a second implementation of the format". The owner's rule outweighs that.

## What was rejected

- **`erofs-utils`.** It is not on a stock Omarchy, and a no-root install (ADR 0063) cannot add it.
- **`odk-oma` as a FUSE server**, mounted inside the sandbox's user namespace. It copies nothing. But it needs a server process alive for the whole run, `/dev/fuse` inside the namespace, and FUSE under this uid map, which is unverified (`docs/checks.md`). If the server dies, every read in the application fails.
- **The kernel's EROFS, through udisks**, as views are mounted (ADR 0065). It works only for the active session, `udiskie` shows every launch as a drive, and the mount is the whole session's. A launch mount has to allow execution, so a verified image would be runnable outside the sandbox while it lasts. Views are mounted `noexec` to avoid exactly that.
- **Another format.** Only an image the kernel can mount keeps ADR 0065's view, and EROFS keeps ADR 0060's prefix.

## Consequences

- ADR 0052 is amended:
  - `mkfs.erofs`, `erofsfuse`, the session mount and the unmount helper are replaced by the above;
  - its consequence that `oma bundle` and `oma run` depend on those tools is gone;
  - its sentence that nothing is extracted becomes: nothing is extracted onto the host;
  - zstd at level 9 waits for the compression ADR.
- ADR 0060 section 6 is amended. The image checks are this reader's, and `erofsfuse` is not part of a launch. The order is unchanged: the prefix verifies first, then the image is read.
- ADR 0063's note that the single-file Bundle will need `erofs-utils` no longer holds.
- `docs/oma-trust-plan.md` phases 3 and 7 (issues [#12](https://github.com/csfh/odk/issues/12) and [#16](https://github.com/csfh/odk/issues/16)) follow this record.
- Checks for `docs/checks.md`:
  - the kernel mounts what the writer writes, in CI, where the runner has root and a loop device;
  - on the development machine, `odk oma mount` mounts it too, run live only with the owner's agreement;
  - the time and memory of a launch.

  The writer is tested against the reader, and the reader against images built by hand in tests.
