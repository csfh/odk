---
status: amended by ADR 0063
---

# The `odk` command dispatches to `odk-<name>`, and the Bundle program is `odk-oma`

The owner asked to make the oma CLI a facade, as `odk oma`, and then `odk-oma`. This record is that shape, on the model of `git` and `cargo`: one short umbrella command looks up a separate program and replaces itself with it. `odk <name> <args>` runs `odk-<name>`. The umbrella does not grow the Bundle commands, and it does not link the library applications depend on.

Nothing here is built. The binary on disk is still `oma`, and it is still on `PATH` when installed.

The number is 0056. `origin/atmos-v1.0.0-rc` (there is no tag of that name; the only tag fetched was `v0.0.1`) already has ADR 0050 and ADR 0051, which are not in this branch. ADR 0052 to ADR 0055 are the single-file Bundle records on this branch. 0056 is the next free number after 0055 that those lines do not use.

This record depends on ADR 0052, ADR 0053, ADR 0054 and ADR 0055. Those four are pull request #3. This record is stacked on that pull request and lands after it. The paths below assume the single-file Bundle, the desktop and `binfmt_misc` registration, the `install.sh` bootstrap, and the three-layer trust rules those ADRs already decided.

## The command a person types

Decided by the owner on 2026-10-25: there is no command named `oma`. The Bundle program's file name is `odk-oma`, and that file is not installed on `PATH`. `odk` is the only user-facing command. `odk oma …` is the front door to the CLI that is `oma` today.

`run`, `install`, `bundle`, `mount`, `self-upgrade` and the rest stay inside `odk-oma`. `odk run` looks for a program named `odk-run`. It does not forward to `odk oma run`. The symlink and hardlink options that would have kept a short `oma` are closed.

The state directory `~/.local/share/oma/`, the MIME type `application/x-oma`, and the word Bundle stay. Those are not commands. This decision is about what is on `PATH`.

## What the umbrella is

`odk` is a dispatcher. It understands a handful of its own flags, and otherwise it finds an external program and executes it. It has no Bundle subcommands of its own.

That split is the point of the facade. The dispatcher stays small enough to read. A new `odk-<name>` can appear without a change to `odk`. The program that verifies a Bundle (ADR 0055) stays a separate file the desktop and the kernel can name absolutely, in a directory that is not on `PATH`. Folding `odk-oma` into the dispatcher, or teaching the dispatcher each Bundle flag, would make the facade the program it is only supposed to find.

`git` and `cargo` are the precedent for dispatch. Both run an external `tool-<name>` when the name is not one of their own commands, both replace the process on Unix, and both let a built-in command win over an external program of the same name. Arch's `git` package is also the precedent for the directory: `/usr/bin/git` is on `PATH`, and `git-add` and the rest live in `/usr/lib/git-core/`, which is not on `PATH` (package file list, extra, read 2026-10-25). `odk-oma` follows that split. It does not follow cargo's habit of putting `cargo-<name>` on `PATH`.

## Where `odk-oma` lives

**Decided.** The install does not copy `odk-oma` onto `PATH`. Not `/usr/bin/odk-oma`, not `~/.local/bin/odk-oma`. The only `PATH` entry from this runtime is `odk`. The private directories are the ones in the table. A development build finds `odk-oma` next to `odk`.

| Install | Dispatcher, on `PATH` | Bundle program, not on `PATH` | Dialog helper, beside it |
|---|---|---|---|
| System and the package | `/usr/bin/odk` | `/usr/lib/odk/odk-oma` | `/usr/lib/odk/oma-consent` |
| `--user` and `install.sh --user` | `~/.local/bin/odk` | `~/.local/lib/odk/odk-oma` | `~/.local/lib/odk/oma-consent` |
| `cargo build`, not an install | `target/debug/odk` (or `release`) | the `odk-oma` beside it | an `oma-consent` beside it, when a test puts one there |

FHS allows `/usr/libexec` for binaries that other programs run and users do not (Filesystem Hierarchy Standard, `/usr/libexec`). Arch's package guidelines say the opposite of using it: avoid `/usr/libexec/` for anything, and use `/usr/lib/$pkgname/` instead (Arch package guidelines, "Package etiquette"). gparted and libvirt on Arch are built that way. `/usr/lib/odk/` is that rule when the package name is `odk`. `/usr/libexec/odk/` would be the FHS spelling and the one Arch packagers are told not to add.

