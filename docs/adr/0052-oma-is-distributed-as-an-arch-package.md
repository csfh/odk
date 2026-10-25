---
status: proposed
---

# `oma` is distributed as an Arch package

The way onto a machine is an Arch package. A GitHub release tarball is the fallback for a machine that cannot install from the AUR. Both install the same programs and register the same file type (ADR 0051); the fallback cannot do the steps that need root. This is the install of `oma` and `oma-consent`. It is not an OMA runtime. ADR 0010 stands: `oma` verifies, installs, registers, sandboxes and starts Bundles, and it does not host them, update them or compile them. The Luau machine and the ODK libraries stay linked into the applications that use them.

The package is `oma`. The PKGBUILD lives in `packaging/` next to the units, and the published package is the AUR until a maintainer takes it into `[extra]` or Omarchy vendors it. We do not publish into `[extra]` ourselves. The name has not been checked against the AUR (`docs/checks.md`).

`oma` is installed at `/usr/bin/oma` and `oma-consent` at `/usr/bin/oma-consent`. ADR 0044's units assume the first path and say no ADR has decided it. This one does. A package that installed them elsewhere would rewrite `ExecStart`, and the fallback below does that rewrite because it has to.

## What the package installs

| Path | What it is |
|---|---|
| `/usr/bin/oma` | The program. |
| `/usr/bin/oma-consent` | The consent dialog (ADR 0025, ADR 0033). |
| `/usr/lib/oma/binfmt` | The `binfmt_misc` interpreter: `exec /usr/bin/oma run -- "$1"` (ADR 0051). |
| `/usr/lib/binfmt.d/oma.conf` | Magic `OMA1` at offset 0, interpreter the line above. |
| `/usr/share/mime/packages/oma.xml` | `application/x-oma` (ADR 0051). |
| `/usr/share/applications/oma.desktop` | The handler, `NoDisplay=true`, with the Show-contents action. |
| `/usr/lib/systemd/user/oma-omarchy.path` and `.service` | The theme-copy units (ADR 0044). |
| `/usr/lib/systemd/user/oma-register.service` | Login reconcile (ADR 0051). |
| `/usr/lib/systemd/user/oma-applications.path` | Watches `~/Applications`. |
| `/usr/lib/systemd/user/oma-inbox@.path` | Template. Not enabled globally. `--register` enables instances. |

Dependencies: `erofs-utils` and `erofsfuse` (ADR 0050), `zstd`, `shared-mime-info`, `desktop-file-utils`, `systemd`. `fuse2` comes in with `erofsfuse`. No GPUI library is bundled. `oma-consent` links GPUI the way the repository already builds it.

The package does not create `~/Applications`, does not write into any home, and does not start a daemon. The user oneshot creates the directory at login.

## Install

From the PKGBUILD's install script, in this order:

1. Pacman has already copied the files above.
2. `update-mime-database /usr/share/mime`. Arch's `shared-mime-info` hook may do this when a file appears under `/usr/share/mime/packages`; the script runs it as well so the type exists before the next step, and a second run is harmless.
3. `update-desktop-database /usr/share/applications`, with the same remark about the `desktop-file-utils` hook.
4. Add `application/x-oma=oma.desktop` to `[Default Applications]` in `/usr/share/applications/mimeapps.list`, creating the file if it is absent (ADR 0051). Leave every other line.
5. `systemctl --global enable oma-omarchy.path oma-omarchy.service oma-register.service oma-applications.path`.
6. `systemctl restart systemd-binfmt` so the new magic is live without a reboot. If the unit is not running, the boot will load the file.

None of this enables an inbox instance. There is no installed Bundle yet.

## Uninstall

1. `systemctl --global disable oma-omarchy.path oma-omarchy.service oma-register.service oma-applications.path`.
2. Remove the `application/x-oma` line from `mimeapps.list`, and remove the file if it is then empty of entries.
3. Pacman removes the files it installed, including the `binfmt.d` snippet and the inbox template.
4. `update-mime-database /usr/share/mime` and `update-desktop-database /usr/share/applications`.
5. `systemctl restart systemd-binfmt`, so the magic is gone.

The script does not walk home directories. It does not delete `~/Applications`, `~/.local/share/oma` (data, previous versions, inboxes, consent records, the Omarchy copy) or the generated `.desktop` files under `~/.local/share/applications/`. Those are the user's. An entry whose `Exec` is `oma` fails until the package is installed again; `--register` on the next install reconciles them. Inbox instances a user session enabled (`oma-inbox@*.path`) are left behind and fail because the unit file and the binary are gone. Reinstalling and running `--register` enables the ones that still have an inbox and the dead ones stay disabled once that reconcile runs. The script does not call `systemctl --user` for every uid.

## The GitHub fallback

A release tarball holds the two binaries for Linux x86_64, `binfmt`'s script, the unit files, `oma.xml`, `oma.desktop` and two scripts, `install-user` and `uninstall-user`. `install-user`:

1. Copies `oma` and `oma-consent` to `~/.local/bin/`.
2. Copies the units to `~/.config/systemd/user/`, with `ExecStart` rewritten to `~/.local/bin/oma`.
3. `systemctl --user daemon-reload` and `systemctl --user enable --now` of the same four units the package enables globally.
4. Installs `oma.xml` to `~/.local/share/mime/packages/` and runs `update-mime-database ~/.local/share/mime`.
5. Installs `oma.desktop` to `~/.local/share/applications/` and runs `update-desktop-database ~/.local/share/applications`.
6. Sets the default with `xdg-mime default oma.desktop application/x-oma`, which writes the user's `mimeapps.list`.

It does not register `binfmt_misc`, does not install `erofs-utils`, and does not touch `/usr`. `oma bundle` and `oma run` still need `mkfs.erofs` and `erofsfuse` on `PATH`; the script says so and does not run `pacman`. `uninstall-user` reverses the copies, the `systemctl --user disable --now`, the user MIME and desktop files and the `xdg-mime` line, and deletes nothing under `~/Applications` or `~/.local/share/oma`.

The package is the supported install. The tarball is for a machine without the AUR. CI does not build either until a release exists; both are packaging, and this ADR does not cut a release.

## Consequences

- ADR 0044 is amended: `/usr/bin/oma` is the path. The two units it already describes are unchanged, and the package enables them the way that ADR already asked.
- `docs/spec/oma-cli.md` gains `oma mount` when this and ADR 0051 are accepted. It does not gain `update`, `list` or `uninstall` of a Bundle. Uninstalling an application is still deleting its file (ADR 0009, ADR 0050). Uninstalling `oma` is removing the package.
- A sandboxed application still sees `oma`'s copy of the text size and the font (ADR 0044). The package does not put the user's real `~/.config/omarchy` into the sandbox.
- End-to-end tests that run a private `oma` with no `oma-consent` beside it are unchanged. The package installs the helper; a test tree does not have to.

## Open questions

1. Is the AUR the right place until Omarchy vendors the package, and is the name `oma` free there?
2. Is `/usr/bin/oma` and `/usr/bin/oma-consent` the path to fix?
3. Are leftover inbox units and generated `.desktop` files on uninstall acceptable, or should uninstall refuse to finish while any Bundle is installed?
4. Is a GitHub tarball wanted before 1.0, and should the release CI build it?
