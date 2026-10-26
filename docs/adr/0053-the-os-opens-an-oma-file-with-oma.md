---
status: accepted
---

# The OS opens an `.oma` file with `oma`

Installing `oma` registers the file from ADR 0052 with the desktop, so opening it runs the Bundle and a file manager can mount it as a directory. The per-application `.desktop` entries of ADR 0014 stay: each installed Bundle still gets an entry under `~/.local/share/applications/` whose `Exec` is `oma run` plus the Identifier, never the ELF, and the entry goes away when the file disappears from `~/Applications`. What changes is the sentence in ADR 0014 that registered no MIME type, which was true because a directory cannot take a file-name association. This amends ADR 0014. It also amends the part of ADR 0009 that enables the path unit on first use. The units are installed with `oma` (ADR 0054) and enabled for every user, which is the pattern ADR 0044 already uses for the theme copy. No Omarchy hook is installed; none fires when an application is removed (ADR 0014). The launcher refreshes its own list when a `.desktop` file changes (`docs/checks.md`).

The code still installs a directory and still registers no MIME type. This ADR is the behaviour it moves to.

## The MIME type

`shared-mime-info` type `application/x-oma`, in `/usr/share/mime/packages/oma.xml`:

- comment: Omarchy application bundle
- glob: `*.oma`
- magic: the four bytes `OMA1` at offset 0, priority 80, so a magic match beats a generic one

The glob is what a file named `.oma` needs. The magic is what distinguishes it from a renamed archive, and it is why the preamble exists (ADR 0052). A bare EROFS magic at offset 1024 is every EROFS image, not a Bundle. `update-mime-database /usr/share/mime` runs when the package is installed and when it is removed. The type is `application/x-oma`. A vendor name such as `application/vnd.omarchy.oma` would want a registration this project does not have, and it is not the type.

## The handler

`/usr/share/applications/oma.desktop` is the handler for the file, and it is not an application in the launcher:

```
[Desktop Entry]
Type=Application
Name=oma
Comment=Run an Omarchy application bundle
Exec=oma run %f
MimeType=application/x-oma;
NoDisplay=true
Terminal=false
Actions=show-contents;

[Desktop Action show-contents]
Name=Show contents
Exec=oma mount --open %f
```

`NoDisplay=true` keeps `oma` itself out of the application list. The entries users launch are the per-Bundle ones ADR 0014 generates. `Exec=oma run %f` is the same command Flea and a downloaded file already need: a path that is not inside `~/Applications` gets Install, Run once or Cancel (ADR 0018), and a path that is an installed Bundle is governed by the consent record (ADR 0023) and by the signature rule (ADR 0055). The sandbox still applies, because the desktop entry never executes the ELF.

The default handler is one line in `/usr/share/applications/mimeapps.list`, under `[Default Applications]`:

```
application/x-oma=oma.desktop
```

The package does not own that file. Install adds the line if it is missing, creating the file with only that section if there is no file yet. Uninstall removes the line, and removes the file if nothing else is left in it. A desktop that ignores the system list still has the `MimeType=` key, so `oma` is offered as a handler; the line is what makes it the default without a per-user `xdg-mime` command. Whether anything else on Omarchy already ships `mimeapps.list` is unchecked (`docs/checks.md`).

## Path units

Two units join the two ADR 0044 already ships, all under `/usr/lib/systemd/user/`, all with `ExecStart=/usr/bin/oma` (ADR 0054):

- `oma-register.service`, a oneshot, `WantedBy=default.target`. It runs `oma --register`. That command creates `~/Applications` if it is missing, then reconciles: it writes `.desktop` entries for valid `.oma` files, removes entries whose files are gone, accepts inbox upgrades, and enables or disables inbox watches. It is idempotent (ADR 0009). It does not open the consent dialog. A file ADR 0055 will not apply on its own is left held, and the oneshot logs that and continues.
- `oma-applications.path`, `PathChanged=%h/Applications`, `Unit=oma-register.service`, `After=oma-register.service`, also `WantedBy=default.target`. The `After=` is so the path unit watches a directory the oneshot has already created. ADR 0009 found that a path unit cannot watch a directory that is not there. Whether `After=` on a oneshot is enough at first login is unchecked (`docs/checks.md`).