The per-user directory is `~/.local/lib/odk/`, the same shape under the home prefix. It is not `$XDG_DATA_HOME/odk` (`~/.local/share/…`). That tree is data. The consent record, the runtime receipt and application state already live under `~/.local/share/oma/` (ADR 0023, ADR 0054). The verifier does not sit in the directory those files are written to. XDG does not name a user libexec; `~/.local/lib` is the counterpart of `/usr/lib`.

An unpackaged build does not get a copy installed anywhere. Cargo writes `odk` and `odk-oma` into one target directory. That directory is not put on `PATH` by this project. The lookup below finds the sibling there so `cargo run -p odk-cli -- oma …` works.

`oma-consent` stays beside the `odk-oma` file, because `odk-oma` finds the helper with `current_exe` (ADR 0025). The dispatcher is not on that search.

## Dispatch

The first argument decides. No argument parsing is done on the rest.

| Invocation | What happens |
|---|---|
| `odk`, `odk help`, `odk --help`, `odk -h` | Dispatcher help on standard output, exit 0. |
| `odk help <name>` | Same lookup as `odk <name>`, then that program is executed with a single argument `--help`. |
| `odk --version`, `odk -V` | The dispatcher's own version, one line, exit 0. Does not run `odk-oma`. |
| `odk --list` | Discovered external commands, exit 0. |
| `odk <name> [args...]` | Look up `odk-<name>` and execute it with `[args...]`. |
| `odk oma --help`, `odk oma --version` | Ordinary passthrough. The dispatcher does not read those flags. |

A built-in wins over any file. `odk help` never runs `odk-help`. `odk --version` never runs `odk-version`. There is no built-in file named `list` or `version`: `odk list` looks up `odk-list`, and `odk version` looks up `odk-version`. `--list` and `--version` are flags so those names stay available.

`oma` is a built-in name. It is resolved only by the rule in the next section. A file named `odk-oma` on `PATH` is not that program, and the dispatcher does not run it.

Anything whose first argument starts with `-` and is not exactly `--help`, `-h`, `--version`, `-V` or `--list` is a usage error. Flags of the external program go after the name: `odk oma --flag`. `--` is not a name and does not introduce one.

`odk help` with more than one following argument is a usage error. `odk --help`, `odk --version` and `odk --list` take no further arguments.

### Names

A name is one token: an ASCII lowercase letter, then up to 63 characters from `[a-z0-9-]`. Upper case, underscore, dot, space and non-ASCII are rejected. Empty is rejected.

