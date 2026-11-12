---
status: proposed
---

# v1 registers a Bundle file for one user, and installs no `binfmt_misc` entry

`docs/roadmap.md` (Track B, item 4) leaves one question for an ADR: how v1 registers a Bundle file with the system, given that ADR 0087 forbids a root install. `packaging/` holds three texts that nothing installs: the `binfmt_misc` line (`binfmt.d/odk.conf`), the MIME type (`mime/oma.xml`, which `install.sh` does install) and a desktop handler (`applications/oma.desktop`). The owner decides; this record recommends an answer, and it is `proposed` until the owner says so.

## What is true

- **`binfmt_misc` is root's.** The entry is `/etc/binfmt.d/odk.conf` read by `systemd-binfmt`, or a write to `/proc/sys/fs/binfmt_misc/register`, and a user cannot do either. ADR 0087 says there is no root install and no package, so `install.sh` cannot put it down, and `CLAUDE.md` forbids it on the development machine. What the entry would give is `./App.oma` run from a shell, with no `odk oma run` in front of it. Nothing else needs it.
- **The type is already registered for one user.** `install.sh` writes `~/.local/share/mime/packages/oma.xml` and rebuilds the user's MIME database, and ADR 0065 makes `application/x-oma` a sub-class of the raw disk image, so a file manager offers Disk Image Mounter for a `.oma` file, which mounts it read-only. That is deliberate: looking inside a Bundle is the file manager's feature.
- **The handler in `packaging/` is not usable as it stands.** Its `Exec=/usr/bin/odk oma run %f` names a system path an install for one user does not have (`~/.local/bin/odk`), and a `.desktop` file takes no `%h`.
- **An installed Bundle already has a launcher.** `odk oma --register` writes a `.desktop` entry for each installed Bundle, with its icon (ADR 0090), under the user's `applications` directory. That, and `odk oma run <file>`, is how a Bundle is started by a person.

## Decision

1. **v1 installs no `binfmt_misc` entry, and says so.** `packaging/binfmt.d/odk.conf` stays in the repository as text for an administrator who wants `./App.oma` to run, with its README line saying that it is not installed, needs root and is not supported by the installer. `odk oma run <file>` is the way to run a file from a terminal.
2. **`install.sh` installs a per-user handler for `application/x-oma`**: `~/.local/share/applications/odk-oma.desktop`, written by the script with the absolute path of the `odk` it just installed in its `Exec=`, so it names `odk oma run %f`. It is **offered, not made the default**: ADR 0065's default (Disk Image Mounter) stays, so a double-click on a file in a file manager still shows what is inside, and "Open With ▸ odk" opens the Bundle's first-install dialog or runs it (`flow::open_file`). A user who wants a double-click to run can choose it once; ODK does not change `mimeapps.list`. The script runs `update-desktop-database` on that directory when the tool is there, and says so when it is not, as it does for `update-mime-database`.
3. **`--uninstall` removes the handler**, exactly as it removes the other files, and the installer's test (`crates/odk-cli/tests/install.rs`) stubs `update-desktop-database` and checks the file's `Exec=` names the installed `odk`.
4. **The system-path handler in `packaging/applications/oma.desktop` is replaced** by a template with a placeholder the script fills in, so the text in the repository and the file installed cannot drift.

## Why

A registration that needs root contradicts ADR 0087 and is what the owner has refused three times (ADR 0053, 0063 and 0087); the only thing it buys is a shell convenience that `odk oma run` already gives. The per-user handler needs no privilege, is removed with the rest, and leaves ADR 0065's choice alone: a Bundle file is a disk image first, because that is what a file manager can do with it without running anything the user has not agreed to. Making ODK the default would turn a double-click into running a program from a file the user may only have wanted to look at, before the first-install dialog has said what it asks for.

## What was not checked

- Whether a file manager on Omarchy (Nautilus) lists a user-level handler for a sub-class of `application/vnd.efi.img` beside Disk Image Mounter, or hides it behind the parent type's handlers. This is a hand check on a real desktop, which an agent does not do (`CLAUDE.md`); until it is done the decision is `proposed`.
- `update-desktop-database` is in `desktop-file-utils`, which a stock Omarchy has (read from its package lists on 2026-10-27, not re-read for this record); the script does not depend on it.

## Consequences

- If accepted: `install.sh`, its test, `packaging/applications/oma.desktop` and `packaging/README.md` change, and the "registration" row of `docs/checks.md` and Track B item 4 of the roadmap are closed by that change.
- ADR 0053 and ADR 0063 are unchanged by it; ADR 0065 is not amended, since its default is kept.
