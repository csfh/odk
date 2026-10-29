---
status: accepted
---

# odk is never installed as root

ADR 0060, section 8, gives `install.sh` a `--system` flag. Run as root, it installs under `/usr` and registers the `binfmt_misc` line. ADR 0086 kept that flag, a receipt at `/var/lib/oma/runtime.toml`, and an `ODK_DESTDIR` for testing it. On 2026-10-29, shown that install, the owner said: "i dont want root at all".

## Decision

- **One user, always.** `install.sh` installs for the user who runs it, under `~/.local`, and has no `--system`. Run as root, it writes nothing and says to run it as the user. It never calls `sudo`.
- **No `binfmt_misc` line.** Registering one needs root, so the script never registers it, and the kernel never runs a `.oma` file. A Bundle file is run with `odk oma run`. `packaging/binfmt.d/odk.conf` stays as text and goes in no tarball.
- **The receipt.** It is `~/.local/share/oma/runtime.toml` and nowhere else, mode `0644`, and an uninstall keeps it. It holds the release's version. ADR 0086 also named the root's key id, which the script cannot know and nothing reads, so it is not written. The rest of ADR 0086's receipt rule stands: the script refuses an older release, and the same release again is a repair.
- **`odk oma self-upgrade`** replaces that one user's install, as ADR 0086 says, and never runs as root.
- `ODK_DESTDIR` goes: a user install's tests already have a home of their own.

## Consequences

- ADR 0060 is amended. Section 8's `--system`, its root install under `/usr`, its `binfmt_misc` registration and the receipt at `/var/lib/oma` are gone. The rest of that section stands.
- ADR 0086 is amended. Its `--system`, its `ODK_DESTDIR`, its receipt at `/var/lib/oma` and the root's key id in the receipt are gone.
- ADR 0054's later handoff to Omarchy's package repository is not decided again here. A package is installed by pacman as root, which this record does not cover. If that handoff is still wanted, it needs its own decision.
- Issue #15's tests for `--system` are replaced by one: as root, the script writes nothing and calls no `sudo`.