The token is not a path. `/`, `\` and a NUL byte are rejected before any directory is opened. `.` and `..` are not names. The dispatcher never joins the token onto the current directory, never treats it as a relative path, and never executes a path the user typed. `odk /usr/lib/odk/odk-oma` and `odk ../odk-oma` are usage errors.

The file name looked up is exactly `odk-` plus that token, with no added suffix.

Invalid name, unknown flag, or a built-in flag with unexpected arguments: one line on standard error, prefixed `odk:`, exit 2. Nothing is executed.

### Resolving `oma`

The built-in name `oma` never consults `PATH`.

1. If the directory containing the `odk` executable is named `bin`, the only candidate is `../lib/odk/odk-oma` relative to that directory. `/usr/bin/odk` resolves `/usr/lib/odk/odk-oma`. `~/.local/bin/odk` resolves `~/.local/lib/odk/odk-oma`. A sibling `/usr/bin/odk-oma` is ignored, because that location is on `PATH`.
2. Otherwise the only candidate is `odk-oma` in the same directory as `odk`. That is the Cargo target directory.

The candidate is used when it is a regular file. Symlinks are followed for the file-type check. A directory or other non-regular file is a miss. There is no second candidate and no `PATH` walk.

### Other names

**Decided.** The private directory wins. The built-in name `oma` is never taken from `PATH`. Any other name looks in the private directory first, then in absolute `PATH` entries, and skips `.` and the current directory.

For a name other than `oma`:

1. The same place step 1 or 2 above would look: `../lib/odk/odk-<name>` when `odk` lives in `bin`, or the sibling when it does not.
2. Then each absolute entry of `PATH`, left to right. An empty entry is skipped (an empty entry means the current directory). `.` is skipped. A relative entry is skipped. A directory already used in step 1 is skipped.

The first regular file wins. A later copy is not tried when the first is not executable: the exec error is reported. The current directory is not added. A `PATH` the user wrote to include it is still not honoured.

`execvp` of a bare `odk-<name>` is not used. The executed path is the absolute path that was selected.

Two other layouts were rejected:

| | What it does | Why it was rejected |
|---|---|---|
| Private directory only | No `PATH` walk at all. A third-party `odk-foo` works only when it is installed into `/usr/lib/odk` or `~/.local/lib/odk`. | Extending the tool would mean writing the private directory, which a normal `cargo install` onto `PATH` does not do. |
| `PATH` first | A `odk-foo` earlier on `PATH` hides the copy in the private directory. `oma` stays exempt. | The private copy and the `PATH` copy can disagree. |

A third-party program is trusted the way cargo trusts `cargo-<name>`: whoever can write an earlier absolute `PATH` entry can supply `odk-foo`. They cannot supply `oma`. The closed set of built-in names is `oma`, plus the dispatcher flags above. A later built-in is an addition to that set in a later ADR, not a `PATH` race.

If `current_exe` fails, an installed layout cannot be derived. `oma` is then not found. Other names use the `PATH` walk only.

### Arguments, process, signals

Arguments after the name are passed through unchanged, as raw arguments, including `--`, empty arguments, and bytes that are not UTF-8. The name itself is not forwarded. `odk oma run --wait ./App.oma` executes the resolved `odk-oma` with `run`, `--wait`, `./App.oma`.

`argv[0]` is the absolute path of the file that was selected. It is not rewritten to `oma` or to `odk`.

On Linux the dispatcher replaces itself (`exec`). There is no spawn-and-wait. The exit status of a successful dispatch is the external program's status, because it is the same process. A signal delivered to the command is delivered to that program. Standard input, output and error are the descriptors already open. The working directory, the environment, the process group and the session are unchanged. The dispatcher installs no signal handlers and does not call `setsid`.

Spawn-and-wait was rejected. It would make `odk` the parent of `odk-oma` for the whole command, it would have to forward signals and to invent an exit code when the child died by signal, and during `odk oma run` it would make `odk` an ancestor of the application until `odk-oma` returned. ADR 0034 keeps the Bundle program from being that ancestor. The dispatcher must not add a new one. `exec` does not. A desktop file that names `/usr/bin/odk` does one extra exec, and the dispatcher is gone before `odk-oma` does its own work. Units and the binfmt script do not take that hop; they name the private program.

Before the exec, the environment variable `ODK` is set to the absolute path of the dispatcher. A value the caller already had is replaced, so a subcommand sees the `odk` that launched it. No other variable is added or removed. Subcommands may ignore `ODK`. `odk-oma` does not need it to find `oma-consent`. The collision check for this variable is below; it stays `ODK`.

If the real user or group id differs from the effective id, the dispatcher refuses and exits 126. It is not a setuid program.

### Help, version, list

Dispatcher help names the usage, says that Bundle commands are `odk oma` and that `odk-oma` is not on `PATH`, names the built-ins, and lists every external name the lookup above discovers, in byte order. It does not execute any of them to ask for a description.

`odk help <name>` is the same exec as `odk <name> --help`. A missing name uses the not-found path below.

`odk --version` prints `odk` and the dispatcher's version. `odk oma --version` is passthrough. `odk-oma` prints `odk oma` and the release version, not the file name `odk-oma`: the user-facing command is the facade. The two lines are `odk <version>` and `odk oma <version>`. They carry the same release version when both files came from one install. The dispatcher does not run `odk-oma` to check, and a mismatch does not change dispatch.

`odk --list` prints one external command per line, `name`, a tab, the absolute path selected, in byte order of the name. `oma` is included, with the private path, when that file exists. Built-in flags are not listed. A name found in an earlier directory hides the same name later. Files whose suffix is not a valid name are ignored.

### When the program is missing

No regular file from the lookup:

```
odk: no such command: '<name>'
```

For `oma`, the line names the one path that was tried (`/usr/lib/odk/odk-oma`, the per-user equivalent, or the sibling in a Cargo directory) and says a copy on `PATH` is not used. For any other name, if a discovered name is within a Levenshtein distance of 2, up to three of them are printed, closest first, as `Did you mean: odk <name>?`. When `oma` itself was found, the text also says that Bundle commands are `odk oma <command>`. A further line points at `odk --list`.

Exit 127. This is distinct from a usage error (2) and from a failure of the program itself.

The file was found and `exec` fails: exit 126, standard error names the absolute path and the system error. `ENOENT` at that moment is exit 127 with the same path. The search is not resumed.

### What `odk-oma` prints

Decided with the facade: messages from the Bundle program are prefixed `odk oma:`, and its help titles the command `odk oma`. That is the text whether the dispatcher, systemd, or the binfmt script started it. The prefix does not follow `argv[0]`. The file name is `odk-oma`; the command a person types is `odk oma`. Dispatcher messages stay `odk:`.

## Crates and binaries

The dispatcher and the Bundle program are two binaries in two crates. The library applications already depend on stays a library.

| Crate | Binary | What it is |
|---|---|---|
| `odk-cli` (new), `crates/odk-cli` | `odk` | The dispatcher. Its code uses only the standard library. The package depends on the workspace `deps` crate, as every member does, and on nothing else: not `oma`, not `odk`, not clap. Installed on `PATH`. |
| `oma` (existing), `crates/oma` | `odk-oma` | Today's library and today's CLI, with the binary target renamed from `oma` to `odk-oma`. Clap, the subcommands, the sandbox and the init stay here. Installed in the private directory, not on `PATH`. |
| `odk` (existing) | none | The facade library (ADR 0028). It does not gain a binary. |
| `oma-consent` (existing) | `oma-consent` | Unchanged helper (ADR 0025, ADR 0033). Installed beside `odk-oma`, not on `PATH`. |

The `odk` library crate does not host the command. Applications depend on that crate. A binary in the same package would put the command's dependencies on the graph of every application. `odk-cli` is a workspace member and is not published as a library. `oma` does not depend on `odk-cli` or on `odk`. ADR 0013 stands: the OMA crates and the ODK crates do not depend on each other. `odk-cli` is on the ODK side of that line.

What moves out of `crates/oma` is the file name of the binary, nothing else. `main.rs` stays the command implementation. Subcommand arguments stay in that clap definition. The dispatcher does not import them. Sharing is the passthrough: the words after `oma` are the arguments `odk-oma` already defines. Clap's displayed bin name becomes `odk oma`, so `--help` and `--version` use the facade spelling.

What the two binaries share about version is the release, not a Rust module. ADR 0054's `current.json` field `oma_version` remains the version of the runtime as a whole. A release builds `odk`, `odk-oma` and `oma-consent` from the same tag and installs them together. When those crates are first given a release version, they take one workspace `version`. There is no version crate. This record does not bump the `0.0.0` the crates have today.

The init stays a re-exec of the `odk-oma` file. The launcher already holds the binary open and starts the init with `execveat` and `AT_EMPTY_PATH`, passing `--init` (ADR 0024). The init does not search `PATH` and it does not start the dispatcher. The source today passes the string `oma` as that exec's `argv[0]`. That string is a label inside the sandbox, not a lookup. Acceptance sets it to `odk-oma`. It is not a command on `PATH`. `--init` remains a private flag of `odk-oma`.

`cargo run -p odk-cli -- oma …` works once both binaries have been built, because they share a target directory and that directory is not named `bin`. Tests that name `CARGO_BIN_EXE_oma` would name `CARGO_BIN_EXE_odk_oma`. That rename is part of accepting this record, and it is not done here.

## The runtime

ADR 0053 and ADR 0054 name `/usr/bin/oma` in desktop files, units and the binfmt script. The program they describe is the private `odk-oma`. **Decided:** desktop entries, generated entries and the mount action call the absolute facade `/usr/bin/odk oma …`. systemd, `binfmt_misc`, the `install.sh` handoff, and `self-upgrade` / `self-uninstall` call the private binary by absolute path.

Naming `/usr/bin/odk` is absolute, so a `~/.local/bin/odk` earlier on `PATH` does not take the click. Naming a bare `odk` would be that `PATH` search, and it is not used. Naming the private program from a desktop file would keep working if `/usr/bin/odk` were missing, and it would bake the lib directory into every generated entry. One `exec` of the facade is cheap, and the facade then `exec`s the private program, so `odk` is not an ancestor of the application (ADR 0034).

| Caller | What it calls | Why |
|---|---|---|
| System desktop handler, `run %f` | `Exec=/usr/bin/odk oma run %f` | The line a person reads is the user-facing command. Absolute, so `PATH` is not searched. The private directory can move without rewriting desktop files. |
| Show contents | `Exec=/usr/bin/odk oma mount --open %f` | The same file, the same reason. |
| Generated per-Bundle entries | `$prefix/bin/odk oma run` plus the Identifier, absolute | `--register` runs as the private program. If that file is `$prefix/lib/odk/odk-oma`, the entry names `$prefix/bin/odk`. A system install writes `/usr/bin/odk`. A per-user install writes `~/.local/bin/odk`. It does not search `PATH`, and it does not write the private path into the entry. |
| systemd user units | Absolute private program: `/usr/lib/odk/odk-oma omarchy sync`, and the same prefix for `--register` | Not a command a person reads. No second exec. Per-user, the rewritten path is `~/.local/lib/odk/odk-oma`. systemd's `PATH` is not involved. |
| `binfmt_misc` | Interpreter `/usr/lib/odk/binfmt`, a root-owned script whose only job is `exec /usr/lib/odk/odk-oma run -- "$1"` | The kernel requires an absolute interpreter and does not search `PATH`. The registration cannot be the two words `odk oma`. The script is the interpreter; it execs the private program, not the facade, so a missing `odk` on `PATH` cannot stop an executable `.oma`. |
| `install.sh` handoff | Exec the absolute private program with `self-upgrade` | A `PATH` search for `odk` would be the wrong program to trust with the upgrade. |
| `self-upgrade` and `self-uninstall` | Implemented in `odk-oma`. The person types `odk oma self-upgrade`. The code replaces the private files and `/usr/bin/odk` by absolute path. | The user-facing spelling is the facade. The bytes it replaces are the whole set below. |

`NoDisplay=true` stays on the handler. The launcher entries people click are still the per-Bundle ones. The desktop file id stays `oma.desktop`, and `application/x-oma=oma.desktop` stays, because that id is the MIME association ADR 0053 already fixed and it is not a command on `PATH`. `Name=` in that file can say `odk`. Flea, which is not in this repository, should exec the same absolute facade, not a bare `oma` and not a `PATH` lookup.

### `binfmt_misc`

Magic `OMA1` at offset 0 is unchanged (ADR 0053). The conf file is `/usr/lib/binfmt.d/odk.conf`. The interpreter path in it is `/usr/lib/odk/binfmt`. The script:

```
#!/bin/sh
exec /usr/lib/odk/odk-oma run -- "$1"
```

A per-user install still cannot register `binfmt_misc` (ADR 0054). The executable bit is still required. The old path `/usr/lib/oma/binfmt` is not installed.

### `self-upgrade` and `self-uninstall`

Both commands live in `odk-oma`. They upgrade or remove the runtime as one release. The set is:

| System path | Per-user path |
|---|---|
| `/usr/bin/odk` | `~/.local/bin/odk` |
| `/usr/lib/odk/odk-oma` | `~/.local/lib/odk/odk-oma` |
| `/usr/lib/odk/oma-consent` | `~/.local/lib/odk/oma-consent` |
| `/usr/lib/odk/binfmt` | not installed |
| `/usr/lib/binfmt.d/odk.conf` | not installed |
| the MIME, desktop and unit files | the per-user copies |
| `/var/lib/oma/runtime.toml` | `~/.local/share/oma/runtime.toml` |

There is no `/usr/bin/oma` and no `/usr/bin/odk-oma` in the set. Replacing one of `odk` and `odk-oma` without the other is not an upgrade. After the renames, the check ADR 0054 runs is two absolute invocations: `/usr/bin/odk --version` and `/usr/lib/odk/odk-oma --version` (per-user, the home equivalents). Each must exit 0. The lines are `odk <oma_version>` and `odk oma <oma_version>`. A tarball whose binaries were not built as that version is refused before any destination path is changed. Rollback copies the whole set into `previous-runtime/`.

`self-upgrade` and `self-uninstall` refuse when pacman owns any path in the system column. They do not walk `PATH`. They do not delete a Cargo target directory. They do not delete `~/.local/share/oma` application data.

The receipt's fields are unchanged. `oma_version` means this set. `method = "script"` means the script installed it. ADR 0055's rules for the runtime key apply to the tarball as they do today.

### `install.sh`

ADR 0054's script is the bootstrap. The OS check, the key fetch, the pause, the signature on `current.json`, and the hash of the tarball are unchanged. The tarball contains `odk`, `odk-oma`, `oma-consent`, the binfmt script, the units, `oma.xml` and `oma.desktop`. It does not contain a file named `oma`.

The handoff, when a receipt for this domain is already present, execs `/usr/lib/odk/odk-oma` or `~/.local/lib/odk/odk-oma` with `self-upgrade`. It does not search `PATH`.

Per-user install rewrites unit `ExecStart` and the desktop `Exec` to the absolute per-user facade or private program, as the table above says. It creates `~/.local/lib/odk/` and does not register `binfmt_misc`.

### Trust

ADR 0055 is unchanged in its three layers. The bytes that verify a Bundle are `/usr/lib/odk/odk-oma`. That path is not writable by the user who can write `~/Applications`. A per-user copy under `~/.local/lib/odk/` is the weaker install ADR 0054 already describes, for the same reason `~/.local/bin/oma` was. The dispatcher is not a verifier. Desktop files and Flea name `/usr/bin/odk`, which `exec`s the private program beside its own prefix, so a different `odk-oma` on `PATH` cannot take the click. An attacker who can write `/usr/bin` can already replace `/usr/lib/odk/odk-oma`.

External `odk-<name>` programs other than `oma` are as trusted as the `PATH` entry that supplied them. There is no signature check on them. They are not Bundles and they are not this runtime's tarball.

### `docs/spec/oma-cli.md`

The spec stays the surface of the Bundle program. Acceptance rewrites every command as `odk oma <command>`, states that the binary is `odk-oma` and is not on `PATH`, and points desktop files, units and the binfmt script at the paths in the table. `self-upgrade` and `self-uninstall` cover that set. The spec does not gain `update`, `list` or `uninstall` of a Bundle (ADR 0010). The dispatcher does not gain a spec file of its own; this record is the normative text for dispatch.

## What this amends

Not edited in this change. The code is unchanged, and so are the documents below. They are the ones this record amends, and they are updated when the code follows it.

| Document | Change |
|---|---|
| ADR 0026 | The command a person types is `odk oma`. There is no `oma` binary on `PATH`. The file is `odk-oma` in the private directory. The init is a re-exec of that file. |
| ADR 0031 | The `oma` crate remains one library and one binary; the binary target is `odk-oma`, installed off `PATH`. The dispatcher is `odk-cli`. |
| ADR 0028 | One sentence: the `odk` crate stays a library, and the `odk` command is `odk-cli`. |
| ADR 0025, ADR 0033 | The helper is beside the private `odk-oma`, not on `PATH`. The consent command is `odk oma consent`. |
| ADR 0024, ADR 0034 | The init is a re-exec of `odk-oma`. The dispatcher `exec`s and is not an ancestor of the application. |
| ADR 0014 | Generated `Exec` lines are the absolute facade, `odk oma run` plus the Identifier. |
| ADR 0044 | Unit `ExecStart` is `/usr/lib/odk/odk-oma`. The sandbox copy is unchanged. |
| ADR 0053 | Handler `Exec` and the Show-contents action use `/usr/bin/odk oma …`. The binfmt interpreter is `/usr/lib/odk/binfmt`, execing the private program. The desktop file id and the MIME type stay. |
| ADR 0054 | The tarball, the path table, the handoff, the `--version` check, pacman ownership and uninstall use the set in this record. Nothing named `oma` is installed on `PATH`. `oma_version` stays the field name. The package name is `odk`. |
| ADR 0055 | The unwritable verifier is `/usr/lib/odk/odk-oma`. Standard-error lines that say `oma:` say `odk oma:`. The trust rules are the same. |
| ADR 0032 | The assembly command is `odk oma bundle`. The behaviour does not change. |
| `docs/spec/oma-cli.md` | Invocation and the command tables use `odk oma`. |
| `docs/spec/consent-dialog.md` | Any command spelled `oma` in the failure text becomes `odk oma`. |
| `GLOSSARY.md` | `oma` stays the name of the program that runs a Bundle. Record that the command is `odk oma`, the file is `odk-oma` off `PATH`, and the `odk` crate is not that command. Do not call the dispatcher an OMA runtime. |
| `packaging/README.md`, `packaging/systemd/user/oma-omarchy.service` | `ExecStart` is `/usr/lib/odk/odk-oma omarchy sync`. |
| `CLAUDE.md` | The command examples. `odk-cli` is named on the ODK side of the crate split. |

ADR 0010 does not change: the Bundle program still verifies, installs, registers, sandboxes and starts, and still does not update applications or compile them. The dispatcher adds no network and no build.

ADR 0050 and ADR 0051 live on `origin/atmos-v1.0.0-rc` and are not in this branch. If they land, `oma dev` is `odk oma dev`. The dispatcher does not gain a built-in `dev`. They are not edited from here.

## Decisions

The owner locked these on 2026-10-25. Nothing in this list is still open.

- No command named `oma`. `odk oma` is the front door. The Bundle program's messages are prefixed `odk oma:`.
- `odk-oma` is not on `PATH`. A system or package install puts it at `/usr/lib/odk/odk-oma`. `--user` puts it at `~/.local/lib/odk/odk-oma`. A development build finds it next to `odk`.
- `oma` is a built-in. The dispatcher finds it only in that private directory, never on `PATH`. Any other `odk-<name>` is taken from the private directory first, then from absolute `PATH` entries, skipping `.` and the current directory.
- The package name is `odk`. ADR 0054 had named it `oma` because that was the command. `/usr/lib/$pkgname/` is then `/usr/lib/odk/`.
- Desktop entries, generated entries and the mount action call `/usr/bin/odk oma …`. A per-user install uses `~/.local/bin/odk oma …` the same way. systemd, `binfmt_misc`, the `install.sh` handoff, and `self-upgrade` / `self-uninstall` call the private binary by absolute path.

## Recorded findings

These are not open questions. They are what was checked, and the limits of the check.

### The command name `odk`

Official Arch repositories, exact package name `odk`: `https://archlinux.org/packages/search/json/?name=odk` returned `count: 0`. The same search with `q=odk` also returned no results, so no official package name or description contains that string. This is not a scan of every file inside every package. A `/usr/bin/odk` owned by a package whose name and description never mention `odk` was not ruled out by a pkgfile walk; nothing in the name search suggests one.

