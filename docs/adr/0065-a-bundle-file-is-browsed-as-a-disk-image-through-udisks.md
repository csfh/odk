---
status: accepted
---

# A Bundle file is browsed as a disk image, through udisks

The owner decided on 2026-10-27: looking inside a `.oma` file is a feature of the file managers, as opening a disk image is, and ODK provides the tooling for it rather than a browser of its own. Omarchy ships no `erofsfuse`, which ADR 0053 mounted a view with, and no `fuse2`, which `erofsfuse` needs; it does ship what a desktop mounts a disk image with. This record is how a Bundle file uses that, and what it changes in ADR 0053 and ADR 0063. Running a Bundle is not part of it: ADR 0052's per-launch mount stands as it is.

## What a stock Omarchy has

Read on the development machine on 2026-10-27 (`docs/checks.md`), and from Omarchy's package lists (`omarchy-base.packages`):

- `udisks2` and `polkit`, through `gnome-disk-utility` and Nautilus. Polkit lets the active local session set up a loop device and mount a filesystem without a password (`implicit active: yes`); anything else asks for an administrator.
- `gnome-disk-image-mounter`, Disk Image Mounter, registered for raw disk images (`application/vnd.efi.img`) and the default for them.
- `udiskie --automount`, started at login, which mounts any new filesystem it is told about, a loop device included.
- The kernel's EROFS driver, as a module (`CONFIG_EROFS_FS=m`, `CONFIG_EROFS_FS_ZIP_ZSTD=y`).
- Not `erofs-utils`, `erofsfuse`, `fuse2` or `squashfuse`.

A loop device of a Bundle file, made by `udisksctl`, is recognised as EROFS with its label even when the first 1024 bytes are `OMA1` and noise, and the kernel mounts it read-only. That was the open question about the preamble for a kernel mount (ADR 0052): the kernel and `libblkid` ignore those bytes.

## Decision

**The type.** `application/x-oma` is a sub-class of `application/vnd.efi.img`, the raw disk image. A file manager offers what it offers for a disk image, so "Open With" lists Disk Image Mounter, which mounts the file read-only, and the mount appears beside the user's drives with a button to eject it. Nothing in ODK has to run for that.

**The command.** `odk oma mount <file>` is the same operation from ODK, for a terminal, for Flea and for a desktop Action (ADR 0053):

1. It reads the file's first 1028 bytes and refuses it unless they hold `OMA1` at byte 0 and the EROFS magic at byte 1024. It is not a general disk-image mounter.
2. A loop device whose backing file is this file, by device and inode, is the view already made, by this command or by Disk Image Mounter, and is used again. Otherwise `udisksctl loop-setup --read-only --no-partition-scan` makes one.
3. `udisksctl mount -o noexec` mounts it, and the directory is what `/proc/self/mountinfo` says, not what the tool prints. The automounter may mount a new loop device first, without `noexec`; a mount of the device without `noexec` is unmounted and mounted again with it.
4. The loop device is set to clear itself (`org.freedesktop.UDisks2.Loop.SetAutoclear`, with `gdbus`), so an unmount from the file manager also takes the loop device away.
5. It prints the directory. `--open` then opens it with `xdg-open`, and `--unmount` unmounts every view of the file and deletes its loop devices.

Every call asks udisks for no interaction (`--no-user-interaction`, or `auth.no_user_interaction` for `gdbus`), so outside the active session it fails and says that udisks allows this only there, rather than asking for an administrator's password. `LC_ALL=C` is set for them, and nothing they print is parsed.

**`noexec`.** A view shows the application's ELF. It is mounted `noexec` so that nothing in it runs outside the sandbox, which is what keeps the file inert (ADR 0060 on pull request #9): the view verifies no signature, and needs to verify none, because nothing in it executes. Disk Image Mounter and `udiskie` mount without `noexec`, and ODK does not change them. A view made by those tools is mounted again with `noexec` the next time `odk oma mount` is asked for it, and not otherwise.

**The install.** `install.sh` puts the type in `${XDG_DATA_HOME:-~/.local/share}/mime/packages/oma.xml` and runs `update-mime-database` on that directory when it is installed; `--uninstall` removes it and runs it again. This file follows `XDG_DATA_HOME`, as the units follow `XDG_CONFIG_HOME`. No handler is installed and `mimeapps.list` is not written: ADR 0063's reason for installing neither was that a handler that only refuses would change the user's default for nothing, and a type alone sets no default.

**Double-click.** With no handler of ODK's, the default for `application/x-oma` is the parent type's, Disk Image Mounter (`gio mime application/x-oma`), so double-clicking a Bundle file mounts it. That holds until running a single file is built and its handler is installed (ADR 0053); then double-click runs it, and the disk-image entries stay in "Open With". "Open With" also lists Disk Image Writer, which offers to write the file onto a disk; it asks for the disk and for authentication first.

## Considered Options

- **`erofsfuse`, as ADR 0053 had it.** In userspace and not tied to the active session, but neither it nor `fuse2` is on a stock Omarchy, and a no-root install (ADR 0063) cannot add them.
- **A FUSE server of ODK's, through the stock `fusermount3`.** Keeps parsing out of the kernel and needs no session, at the cost of an EROFS reader in Rust and a server per view. A file manager would know nothing of it: the mount would be a FUSE directory, not a disk image with an eject button.
- **gvfs's archive backend.** Not installed (`/usr/lib/gvfsd-archive`), and `libarchive` does not read EROFS.
- **systemd's `mountfsd`.** Installed and disabled; it needs root to enable and mounts only images it can verify.

The cost of this one is that the kernel parses the image, as it does any disk image or USB stick the desktop mounts. That is acceptable for a view a person asks for. It is not acceptable for running a Bundle, so it is not how one runs.

## Conflicts with earlier records

- ADR 0053 is amended. Its `oma mount` was `erofsfuse` at `$XDG_RUNTIME_DIR/oma/view/<identity>`, lasting until logout. It is now udisks: the directory is the one udisks chooses under `/run/media/<user>/`, named by the image's label, and a view lasts until it is unmounted or ejected, or the machine stops; logout does not end it. The identity, the file's device and inode, is the same, and so are `--open` and `--unmount`.
- ADR 0063 is amended: the install registers the MIME type, and still no handler.

## Consequences

- `odk oma mount` needs `udisks2` and `glib2` (for `gdbus`), both on a stock Omarchy; missing, it names the package.
- A Bundle with no label is shown by udisks under its UUID or as a volume of its size. `odk oma bundle` should pass `mkfs.erofs -L` with the application's name, at most 16 bytes in EROFS; it does not yet, because it does not write the file yet (ADR 0052).
- Nothing makes a Bundle file yet, so the command and the type have only been tried on an image made by hand with `mkfs.erofs` (`docs/checks.md`).
