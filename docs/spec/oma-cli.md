# oma command-line specification, version 1

Normative. This spec defines the whole command-line surface of `oma` (ADR 0010). It is deliberately small. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

There is no `update`, `list` or `uninstall` of a Bundle. oma does not compile applications or update them (ADR 0010), and uninstalling an application is deleting its `<Name>.oma` file (ADR 0009, ADR 0052). It can assemble a Bundle from a built executable (`bundle`, ADR 0032) but never builds the executable. `odk oma self-upgrade` and `odk oma self-uninstall` replace or remove the `oma` program itself (ADR 0054). They do not fetch or remove Bundles. The commands below are the accepted surface. The built binary still takes a Bundle directory and still verifies no signature; it changes to follow ADR 0052 and ADR 0055.

The command a person types is `odk oma …` (ADR 0056). `odk` is the only command on `PATH`: a small dispatcher, crate `odk-cli`, that finds `odk-oma` in `/usr/lib/odk/` or `~/.local/lib/odk/` and replaces itself with it. The program behind it is `odk-oma`, the crate `oma`'s binary, which is private; its messages begin `odk oma:` and its help is titled `odk oma`. With one argument that is a regular file, `odk-oma` is the entry the kernel uses for a `.oma` file (`packaging/binfmt.d/odk.conf`): until an envelope can be verified it refuses that file without opening it.

## Commands

