---
status: amended by ADR 0060 and ADR 0067
---

# A Bundle is one EROFS file

A Bundle is a single file, `<App>.oma`, not a directory. The file is an EROFS image of the directory ADR 0022 described: `manifest.toml` at the root, the x86_64 ELF at the path the Manifest names, and the icon and other resources beside them. Opening the file runs the application. Browsing it is a read-only mount of that image, with nothing extracted and nothing left behind afterwards. This supersedes ADR 0022, and it supersedes ADR 0008's tar+zstd archive, which ADR 0022 had already set aside. The layout inside the image is unchanged, so `docs/spec/manifest.md` describes the tree a mount shows. The code still installs a directory; it changes to follow this ADR, and it has not yet.

`oma bundle` (ADR 0032) is what writes the file. It still does not compile, and it still refuses to replace an existing Bundle. It can no longer leave a directory for an author to copy files into afterwards: the image is sealed when the command finishes, so every file has to be named before then. A repeated `--include <file>[=<path>]` copies extra resources in, under the same containment rules as the executable (relative, no `..`, no symlink that leaves the tree). The command stages that tree, runs `mkfs.erofs`, stamps the preamble below, and renames the result into place only if it validates. `mkfs.erofs` comes from `erofs-utils`. A pure-Rust writer was set aside: it would be a second implementation of the format, and the install of `oma` depends on the tool (ADR 0054).

The image is reproducible. `oma bundle` invokes:

```
mkfs.erofs -z zstd,9 -T 0 --all-root -U clear <image> <staged-tree>
```

