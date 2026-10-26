---
status: accepted
---

# `odk` is installed for one user by a script from GitHub Releases

The owner decided on 2026-10-26: the install is a simple script, run with `curl`, that puts the daemon, the `odk` command and the programs beside it in the user's own directories, and it downloads a prebuilt tarball from the project's GitHub releases and checks its SHA-256. It is not built from source on the user's machine. This record is that decision and what it leaves undone. ADR 0054 decided a larger install, from a domain, with signed releases and a system-wide default; none of that is built, and this is the first route, which that ADR said a GitHub release tarball would not be.

## What it installs, and where

For the user who runs it, with no root and nothing outside the home directory:

| Path | What |
|---|---|
| `~/.local/bin/odk` | The command, the only one on `PATH` (ADR 0056) |
| `~/.local/lib/odk/odk-oma` | The Bundle program behind `odk oma`, private (ADR 0056) |
| `~/.local/lib/odk/oma-consent` | The consent dialog, beside it (ADR 0025) |
| `~/.local/lib/odk/odkd` | The daemon that tells every application what the host says (ADR 0062) |
| `${XDG_CONFIG_HOME:-~/.config}/systemd/user/odkd.socket` and `odkd.service` | Its units, which name the program with `%h` |

The prefix is `~/.local` and does not move, because `odk` finds `odk-oma` at `../lib/odk` of the directory it runs from (ADR 0056); only the units follow `XDG_CONFIG_HOME`. Then it runs `systemctl --user daemon-reload`, `enable --now odkd.socket` and `try-restart odkd.service`, so a new install starts `odkd` and an upgrade runs the new program. Where there is no systemd user session (ssh without a bus) it says so and installs the files. Running it again upgrades; `--uninstall` removes exactly these files and keeps application data under `~/.local/share/oma`. `--version TAG` installs a named release.

Every file is written under a temporary name in its directory and renamed into place. A tarball with a path outside `odk/`, with `..`, or without any of the files above is refused before anything is written. The script refuses to run as root.

## Where it downloads from, and how far to trust it

`https://github.com/csfh/odk/releases/latest/download/` (or `download/<tag>/`): `odk-x86_64-linux.tar.gz`, its `.sha256`, and `install.sh` itself, so the one-liner is `curl --proto '=https' --tlsv1.2 -fsSL https://github.com/csfh/odk/releases/latest/download/install.sh | sh`, and the script always belongs to the tarball beside it. A tag `v*` runs `.github/workflows/release.yml`, which builds with `--locked --release` and packs with `tools/package-release`, the same script the installer's test uses, so the layout cannot drift between them.

A release download is a redirect to GitHub's CDN, so redirects are followed, and only to HTTPS (`--proto =https --proto-redir =https`). ADR 0054 forbids redirects for its own domain's documents; that rule cannot hold here.

The checksum is published in the same release as the tarball. It catches a truncated or corrupted download and a mirror that has drifted. It does not catch a release that was published maliciously: the trust is TLS to github.com and the owner's control of the repository, and nothing in the script goes beyond that. ADR 0054 and ADR 0060 name the stronger scheme, a pinned root key and a signed release, as the next phase. This install is not a Bundle's root of trust.

## What it does not do

- **Register `binfmt_misc`.** That needs root, and a user install cannot (ADR 0060). The line is text in `packaging/binfmt.d/odk.conf`, and the registration is a system install's.
- **Register the MIME type or the desktop handler.** Nothing produces a single-file `.oma` yet, and `odk oma` refuses one, so a default handler that only refuses would edit the user's `mimeapps.list` for nothing. The files are in `packaging/` for the phase that needs them.
- **Check the distribution,** pause for a confirmation, keep a receipt, or hand off to `odk oma self-upgrade`. ADR 0054's OS check (Arch or Omarchy), the pause on `/dev/tty` and the `runtime.toml` receipt belong to the signed install; it needs a pinned key to put in the receipt. It stops on anything but Linux on x86_64.
- **Install `erofs-utils`,** which the single-file Bundle will need (ADR 0052).

## Conflicts with earlier records

- ADR 0054 is amended: its domain, key document, signed `current.json`, `.tar.zst`, system-wide default and pause are the later install, and a GitHub release tarball is, for now, the route.
- ADR 0056 describes a root-owned `/usr/lib/odk/binfmt` shell wrapper as the `binfmt_misc` interpreter. ADR 0060 (pull request #9) and issue #11 drop it: the kernel runs `odk-oma` directly, with no shell between, and `packaging/binfmt.d/odk.conf` is that line. The wrapper is not built and not installed.
- ADR 0056 says `oma-consent` stays beside `odk-oma`; the install does that.

## Consequences

- `oma` is no longer a command (ADR 0056). A program that calls it, as Atmos's documentation and tests do, must call `odk oma …`.
- Nothing here has been run against a real release: the tag, the workflow and the one-liner's URL do not exist until a first release is published. The script is tested against a tarball built by the same packaging script, in a throwaway home with a stub `systemctl` and a `file://` address.
- The version `odk --version` prints is the crates' `0.0.0`; the release tag is what `install.sh` reports.
