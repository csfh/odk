---
status: accepted
---

# v1 registers a Bundle file for one user, and installs no `binfmt_misc` entry

`docs/roadmap.md` (Track B, item 4) leaves one question for an ADR: how v1 registers a Bundle file with the system, given that ADR 0087 forbids a root install. `packaging/` holds three texts that nothing installs: the `binfmt_misc` line (`binfmt.d/odk.conf`), the MIME type (`mime/oma.xml`, which `install.sh` does install) and a desktop handler (`applications/oma.desktop`). The owner decides; this record recommends an answer, and it is `proposed` until the owner says so.

## What is true

- **`binfmt_misc` is root's.** The entry is `/etc/binfmt.d/odk.conf` read by `systemd-binfmt`, or a write to `/proc/sys/fs/binfmt_misc/register`, and a user cannot do either. ADR 0087 says there is no root install and no package, so `install.sh` cannot put it down, and `CLAUDE.md` forbids it on the development machine. What the entry would give is `./App.oma` run from a shell, with no `odk oma run` in front of it. Nothing else needs it.
- **The type is already registered for one user.** `install.sh` writes `~/.local/share/mime/packages/oma.xml` and rebuilds the user's MIME database, and ADR 0065 makes `application/x-oma` a sub-class of the raw disk image, so a file manager offers Disk Image Mounter for a `.oma` file, which mounts it read-only. That is deliberate: looking inside a Bundle is the file manager's feature.
- **The handler in `packaging/` is not usable as it stands.** Its `Exec=/usr/bin/odk oma run %f` names a system path an install for one user does not have (`~/.local/bin/odk`), and a `.desktop` file takes no `%h`.
- **An installed Bundle already has a launcher.** `odk oma --register` writes a `.desktop` entry for each installed Bundle, with its icon (ADR 0090), under the user's `applications` directory. That, and `odk oma run <file>`, is how a Bundle is started by a person.

## Decision

The owner accepted this on 2026-11-12, with one change to the recommendation: ODK is the default for a `.oma` file.

1. **v1 installs no `binfmt_misc` entry, and says so.** `packaging/binfmt.d/odk.conf` stays in the repository as text for an administrator who wants `./App.oma` to run, with its README line saying that it is not installed, needs root and is not supported by the installer. `odk oma run <file>` is the way to run a file from a terminal.
2. **`install.sh` installs a per-user handler for `application/x-oma`**: `~/.local/share/applications/odk-oma.desktop`, written by the script from the template in `packaging/applications/oma.desktop` with the quoted absolute path of the `odk` it just installed in its `Exec=`, so it names `odk oma run %f`. The script runs `update-desktop-database` on that directory when the tool is there and says so when it is not, as it does for `update-mime-database`.
3. **It is the default for the type.** The script puts `application/x-oma=odk-oma.desktop` first under `[Default Applications]` in the user's `mimeapps.list` (`$XDG_CONFIG_HOME`, default `~/.config`), in place of any other default for that type and touching nothing else in the file, with no `xdg-mime` and no other tool but `awk`. A double-click on a `.oma` file runs it through `odk oma run`, which for a Bundle that is not installed opens the first-install dialog (Install, Run once or Cancel) before anything runs, so the dialog says what the Bundle asks for first. A user who wants the file manager to mount it instead uses "Open With ▸ Disk Image Mounter" or `odk oma mount`, and ADR 0065's mount is unchanged. This amends ADR 0065, whose default was Disk Image Mounter.
4. **`--uninstall` removes the handler and the default line**, and only that line, and the `mimeapps.list` too when nothing else is left in it. A path with a character a `.desktop` file or the script's substitution would misread (`"`, `\`, a backtick, `$`, `|`, `&`) is refused before anything is written.
5. **The installer's test** (`crates/odk-cli/tests/install.rs`) stubs `update-desktop-database`, and checks the handler's `Exec=`, that an existing default for the type is replaced and the rest of the user's file is kept, that a second run leaves one line, and that uninstall removes ours and keeps theirs. `tools/package-release` puts the template in the tarball as `share/applications/odk-oma.desktop`; a release from before it has none, and the script then installs no handler.

## Why

A registration that needs root contradicts ADR 0087 and is what the owner has refused three times (ADR 0053, 0063 and 0087); the only thing it buys is a shell convenience that `odk oma run` already gives. The per-user handler needs no privilege and is removed with the rest. The owner chose to make it the default: a person who has installed ODK expects a double-click on a Bundle to open it, and the first-install dialog stands between the double-click and anything running. That takes the default from Disk Image Mounter (ADR 0065), whose mount stays one "Open With" or `odk oma mount` away.

## What was not checked

- Whether a file manager on Omarchy (Nautilus) honours the default for a sub-class of `application/vnd.efi.img`, and lists Disk Image Mounter beside it. This is a hand check on a real desktop, which an agent does not do (`CLAUDE.md`); the tests check the files the script writes and not what a file manager does with them.
- `update-desktop-database` is in `desktop-file-utils`, which a stock Omarchy has (read from its package lists on 2026-10-27, not re-read for this record); the script does not depend on it.

## Consequences

- `install.sh`, its test, `packaging/applications/oma.desktop`, `tools/package-release` and `packaging/README.md` changed with this acceptance, and Track B item 4 of the roadmap is closed.
- ADR 0053 and ADR 0063 are unchanged by it; ADR 0065 is amended (its default for the type).
