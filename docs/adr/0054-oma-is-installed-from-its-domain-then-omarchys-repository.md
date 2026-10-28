---
status: amended by ADR 0063
---

# `oma` is installed from its domain, then from Omarchy's repository

`oma` has to be on the machine before any signed Bundle can be trusted, because `oma` is the program that checks the signature (ADR 0055). That install is its own trust decision, made before a Bundle exists. The first way on is a shell script fetched from `oma`'s own domain. Later, Omarchy carries `oma` in its own package repository, and that repository's signing key takes over as the trust anchor for the program. The AUR is not a route, and a GitHub release tarball is not a fallback. This replaces both.

This is the install of `oma` and `oma-consent`. It is not an OMA runtime. ADR 0010 stands: `oma` verifies, installs, registers, sandboxes and starts Bundles, and it does not host them, update them or compile them. The Luau machine and the ODK libraries stay linked into the applications that use them. Replacing `oma`'s own files is not an application update, and this ADR is the only place that happens.

Nothing here is built. The one-liner below is the decision, not a script in the tree.

## What other installers do

Four bootstraps were read, and each one is HTTPS to a known host plus a later mechanism that no longer trusts a fresh script.

- **rustup** documents `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`. The script is trusted as TLS to that host. The book also tells you to download `rustup-init` and run it yourself. Once `rustup` is installed, it verifies toolchain manifests. The shell script is not the long-term verifier.
- **Homebrew** fetches `install.sh` from the Homebrew repository on GitHub. The script prints what it will change and pauses for a confirmation it reads from the terminal, so a pipe does not skip the pause. Formulae then carry SHA-256 checksums of their downloads. The script's own trust is GitHub over TLS.
- **Deno** documents `curl -fsSL https://deno.land/install.sh | sh`, and also a SHA-256 of that script on the install repository, checked before running. Both URLs belong to Deno. The checksum catches a script that does not match the published file. It does not survive the owner of both URLs being the attacker.
- **Tailscale** documents `curl -fsSL https://tailscale.com/install.sh | sh`. The script reads `/etc/os-release`, installs Tailscale's package repository and its signing key, and lets apt, dnf or pacman install the package. After that, upgrades are the package manager checking that key. The script is the bootstrap, not the thing that runs again for every upgrade.

The shape here is Tailscale's end state, rustup's honesty about the first hop, and Homebrew's pause. Deno's same-owner checksum is not a second root, and it is not copied as one.

## The install flow

The documented command is:

```
curl --proto '=https' --tlsv1.2 -fsS https://<oma-domain>/install.sh | sh
```

`--proto '=https' --tlsv1.2` refuses a downgrade. There is no `-L`: a redirect is a failed fetch, the same rule ADR 0055 uses for the key document. The hostname is not chosen yet (`docs/checks.md`). The path is `/install.sh`.

The user is agreeing to trust that domain over TLS by running the command. That is the first-install consent for `oma` itself. There is no Bundle and no consent dialog yet, because the dialog is `oma-consent`, which this install is what delivers.

The script then does only the steps in the next section, in order, and stops at the first failure. It prints each step as it takes it, and a summary when it finishes.

A careful install does not pipe. The same URL, saved and read first:

```
curl --proto '=https' --tlsv1.2 -fsS -o install.sh https://<oma-domain>/install.sh
sh install.sh
```

Both end in the same script. The pipe is the short command. The saved file is the one a person can read. The script is short on purpose so that reading it is the whole review.

## What the script does

POSIX `sh`. It does not need bash. It checks each command's status and stops on the first failure.

