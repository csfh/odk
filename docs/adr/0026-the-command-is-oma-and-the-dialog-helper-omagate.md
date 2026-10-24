---
status: amended by ADR 0031 and ADR 0033
---

# The command is `oma` and the dialog helper is `omagate`

The program the brief called `omalaunch` is named `oma`, and the consent dialog helper of ADR 0025 is named `omagate`. `omalaunch` was long to type, and everything else it touches already uses the short prefix: the state directory `~/.local/share/oma/`, the unit `oma-inbox@<id>.path` and the `oma-fmt` crate. Omarchy's own applications use the same prefix (`omacalc`, `omacut`). The tool does more than launch (it installs, registers and sandboxes as well), so the shorter name is also the more accurate one. `omagate` names what the helper is: the gate a Bundle passes through to get its permissions. The binaries and crates are `oma`, `omagate` and `oma-fmt` (the Bundle format library, formerly called `oma-format`); the CLI spec is `docs/spec/oma-cli.md`.

Running a Bundle is an explicit subcommand, `oma run <target>`, like `install`, `inspect`, `rollback` and `purge`, rather than a bare `oma <target>`. The `.desktop` entries (ADR 0014) therefore call `oma run <identifier>`. The init runs as `oma --init`.

AOSC OS has an unrelated package manager also called `oma`. It is not part of Arch, so nothing collides on Omarchy, but searching the web for it will be noisy.

ADRs 0001 to 0024 keep the name `omalaunch` because they record decisions as they were made; this ADR is the key. Specs, the glossary, the plan and CLAUDE.md use the new names.
