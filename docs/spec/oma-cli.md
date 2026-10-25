# oma command-line specification, version 1

Normative. This spec defines the whole command-line surface of `oma` (ADR 0010). It is deliberately small. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

There is no `update`, `list` or `uninstall`. oma does not compile applications or update them (ADR 0010), and uninstalling is deleting the Bundle directory (ADR 0009). It can assemble a Bundle from a built executable (`bundle`, ADR 0032) but never builds the executable.

## Commands

| Command | What it does |
|---|---|
| `oma run <target>` | Runs a Bundle in its sandbox and returns once the application has started; setup errors are reported before it returns. If the Bundle requests anything missing from its consent record (ADR 0023) it asks first: a Bundle opened from outside `~/Applications` and not installed gets Install, Run once or Cancel (ADR 0018), anything else Allow or Cancel. Install copies it and runs the installed copy; Run once runs it with a private data directory and records nothing. Fails closed if the dialog cannot run. Used by `.desktop` entries and by Flea. |
| `oma run --wait <target>` | The same, but `oma` stays (never as the application's parent, ADR 0034), relays SIGINT, SIGTERM, SIGHUP, SIGQUIT, SIGCONT, SIGUSR1, SIGUSR2 and SIGWINCH, and exits with the application's status (ADR 0024). Ctrl-Z is not relayed yet: it stops `oma` but not the application (`docs/checks.md`). |
| `oma install <bundle-dir>` | Copies a Bundle into `~/Applications` (ADR 0018), asking Install or Cancel first if it requests anything its consent record lacks. It does not yet write a `.desktop` entry and refuses an Identifier that is already installed. |
| `oma install --accept-permissions <bundle-dir>` | The same, recording consent for exactly the permissions the Bundle requests now, without a dialog (ADR 0016). |
| `oma dev [<project-dir>] [--wait]` | For a project under development (ADR 0051): runs the `build` commands of `<project-dir>/dev.toml` (default the current directory), assembles the Bundle as `bundle` does, asks Install or Cancel if it requests anything its consent record lacks, stops the daemon of the installed copy it replaces, installs over that copy and runs it as `run` does. A failed build or a Cancel changes nothing. |
| `oma rollback <identifier>` | Swaps the current and previous versions (ADR 0011). |
| `oma purge <identifier>` | Deletes the application's data, previous version, inbox and consent record. |
| `oma bundle <executable> --manifest <manifest.toml> [--include <file>[=<path>]]... [-o <dir>]` | Assembles `<Name>.oma/` in `<dir>` (default the current directory) from a built executable and a Manifest, validates it and prints its path (ADR 0032). |
| `oma consent <target>` | Shows the consent dialog for a Bundle through `oma-consent` (Allow or Cancel) and records the answer in its consent record; says so and opens nothing if there is nothing to ask. The same step `run` and `install` take when something is missing (ADR 0033). |
| `oma inspect <bundle-dir>` | Prints the Bundle's name, Identifier, version and the permissions it requests, worded as in `consent-dialog.md`. |
| `oma omarchy sync` | Copies the user's text size, request for reduced motion (what Hyprland says about animating windows, ADR 0048, or the key in `shell.toml` when it says nothing) and font into `~/.local/share/oma/omarchy/` for sandboxes to show (ADR 0044). `run` does it before every launch; the user units the package ships (`packaging/`) run it when either file changes and at login. |
| `oma --register` | Reconciles `~/Applications` and the inboxes: generates and removes `.desktop` entries, activates upgrades and enables or disables inbox watches. Idempotent. Used by the path units and the login run (ADR 0009, ADR 0015). |
| `oma --init` | Internal. Runs the init inside an application's PID namespace (ADR 0024). Not for users and not a stable interface. |

## Targets

`<target>`, for `run`, is either a path to a Bundle directory or an Identifier. An argument that is a valid Identifier (see `manifest.md`) is treated as an Identifier and looked up among installed Bundles; anything else is treated as a path. Identifiers cannot contain `/`, so a path with a slash is never mistaken for one.

## Behaviour

- **`install`** refuses a Bundle whose Identifier is already installed at an equal or higher version. The only way to a lower version is `rollback` (ADR 0015).
- **`rollback`** fails if there is no previous version.
- **`purge`** refuses while the Bundle is still in `~/Applications`, since deleting its consent record would only make it ask again; remove the Bundle first.
- **`bundle`** takes the Bundle's name from the Manifest's `name`, with `/` replaced by `-`. It copies the Manifest unchanged, puts the executable at the Manifest's `executable` path with mode 0755, and copies the icon, if the Manifest names one, from the same relative path beside the Manifest. Each `--include FILE[=PATH]` also puts a regular file in the Bundle, at `PATH` (default its own name at the root), with its executable bit kept and mode 0755 or 0644; a symlink, a directory, a path outside the Bundle and a name the Bundle already has are refused (ADR 0050). It builds the result under a temporary name, validates it as `inspect` does, and renames it into place; if anything fails, nothing is left behind. It refuses to replace an existing `<Name>.oma`.
- **`dev`** is the one command that replaces an installed Bundle (`install` still refuses one). Its `dev.toml` is read from the project's directory and is never put in the Bundle. Keys: `build` (an array of commands, each an array of strings, run in order in the project's directory, stopping at the first failure; may be empty), `executable` (required), `manifest` (default `manifest.toml`) and `include` (as `bundle --include`). Every path is relative to the project's directory, and may go above it. Any other key, an empty command and a missing file are errors. The order is: read, build, assemble, ask, stop the daemon, install, run; the daemon is the unit `run` would start (`oma-<Identifier>`), stopped only if the Manifest declares one. If something else still accepts connections on its socket, `dev` says so on standard error and goes on.
- **Fail closed:** if the dialog cannot give one offered answer (no display, no `oma-consent` next to `oma`, a non-zero exit, or output that is not one offered word), nothing is granted, installed or started, and the error names `oma install --accept-permissions` as the dialog-less way (ADR 0016).
- **Order:** `run` applies its refusals (`dbus`, a kernel without Landlock ABI 6 for `network`) before asking, so nobody consents to something that cannot start.
- **`inspect`** validates the Manifest and exits non-zero if it is invalid.
- **Exit status:** zero on success and non-zero on any failure or when the user cancels the dialog. Under `--wait`, the application's own exit code is returned instead (ADR 0024).

## Not covered here

The text of its output, and shell completions.