1. **OS and arch.** `uname -s` must be `Linux` and `uname -m` must be `x86_64` (ADR 0002). Anything else stops, and nothing is downloaded. The machine must be Arch or Omarchy: `/etc/arch-release` exists, or `/etc/os-release` has `ID=arch`, `ID=omarchy`, or an `ID_LIKE` list that contains `arch`. Omarchy has used `ID=arch`, and Omarchy 4 is reported as `ID=omarchy` (fleetdm, 2026). Both match. Another distribution stops.
2. **Existing install.** If `oma` is already on `PATH` and its receipt says this domain installed it, the script does not copy files itself. It runs `oma self-upgrade` and exits with that command's status. The handoff is the first thing a reader sees after the OS check. It is not a boundary against a domain that serves a different script: a script that skips the handoff is a different program, and piping it trusts the domain again, as on a first install. The supported upgrade is `oma self-upgrade`, which is the binary already on disk.
3. **Mode.** The default is system-wide, under `/usr`, because `binfmt_misc` and the default MIME handler need root (ADR 0053). Root runs it directly. Anyone else uses `sudo` for that install. `--user` installs under the home directory and does not call `sudo`. `--yes` skips the pause in step 5; without a terminal and without `--yes` the script exits before it writes. `--yes` as a non-root user fails if `sudo` cannot run.
4. **Keys.** Fetch `https://<oma-domain>/.well-known/oma-keys` with the rules in ADR 0055: HTTPS only, no redirects, a non-200 or an unparseable body is a failure, three-second timeout on this fetch. The body is that ADR's version-1 document. This fetch is not optional. If it fails, the script stops and installs nothing. An unconfirmed `oma` would be the verifier for every later Bundle, so there is no unsigned fallback here. A Bundle may still be installed unconfirmed (ADR 0055). `oma` itself may not.
5. **The plan, then a pause.** Before any download of a release, the script prints the domain, the mode, the key id and the paths it will write. If `/dev/tty` can be opened, it waits for the user to press Enter. `curl | sh` still has a terminal, and the prompt is read from `/dev/tty`, not from standard input, which is the script. No terminal and no `--yes`: it exits without installing. Homebrew's installer pauses the same way.
6. **Release.** Fetch `https://<oma-domain>/oma/releases/current.json` and `https://<oma-domain>/oma/releases/current.json.sig` from the same host, again with no redirects. `current.json.sig` is one line, the standard base64 of a 64-byte ed25519 signature over the exact bytes of `current.json`. The script tries each key in the well-known document. Exactly one must verify the signature, and that key is the one this install pins. None, or more than one, stops the install. OpenSSL 3 does the check (`pkeyutl`, Ed25519). Missing `openssl`, the script stops and names `pacman -S openssl`. It does not install `oma` and then ask `oma` to verify itself.

   `current.json`:

   ```json
   {
     "version": 1,
     "oma_version": "0.1.0",
     "arch": "x86_64",
     "os": "linux",
     "url": "https://<oma-domain>/oma/releases/0.1.0/oma-x86_64-linux.tar.zst",
     "sha256": "<64 lowercase hex digits>"
   }
   ```

   `version` must be 1. `arch` and `os` must match this machine. `url` must be `https` on `<oma-domain>` and must not be a redirect. Unknown fields are ignored. There is no field by which one key names another.
7. **Download.** The tarball is saved in a temporary directory, mode `0700`, and its SHA-256 must match `sha256`. A mismatch deletes the temporary directory and stops. The tarball holds `oma`, `oma-consent`, the `binfmt` script, the unit files, `oma.xml` and `oma.desktop`. CI builds that tarball, `install.sh` and `current.json` only for a release tag.
8. **Install.** Only after the signature and the hash match. System-wide writes the table below. Per-user writes the same files under the home directory, as the later section says, and does not register `binfmt_misc`. Files are written to a temporary name in the destination directory and renamed into place. A rename on the same filesystem is the point the new file becomes visible.
9. **Register.** System-wide: `update-mime-database /usr/share/mime`, `update-desktop-database /usr/share/applications`, the one `application/x-oma=oma.desktop` line in `/usr/share/applications/mimeapps.list` (ADR 0053), `systemctl --global enable` of the four units, and `systemctl restart systemd-binfmt`. Per-user: the user MIME and desktop databases, `xdg-mime default oma.desktop application/x-oma`, and `systemctl --user enable --now` of the same four units with `ExecStart` rewritten to `~/.local/bin/oma`. Neither mode enables an inbox instance. There is no installed Bundle yet. The script does not install `erofs-utils` or `erofsfuse`. If they are missing it prints `pacman -S erofs-utils erofsfuse` and continues; `oma bundle` and `oma run` need them (ADR 0052) and will say so when they are absent.
10. **Receipt and summary.** The receipt is `/var/lib/oma/runtime.toml` for a system install and `~/.local/share/oma/runtime.toml` for a per-user install. It records the domain, the key id, the public key, `oma_version`, `method = "script"`, and the same `last_keys`, `last_reachable` and `dark_since` fields ADR 0055 keeps per Identifier. The pinned key is the one key that signed `current.json`, not every key the domain listed. The script then prints what it did: the mode, the version, the key id, every path it wrote, whether `binfmt_misc` was registered, whether the units were enabled, and the uninstall command.