`-z zstd,9` is zstd at level 9. `-T 0` fixes every timestamp to the Unix epoch (`--all-time` is that flag's default). `--all-root` sets uid and gid to 0. `-U clear` leaves no random filesystem UUID. The staged tree is oma's own, so it carries no source xattrs, and the paths are inserted in sorted order. Two runs over the same inputs and the same signing key produce the same file, preamble included. Unsigned output has a zero signature, which is deterministic too.

## Why a filesystem image

ADR 0008 wanted any tool to be able to list a Bundle without knowing `.oma`, and said a mounted image could come later without changing the layout. ADR 0022 rejected the archive because running it meant extracting into an ephemeral directory, and because Linux will not launch a directory on double-click. A single file is what the desktop can associate with `oma` (ADR 0053). An image that mounts gives both: the program runs from the mount, and a file manager that can read a directory can read the Bundle. The layout inside stays the one both of those ADRs already fixed.

The sandbox is the one `oma` builds (ADR 0005), not a `bwrap` helper. The child that sets it up is in a new user, mount and PID namespace, with the host uid mapped to itself and not to 0 (`launch.rs`), and it does have `CAP_SYS_ADMIN` in that namespace long enough to mount a tmpfs and `/proc` (ADR 0024). Seccomp then denies `mount` for the application (`docs/spec/seccomp.md`). "Without root, inside the sandbox" means a filesystem that will mount under those rules.

## Considered options

| | tar.zst plus an index | SquashFS, via squashfuse | EROFS, via erofsfuse |
|---|---|---|---|
| Random access | An index (seekable zstd frames and a tar table) jumps to a member, then a window is decompressed. An ELF mapped from that is a copy. | Block compressed, made for random reads. The `squashfuse` binary is single-threaded; `squashfuse_ll` is the threaded one. | Block aligned and aimed at random reads. erofsfuse serves it in userspace. |
| Browsing as a directory | Only a program that understands the index (ratarmount, archivemount, fuse-archive). Anything else sees an archive. | A directory, once mounted. | A directory, once mounted. |
| Mounting without root in the sandbox | No kernel driver. FUSE only, and FUSE inside this uid map is unverified (`docs/checks.md`). | The kernel driver is not `FS_USERNS_MOUNT`. Mounting one needs `CAP_SYS_ADMIN` in the initial user namespace, which this sandbox does not have. squashfuse is unprivileged FUSE in the user's own session. Inside the sandbox's user namespace it is the same unverified FUSE question. | The kernel driver is not `FS_USERNS_MOUNT` either. `fs/erofs/super.c` sets `FS_REQUIRES_DEV` and `FS_ALLOW_IDMAP` only (mainline and v6.12, read 2026-10-25). In 2024 the EROFS maintainer said setting the flag would be straightforward, and that he had no strong wish to: VFS maintainers are wary of it, and unprivileged mounts already have another way. A distro kernel cannot turn the flag on without a patch. erofsfuse is the unprivileged mount. The kernel driver remains a later optimisation that would not change the file. |
| Compression | Solid zstd. The best ratio, and the worst random access. | gzip, lzo, lz4, xz or zstd, per block. `mksquashfs` is `squashfs-tools` in extra. | lz4, lzma, deflate or zstd, per block. Close to SquashFS at the same block size, behind a solid tar. erofsfuse links libzstd, so zstd does not depend on the kernel's `CONFIG_EROFS_FS_ZIP_ZSTD`. |
| `oma bundle` | The `tar` and `zstd` crates. No extra package. | Shell out to `mksquashfs`. | Shell out to `mkfs.erofs`. |
| Arch and Omarchy | `tar` and `zstd` are already installed on a system that can install packages. | `squashfs-tools` (4.7.5-1) and `squashfuse` are in extra. Whether Omarchy's kernel enables the SquashFS driver is unchecked; the FUSE path does not need it. | `erofs-utils` and `erofsfuse` are in extra (`erofsfuse` 1.9.4-1, built 2026-08-25, depends on fuse2 and zstd). Whether Omarchy's kernel enables the EROFS driver is unchecked; erofsfuse does not need it. Omarchy is Arch, so the repositories are the same. The `oma` package depends on the userspace tools; they do not have to be installed already. |

**Decision: EROFS, mounted with erofsfuse, compressed with zstd at level 9.** The deciding fact is the on-disk layout. The EROFS superblock sits at byte 1024, and the first 1024 bytes are unused, so that a boot sector or another format can live there (EROFS on-disk specification). The superblock checksum covers the superblock, from offset 1024, and not that prefix. `oma` stamps a preamble there after `mkfs.erofs` returns:

- `OMA1` at offset 0, then the little-endian header ADR 0055 defines (version, header length, flags, algorithm, key id, public key, signature). The rest of the 1024 bytes stays zero.
- The image itself starts at offset 1024 and runs to the end of the file. There is no trailer. A trailing signature would be a second thing every mounter has to ignore; the reserved prefix is the place the format already allows.

The file is still an EROFS image, so a kernel mount and erofsfuse can open it without an offset. The magic at offset 0 is unique, which is what the MIME type matches (ADR 0053). SquashFS cannot do this. Its superblock is at offset 0 (`hsqs`), so a preamble shifts it: only `squashfuse -o offset=` can mount it, a plain kernel mount cannot, and a bare SquashFS magic is shared with every other SquashFS, so a magic match cannot mean "this is a Bundle". tar.zst has neither a reserved header nor a directory mount. That is ADR 0008's own objection to a custom container, the other way up: once the image is mounted, every tool sees a directory and none of them implements `.oma`.

Also weighed, and rejected. DwarFS compresses well and is FUSE-only; it has no reserved prefix and no place in the Arch tools the other two already have. An ext4 or btrfs image needs a loop device, and loop control is privileged. composefs is an EROFS metadata layer over a separate content store, which is not one file. An ELF stub in front of a SquashFS image is the AppImage shape; the stub is rejected in ADR 0053, and the offset image is the SquashFS column above.

Stamping the prefix and then mounting the result has not been done on a real image (`docs/checks.md`). The format does not wait on that check. If a non-zero prefix is rejected by erofsfuse or by `mkfs.erofs`, a new ADR changes the format.

## Running it

The mount is erofsfuse, run in the foreground as the user, with no extra privileges, on a fresh directory under `$XDG_RUNTIME_DIR/oma/mnt/`. The sandbox bind-mounts that directory read-only at `/app`, which is where a Bundle directory is bind-mounted today (`plan.rs`). The application executes `/app/` plus the Manifest's `executable`. It never sees `/dev/fuse` (the private `/dev` is still null, zero, full, random, urandom and tty) and seccomp never has to allow `mount`. The image file stays outside the sandbox. `~/Applications` is still not writable from inside (ADR 0015): a mounted directory does not loosen that.

The mount has to die with the application, and `oma` is not its parent and not the application's (ADR 0034). The init's job list does not grow (ADR 0024). The intermediate process starts erofsfuse, starts a helper, and exits. Both call `setsid`, as the init does, so a terminal closing does not unmount a running Bundle. The helper holds the read end of a pipe whose write end is inherited by the init and by no one else. It does nothing until that pipe closes, which is when the init exits, including when the init is killed, and then it unmounts and exits. It is orphaned the same way the init is. It does not signal, restart or decide. A setup failure after the mount unmounts before the intermediate exits, and starts no helper. `$XDG_RUNTIME_DIR` is wiped at logout, which is the backstop if the helper is killed first. One mount per launch; a second instance of the same Bundle mounts the file again. A shared mount would couple those lifetimes, and this does not.

Doing the mount inside the sandbox's own mount namespace would remove the helper: the namespace would take the mount with it. FUSE is `FS_USERNS_MOUNT`, so it is the only candidate, and it is unverified with this uid map (`docs/checks.md`). libfuse often mounts by calling `fusermount` rather than `mount(2)`, and `fusermount` from inside a user namespace whose uid is not 0 commonly fails. That has not been tried here. The session mount above is the one that ships. If the check later passes, the in-namespace mount can replace the helper, and the file format does not change.

**Run once** (ADR 0018) mounts the file where it stands, in `~/Downloads` or wherever it was opened. It copies nothing, records no consent, installs nothing, and still gets a private tmpfs for data and no `updates` permission. The helper unmounts when the application exits, so there is nothing to clean up. **Install** copies the file into `~/Applications` and then runs that copy. It does not move the original. Copying is a write to a temporary name in `~/Applications` and a rename, so a reader never sees a half-written file.

**`~/Applications`** holds `<Name>.oma` files, the name taken from the Manifest as `oma bundle` already takes it. Removing an application is deleting the file. Data, the previous version, the inbox, a held upgrade and the consent record stay under `~/.local/share/oma/` (ADR 0009, ADR 0055). A Bundle run from anywhere else still works. Installs stay per-user, and `oma` still never needs root.

**Rollback** (ADR 0011) swaps two files: the installed one and `~/.local/share/oma/previous/<identifier>.oma`. Both are on the home filesystem, so the swap is three renames through a temporary name and a crash cannot drop one of them. Data is still not snapshotted. Deleting the file from `~/Applications` drops the `.desktop` entry on the next scan and keeps the data and the previous file until `oma purge`.

**The inbox** (ADR 0015) receives one file. The application writes `<name>.oma.partial`, syncs it, and renames it to `<name>.oma`. A name that does not end in `.oma` is ignored, as a half-built directory is ignored today, and the rename is the point the path unit fires. `oma` renames the file out of the inbox into a directory only it can reach before it reads it, and accepts it only if the Identifier is the inbox's, the version is higher, the image and the Manifest are valid, and the signature verifies under the pinned key (ADR 0055). A file that fails those structural checks, or whose signature does not verify, is deleted. A file signed by any other key, and a file that cannot be checked against its publisher's domain yet, is not deleted and not installed: ADR 0055 holds it. Write access to `~/Applications` is still not a permission: one write would replace another application's whole file and run with its grants. A file the user drops into `~/Applications` by hand is an upgrade when the Identifier matches, as a dropped directory is today, subject to the same signature rule. `--register` skips a file that is not yet a valid image and does not delete it; it is the user's file, unlike an inbox reject. The login run is the backstop if the path unit fired while the copy was still short (`docs/checks.md`).

`oma inspect` and `oma --register` need `manifest.toml` without starting the application. They take a short-lived mount, read the file, and unmount. A library that can read one file out of an EROFS image would remove that mount; there is no such reader in the tree, and shelling out to an extractor would be the extraction step this format exists to avoid.

## Consequences

- ADR 0022 is superseded. ADR 0008 is superseded by ADR 0022 and by this one. ADR 0009, ADR 0011, ADR 0015, ADR 0018 and ADR 0032 are amended: a file where each of them says a directory, the same locations and the same rules. ADR 0032's command remains; "copy extra files in afterwards" does not, and the repeated `--include` above replaces it.
- The sandbox's device list, Landlock rules and seccomp denylist do not gain a mount. `/app` stays the read-only Bundle root.
- `oma bundle` and `oma run` depend on `mkfs.erofs` and `erofsfuse`. Missing, they fail with an error that names the package, and they do not start an application.
- Registration of the file with the desktop is ADR 0053. The install that places the tools and the units is ADR 0054. The bytes in the preamble that are a signature, and the rules for accepting one, are ADR 0055.
- ADR 0010 is unchanged. `oma` still does not build an application and does not fetch updates. Assembling an image of an executable someone else built is the same kind of step as copying it into a directory.

## Open questions

1. Stamp a non-zero preamble into a real `mkfs.erofs` image and mount it with `erofsfuse` (`docs/checks.md`). The layout is decided. If that mount fails, a new ADR changes it.
2. Whether `erofsfuse` can mount inside the sandbox's user namespace, with this uid map, is still unchecked (`docs/checks.md`). The session mount does not wait on it.