The inbox template `oma-inbox@.path` is unchanged and is not enabled globally. `--register` enables one instance per installed Identifier, because a path unit does not watch a grandchild and each inbox is its own directory (ADR 0015). A file rename into the inbox fires it, which is the `.oma.partial` to `.oma` publish of ADR 0052.

`systemctl --global enable` of `oma-register.service` and `oma-applications.path` is part of installing the package, together with the two ADR 0044 units. There is nothing for the user to run, and `oma` still does not stay resident.

## Browsing

Double-click and "Open" run the Bundle, through the handler above. "Show contents" mounts it.

`oma mount <file>` mounts the file read-only with erofsfuse, in the user's session, at `$XDG_RUNTIME_DIR/oma/view/<identity>`, where the identity is the file's device and inode so a second request for the same file reuses the mount. It prints the directory. `oma mount --open <file>` does that and then opens the directory with the default file manager (`xdg-open` of a directory, not of the `.oma` file, so it does not loop back into `oma run`). `oma mount --unmount <file>` unmounts that view. A view lasts until that unmount or until the runtime directory is removed at logout. It is not tied to a file manager exiting, and it is not the per-launch mount ADR 0052 uses to run one.

Flea calls `oma` itself, as ADR 0014 already has it do for launching. It shows the Bundle as one application, runs it on open, and calls `oma mount` for "Show contents" rather than parsing the image. The desktop Action is the same operation for a file manager that shows Actions. File managers that only have "Open" will run the Bundle, which is the default. Which file manager Omarchy ships beside Flea, and whether that one shows a desktop Action, is unchecked.

The mount is a directory of the image. Nothing is copied out. A program that reads the directory sees `manifest.toml`, the ELF and the resources, and cannot write them.

## Making the file executable

Opening the file does not require the executable bit. A file manager opens by MIME type, and the handler is the installed `oma`, which is root-owned and is not bytes from the Bundle. That is the path that has to work. Three ways to make `./App.oma` run from a shell were weighed.

- **`chmod +x` alone.** The kernel will not execute an EROFS image. The bit does nothing without a shebang or a `binfmt_misc` entry.
- **A shebang in the 1024-byte prefix.** The kernel runs whatever interpreter the file names, before `oma` can look at a signature. A file whose first line is `#!/bin/sh` is a shell script that happens to be named `.oma`. The Bundle chooses the interpreter.
- **An ELF stub**, in the prefix or prepended as AppImage does. The kernel maps and runs bytes from the file. A signature the stub then checks is a check the attacker gets to skip, because the stub is the part they replace. Trust has to sit in a program the Bundle does not contain.

**Decision: a `binfmt_misc` entry, shipped by the system install.** That install (ADR 0054) writes `/usr/lib/binfmt.d/oma.conf`, matching magic `OMA1` at offset 0, with the interpreter `/usr/lib/oma/binfmt`. That interpreter is a root-owned script whose only job is `exec /usr/bin/oma run -- "$1"`. The kernel therefore runs the installed `oma`, the public command stays `oma run`, and a file that merely begins with the magic is validated like any other Bundle before anything from it executes. `systemd-binfmt` picks the file up at install. The executable bit is still required, so a download is not run by accident; a double-click uses the MIME handler either way. The per-user install of ADR 0054 cannot register `binfmt_misc`, because that needs root, and those installs get the MIME handler only.

This is the only one of the three that does not run the Bundle's own bytes to reach `oma`.

## Consequences

- ADR 0014 is amended, not superseded. Generated entries, `Exec` by Identifier, and no Omarchy hooks all stand. The MIME type, the handler and "Show contents" are new.
- ADR 0009 is amended. `~/Applications` holds files (ADR 0052). The path unit is enabled for every user when `oma` is installed, and the login oneshot creates the directory, instead of `oma` enabling the unit on first use.
- A Bundle that is not valid is not given an entry. `--register` skipping a short copy (ADR 0052) is what stops a half-written drop from appearing in the launcher.
- The consent dialog, the sandbox and the init are unchanged. The handler only chooses `oma run`.

## Open questions

1. Which file manager Omarchy ships besides Flea, and does it show a desktop Action (`docs/checks.md`)? If it does not, Flea's own "Show contents" is the browsing path that is designed here, and the desktop Action stays for those that do.