### System paths

`oma` is at `/usr/bin/oma` and `oma-consent` at `/usr/bin/oma-consent`. ADR 0044's units assume the first path. This ADR is what decides it.

| Path | What it is |
|---|---|
| `/usr/bin/oma` | The program. |
| `/usr/bin/oma-consent` | The consent dialog (ADR 0025, ADR 0033). |
| `/usr/lib/oma/binfmt` | The `binfmt_misc` interpreter: `exec /usr/bin/oma run -- "$1"` (ADR 0053). |
| `/usr/lib/binfmt.d/oma.conf` | Magic `OMA1` at offset 0, interpreter the line above. |
| `/usr/share/mime/packages/oma.xml` | `application/x-oma` (ADR 0053). |
| `/usr/share/applications/oma.desktop` | The handler, `NoDisplay=true`, with the Show-contents action. |
| `/usr/lib/systemd/user/oma-omarchy.path` and `.service` | The theme-copy units (ADR 0044). |
| `/usr/lib/systemd/user/oma-register.service` | Login reconcile (ADR 0053). |
| `/usr/lib/systemd/user/oma-applications.path` | Watches `~/Applications`. |
| `/usr/lib/systemd/user/oma-inbox@.path` | Template. Not enabled globally. `--register` enables instances. |
| `/var/lib/oma/runtime.toml` | The runtime receipt: domain, pinned key, version, method. |

The script does not create `~/Applications`, does not write application data, and does not start a daemon. The user oneshot creates the directory at login.

### Per-user paths

`--user` copies `oma` and `oma-consent` to `~/.local/bin/`, the units to `~/.config/systemd/user/` with `ExecStart` rewritten, `oma.xml` to `~/.local/share/mime/packages/`, and `oma.desktop` to `~/.local/share/applications/`. The receipt is `~/.local/share/oma/runtime.toml`. It does not register `binfmt_misc`, does not touch `/usr`, and prints that an executable `.oma` file will not run from the kernel. A double-click still uses the MIME handler. This install is the weaker place to put the verifier: the user who can replace a Bundle can also replace `~/.local/bin/oma`. ADR 0055's "the installed `oma` is not writable by that user" is the system install. The default is system-wide for that reason.

## How the script is trusted

`curl | sh` trusts TLS to `<oma-domain>` and nothing else. That is the same root ADR 0055 already uses for a publisher: the domain's HTTPS name. It is a weak root, and it is the one this bootstrap has, because there is no earlier `oma` to ask. The three options that look like they add a root were weighed.

- **A key pinned inside the script.** On a first install the script and the pin arrive together from the same host. An attacker who can serve `install.sh` can serve a script whose pin is their own key. The pin bites only if the bytes the user runs were obtained from somewhere the attacker does not control. The documented saved-file review is that case, when the copy the user reads is the copy the user runs. A pin baked into the file does not improve on reading it, and a pin the script fetches from the same domain is the well-known document, which the script already checks. Not used.
- **A published checksum or signature of the script, on the same domain.** Deno publishes a SHA-256 of `install.sh` on a second URL the Deno project also controls. It catches a mirror that drifted from the repository. It does not catch the owner signing, or serving, a new pair. `<oma-domain>` is one origin. A checksum next to `install.sh` is the same TLS fetch. Not used as a trust anchor. The ed25519 signature on `current.json` is the check on the bytes that get installed, under the well-known key, and it is not a checksum of the script.
- **Download, inspect, then run.** This is the careful path, and it is documented next to the one-liner. The script prints the key id, the version and the paths, and waits on `/dev/tty`, so the pipe shows the same facts before it writes. That pause is not a cryptographic check. It is the moment the person sees the key they are about to pin.