AUR RPC, exact `https://aur.archlinux.org/rpc/v5/info/odk`: `resultcount` 0. A substring search for `odk` returned `podkit`, `modkit-bin`, `r-podkat`, `python-vodka` and `autohidewibox`. None is a package or command named `odk`.

Omarchy's command directory `bin/` on `omacom/omarchy`, branch `quattro` (the repository GitHub now serves for basecamp/omarchy; 558 files): no file named `odk` and none whose name starts with `odk`. Those files install as `/usr/bin/omarchy-*`. The package list `omacom/omarchy-pkgs` `pkgbuilds/` (209 names, branch `master`) has no `odk`. It does have `omacalc`, `omacut` and the other `oma*` tools ADR 0026 already noted. Those are not the command `odk`.

Open Data Kit (getodk.org) uses the same initials. Its server CLI inside the container is `odk-cmd` (ODK docs, Central command line). That is not `/usr/bin/odk` on an Omarchy machine. Web search for the bare word will be noisy in the way ADR 0026 already accepted for AOSC's `oma`. It is not a `PATH` collision.

### Findings for `ODK`

Android's documented variables are `ANDROID_HOME`, `ANDROID_USER_HOME` and the rest of that table (Android developer docs, "Environment variables"). The NDK is `NDK_HOME` in the AUR `android-ndk` discussion, not `ODK`. OpenDDS's Android build uses `DDS_ROOT`, `NDK` and `SDK` (OpenDDS developer guide, building for Android). ODK Central's `.env` uses `DOMAIN`, `SYSADMIN_EMAIL`, `DB_HOST` and PostgreSQL's variables (ODK docs, installing Central). None of those pages sets `ODK`.

The name is still the Open Data Kit acronym. Overwriting a value a user set by hand is the only clash this has. Nothing checked requires the variable, so the dispatcher does not switch to another spelling.