| Command | What it does |
|---|---|
| `odk oma run <target>` | Runs a Bundle in its sandbox and returns once the application has started; setup errors are reported before it returns. If the Bundle requests anything missing from its consent record (ADR 0023) it asks first: a Bundle opened from outside `~/Applications` and not installed gets Install, Run once or Cancel (ADR 0018), anything else Allow or Cancel. Install copies it and runs the installed copy; Run once runs it with a private data directory and records nothing. Fails closed if the dialog cannot run. Used by `.desktop` entries and by Flea. |
| `odk oma run --wait <target>` | The same, but `oma` stays (never as the application's parent, ADR 0034), relays SIGINT, SIGTERM, SIGHUP, SIGQUIT, SIGCONT, SIGUSR1, SIGUSR2 and SIGWINCH, and exits with the application's status (ADR 0024). Ctrl-Z is not relayed yet: it stops `oma` but not the application (`docs/checks.md`). |
| `odk oma install <file>` | Copies a Bundle into `~/Applications` (ADR 0018), asking Install or Cancel first if it requests anything its consent record lacks, or if its signing key is not the pin (ADR 0055). It does not yet write a `.desktop` entry and refuses an Identifier that is already installed at an equal or higher version. |
| `odk oma install --accept-permissions <file>` | The same, recording consent for exactly the permissions the Bundle requests now, without a dialog (ADR 0016). A signing key other than the pin is not accepted this way. |
| `odk oma dev [<project-dir>] [--wait]` | For a project under development (ADR 0051): runs the `build` commands of `<project-dir>/dev.toml` (default the current directory), assembles the Bundle as `bundle` does, asks Install or Cancel if it requests anything its consent record lacks, stops the daemon of the installed copy it replaces, installs over that copy and runs it as `run` does. A failed build or a Cancel changes nothing. |
| `odk oma rollback <identifier>` | Swaps the current and previous versions (ADR 0011). |
| `odk oma purge <identifier>` | Deletes the application's data, previous version, inbox, held upgrade and consent record. |
| `odk oma bundle <executable> --manifest <manifest.toml> [-o <dir>] [--include <file>[=<path>]]... [--sign <secret-key>]` | Assembles `<Name>.oma` in `<dir>` (default the current directory) from a built executable and a Manifest, validates it and prints its path (ADR 0032, ADR 0052). `--include` may be repeated. `--sign` fills the preamble signature (ADR 0055). |
| `odk oma consent <target>` | Shows the consent dialog for a Bundle through `oma-consent` (Allow or Cancel) and records the answer in its consent record; says so and opens nothing if there is nothing to ask. The same step `run` and `install` take when something is missing (ADR 0033). A different signing key uses Install, Run once or Cancel (ADR 0055). |
| `odk oma inspect <file>` | Prints the Bundle's name, Identifier, version and the permissions it requests, worded as in `consent-dialog.md`. Also the publisher, the key id, whether the domain was reached, and a held upgrade if there is one (ADR 0055). |
| `odk oma mount <file>` | Mounts the file read-only and prints the directory (ADR 0053). |
| `odk oma mount --open <file>` | Mounts it and opens that directory in the file manager. |
| `odk oma mount --unmount <file>` | Unmounts that view. A view otherwise lasts until logout. |
| `odk oma --register` | Reconciles `~/Applications` and the inboxes: generates and removes `.desktop` entries, activates upgrades and enables or disables inbox watches. Idempotent. Used by the path units and the login run (ADR 0009, ADR 0015). |
| `odk oma self-upgrade` | Replaces this `oma` with a newer release from its domain, verified under the pinned key (ADR 0054). A different key stops it unless `--accept-new-key` is confirmed on a terminal. Refuses when pacman owns the binary. |
| `odk oma self-uninstall` | Removes the `oma` install and leaves application data (ADR 0054). Refuses when pacman owns the binary. |
| `oma --init` | Internal. Runs the init inside an application's PID namespace (ADR 0024). Not for users and not a stable interface. |

## Targets

`<target>`, for `run`, is either a path to a Bundle file or an Identifier. An argument that is a valid Identifier (see `manifest.md`) is treated as an Identifier and looked up among installed Bundles; anything else is treated as a path. Identifiers cannot contain `/`, so a path with a slash is never mistaken for one.

## Behaviour

- **`install`** refuses a Bundle whose Identifier is already installed at an equal or higher version. The only way to a lower version is `rollback` (ADR 0015).
- **`rollback`** fails if there is no previous version.
- **`purge`** refuses while the Bundle is still in `~/Applications`, since deleting its consent record would only make it ask again; remove the Bundle first.
- **`bundle`** takes the Bundle's name from the Manifest's `name`, with `/` replaced by `-`. It stages the Manifest unchanged, the executable at the Manifest's `executable` path with mode 0755, the icon if the Manifest names one, and each `--include`. It runs `mkfs.erofs` as ADR 0052 specifies, stamps the preamble, validates the result as `inspect` does, and renames it into place; if anything fails, nothing is left behind. It refuses to replace an existing `<Name>.oma`.
- **A different signing key** is not installed by `--register`. The file is held at `~/.local/share/oma/held/<identifier>.oma`, and the first-install dialog is how a new key gets in (ADR 0055). `--accept-permissions` does not stand in for that dialog. An update that arrives while the publisher's domain cannot be reached is held there too, including one that verifies under the pinned key.
- **Launch** of an installed file that still verifies under the pinned key proceeds when the publisher's domain cannot be reached, and says so on standard error. An update is held rather than applied (ADR 0055).
- **`dev`** is the one command that replaces an installed Bundle (`install` still refuses one). Its `dev.toml` is read from the project's directory and is never put in the Bundle. Keys: `build` (an array of commands, each an array of strings, run in order in the project's directory, stopping at the first failure; may be empty), `executable` (required), `manifest` (default `manifest.toml`) and `include` (as `bundle --include`). Every path is relative to the project's directory, and may go above it. Any other key, an empty command and a missing file are errors. The order is: read, build, assemble, ask, stop the daemon, install, run; the daemon is the unit `run` would start (`oma-<Identifier>`), stopped only if the Manifest declares one. If something else still accepts connections on its socket, `dev` says so on standard error and goes on.
- **Fail closed:** if the dialog cannot give one offered answer (no display, no `oma-consent` next to `oma`, a non-zero exit, or output that is not one offered word), nothing is granted, installed or started, and the error names `odk oma install --accept-permissions` as the dialog-less way (ADR 0016).
- **Order:** `run` applies its refusals (`dbus`, a kernel without Landlock ABI 6 for `network`) before asking, so nobody consents to something that cannot start.
- **`inspect`** validates the Manifest and exits non-zero if it is invalid.
- **Exit status:** zero on success and non-zero on any failure or when the user cancels the dialog. Under `--wait`, the application's own exit code is returned instead (ADR 0024).

## Not covered here

The text of its output, and shell completions.