**Decision.** The first install trusts the domain over TLS, shows the key id, and pins that one key in the receipt. The careful path is to save the script and read it. No key is embedded in the script, and no checksum of the script is treated as a second root. After this install, upgrades are checked by the `oma` already on disk, against the pinned key. A new key is never applied by itself.

## Upgrade

`oma self-upgrade` fetches the well-known document and `current.json` the same way the script does. The installed binary does the verify. The script is not involved, except when an honest re-run of `install.sh` hands off to this command.

- The signature verifies under the pinned key, and the version is higher: replace the files by the same write-to-a-temporary-name-and-rename, then update the receipt. Listing extra keys beside the pin does nothing. A release signed by one of those extra keys is a different key, and the next paragraph applies.
- The document verifies but the signing key is not the pin: stop, change nothing, and print the pinned key id and the new key id. This includes a domain that was dark and comes back with a different key. The receipt's `dark_since` is set and cleared the way ADR 0055 sets it, and it is how the message distinguishes "this domain was unreachable and now signs with a different key" from "this domain stayed reachable and is signing with a different key". Neither case installs the new key. The user re-verifies by running `oma self-upgrade --accept-new-key` from a terminal. The command prints both ids again and reads the confirmation from `/dev/tty`. The flag without a terminal exits non-zero and changes nothing. There is no statement in `current.json`, the receipt or the well-known document by which the old key appoints the new one.
- The domain is dark (the fetch fails, or the key document parses and lists no keys): change nothing. Print that the update cannot be verified until the domain answers. The installed `oma` keeps running. There is nothing to hold: a half-applied runtime is worse than the current one, and the release can be fetched again.
- The version is not higher: change nothing and say so.

`oma self-upgrade` refuses to replace a binary that pacman owns. Upgrades then come from the package, as below.

## Failure and rollback

Verification happens in the temporary directory, before any destination path is changed. A bad signature, a bad hash, a failed fetch or a refused key deletes that directory and leaves the machine as it was.

On an upgrade, `oma self-upgrade` copies the files it is about to replace into `/var/lib/oma/previous-runtime/` (or `~/.local/share/oma/previous-runtime/` for `--user`) before the first rename. After the renames it runs the new `oma` with `--version`. That must exit 0 and print the version from `current.json`. If it does not, the previous files are renamed back, the new files are removed, and the command exits non-zero. The previous-runtime directory is removed only after that check passes.

On a first install, a failure after some renames have succeeded deletes the paths this run created and does not leave a receipt. A re-run starts from an absent install. The script does not try to finish a half-written first install in place.

`update-mime-database`, `update-desktop-database`, `systemctl` and `systemd-binfmt` run after the binaries are in place and have passed `--version`. If one of those fails, the binaries stay, the receipt records the step that failed, and the summary says the install is incomplete and that re-running `install.sh` (or `oma self-upgrade`) retries the registration. Rolling the binaries back because a desktop database failed would remove a program that already runs. The summary is the signal, not a second copy of the binaries.

A crash between two renames is recovered by re-running. The command sees the receipt, or the absence of one, and either upgrades or starts again. It does not leave two different versions as the live `oma` and `oma-consent`: before renaming, both new binaries are present in the temporary directory, and a mismatch between their versions refuses the install.

## Uninstall

`install.sh --uninstall`, or `oma self-uninstall`, removes the runtime and leaves the user's applications.

System-wide, in order: `systemctl --global disable` of the four units, remove the `application/x-oma` line from `mimeapps.list` and remove that file if it is then empty, delete the paths in the table above including the receipt and `previous-runtime`, `update-mime-database`, `update-desktop-database`, `systemctl restart systemd-binfmt`. Per-user: `systemctl --user disable --now` of the four units, remove the user MIME and desktop files and the `xdg-mime` default, delete `~/.local/bin/oma`, `~/.local/bin/oma-consent` and the runtime receipt. Neither walks every home directory, and neither calls `systemctl --user` for every uid.

Neither deletes `~/Applications`, application data, previous Bundle versions, inboxes, held upgrades, consent records or the Omarchy copy under `~/.local/share/oma`, or the generated `.desktop` files under `~/.local/share/applications/`. Those are the user's. An entry whose `Exec` is `oma` fails until `oma` is installed again. Inbox units a session enabled are left behind and fail because the unit file is gone; the next install's `--register` enables the ones that still have an inbox. Uninstall does not refuse to finish while a Bundle is installed. Removing an application is still deleting its file (ADR 0009, ADR 0052).

The command prints each path it removed and states that application data was left.

## The Omarchy package, later

When Omarchy ships `oma` in its own repository, the trust anchor for the program is that repository's signing key, already configured for pacman on an Omarchy machine. It is not the domain key. Installing the package does not change the pinned domain key and does not accept a new one. The user accepts the repository by installing the package. The PKGBUILD lives in `packaging/` and installs the system paths above, with the same dependencies (`erofs-utils`, `erofsfuse`, `zstd`, `shared-mime-info`, `desktop-file-utils`, `systemd`, and `openssl` for the bootstrap only). Pacman owns the files. The package does not write a runtime receipt and does not pin the domain key: pacman already verified the package.

Until that repository has the package, `install.sh` does not call pacman to install `oma`. Once `pacman -Si oma` shows the package from Omarchy's repository, the script does not download a tarball. It prints that the package is the install, and with the same `/dev/tty` confirmation runs `pacman -S --needed oma`. `--user` does not switch to pacman, because the package is system-wide; it prints that a per-user copy is separate and that `~/.local/bin` ahead of `/usr/bin` on `PATH` will shadow the package.

Moving a machine that already used the script: install the package. Pacman replaces the `/usr` files. `oma self-upgrade` sees that pacman owns `/usr/bin/oma` and will not replace it from the domain. The domain pin in the receipt is no longer what authorises those bytes. The user can delete the receipt; `oma self-uninstall` is the wrong tool once pacman owns the files, and it refuses, telling the user to remove the package. Application data is untouched by the move. A per-user script install is not removed by pacman. The package's install message and `install.sh` both say to run `install.sh --user --uninstall` if `~/.local/bin/oma` is still on `PATH`.

We do not publish `oma` to the AUR, and we do not publish it to Arch `[extra]` ourselves. Omarchy's repository is the package route. The name of the package is `oma`.

## Consequences

- ADR 0044 is amended: `/usr/bin/oma` is the path for the system install and for the later package. The two units it already describes are unchanged, and both installs enable them the way that ADR already asked.
- ADR 0053's system install is this script, and later the package. The per-user install is `--user`. It still cannot register `binfmt_misc`.
- ADR 0055 is unchanged in its rules. The runtime receipt uses those rules for `oma`'s own key: one pinned key, a dark domain does not install an update, a new key is a re-verification. The per-user binary is not the unwritable verifier that ADR describes.
- `docs/spec/oma-cli.md` gains `oma self-upgrade` and `oma self-uninstall`. It does not gain `update`, `list` or `uninstall` of a Bundle.
- A sandboxed application still sees `oma`'s copy of the text size and the font (ADR 0044). Neither install puts the user's real `~/.config/omarchy` into the sandbox.
- End-to-end tests that run a private `oma` with no `oma-consent` beside it are unchanged.

## Open questions

1. The hostname in the one-liner. The path is `/install.sh`, and the key URL is `/.well-known/oma-keys` on that host.
2. Whether a default Omarchy install has OpenSSL 3, and whether its `/etc/os-release` is `ID=arch`, `ID=omarchy`, or both across the versions people run (`docs/checks.md`). The accept list above is the decision until a real image matches none of it.
3. When Omarchy's repository carries the `oma` package, and which key signs that repository. The handoff is decided. This ADR does not publish the repository.
