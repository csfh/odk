---
status: proposed
---

# The `odk` command dispatches to `odk-<name>`, and the Bundle program is `odk-oma`

The owner asked to make the oma CLI a facade, as `odk oma`, and then `odk-oma`. This record is that shape, on the model of `git` and `cargo`: one short umbrella command looks up a separate program and replaces itself with it. `odk oma <args>` runs the program `odk-oma`, which is today's `oma` binary under a new file name. `odk <name> <args>` runs `odk-<name>` the same way. The umbrella does not grow the Bundle commands, and it does not link the library applications depend on.

Nothing here is built. The binary on disk is still `oma`.

The number is 0056. `origin/atmos-v1.0.0-rc` (there is no tag of that name; the only tag fetched was `v0.0.1`) already has ADR 0050 and ADR 0051, which are not in this branch. ADR 0052 to ADR 0055 are the single-file Bundle records on this branch. 0056 is the next free number after 0055 that those lines do not use.

One question is left to the owner and is not decided here: whether a short command spelled `oma` stays on the machine, beside `odk` and `odk-oma`. The options, and a recommendation, are at the end. Everything else in this record is the proposal.

## What the umbrella is

`odk` is a dispatcher. It understands a handful of its own flags, and otherwise it finds an external program and executes it. It has no Bundle subcommands of its own. `run`, `install`, `bundle`, `mount`, `self-upgrade` and the rest stay inside `odk-oma`, reached as `odk oma run` or by running `odk-oma` directly. `odk run` looks for a program named `odk-run`. It does not forward to `odk oma run`.

That split is the point of the facade. The dispatcher stays small enough to read, a new `odk-<name>` appears on `PATH` without a change to `odk`, and the program that verifies a Bundle (ADR 0055) stays a separate file the desktop and the kernel can name absolutely. Folding `odk-oma` into the dispatcher, or teaching the dispatcher each Bundle flag, would make the facade the program it is only supposed to find.

`git` and `cargo` are the precedent. Both run an external `tool-<name>` when the name is not one of their own commands, both replace the process on Unix, and both let a built-in command win over an external program of the same name. `git` looks in its own exec-path before `PATH`, so the helpers installed beside it win over a copy found later. This command does the same. Cargo's manual only requires the external program to be on `PATH`; the own-directory rule below is deliberate, and it is there so a system `odk` cannot be paired with a different `odk-oma` that happens to sort earlier on `PATH`.

## Dispatch

The first argument decides. No argument parsing is done on the rest.

| Invocation | What happens |
|---|---|
| `odk`, `odk help`, `odk --help`, `odk -h` | Dispatcher help on standard output, exit 0. |
| `odk help <name>` | Same lookup as `odk <name>`, then that program is executed with a single argument `--help`. |
| `odk --version`, `odk -V` | The dispatcher's own version, one line, exit 0. Does not run `odk-oma`. |
| `odk --list` | Discovered external commands, exit 0. |
| `odk <name> [args...]` | Look up `odk-<name>` and execute it with `[args...]`. |
| `odk <name> --help`, `odk <name> --version` | Ordinary passthrough. The dispatcher does not read those flags. |

A built-in wins over an external program. `odk help` never runs `odk-help`. `odk --version` never runs `odk-version`. There is no built-in named `oma`, `list` or `version`: `odk list` looks up `odk-list`, and `odk version` looks up `odk-version`. `--list` and `--version` are flags so those names stay available.

Anything whose first argument starts with `-` and is not exactly `--help`, `-h`, `--version`, `-V` or `--list` is a usage error. Flags of the external program go after the name: `odk oma --flag`, not `odk --flag oma`. `--` is not a name and does not introduce one.

`odk help` with more than one following argument is a usage error. `odk --help`, `odk --version` and `odk --list` take no further arguments. Extra words are a usage error, not ignored.

### Names

A name is one token: an ASCII lowercase letter, then up to 63 characters from `[a-z0-9-]`. Upper case, underscore, dot, space and non-ASCII are rejected. Empty is rejected.

The token is not a path. `/`, `\` and a NUL byte are rejected before any directory is opened. `.` and `..` are not names. The dispatcher never joins the token onto the current directory, never treats it as a relative path, and never executes a path the user typed. `odk /usr/bin/odk-oma` and `odk ../odk-oma` are usage errors, not launches.

The file name looked up is exactly `odk-` plus that token, with no added suffix (Linux has none).

Invalid name, unknown flag, or a built-in with unexpected arguments: one line on standard error, exit 2. Nothing is executed.

### Where it looks

Search directories, in order, first regular file wins:

1. The directory containing the `odk` executable, from `current_exe`, when that path is absolute. This is the pairing rule. `/usr/bin/odk` runs `/usr/bin/odk-oma` even when `~/.local/bin` is earlier on `PATH` and holds another `odk-oma`. A developer build finds `target/debug/odk-oma` beside `target/debug/odk` the same way.
2. Each entry of `PATH`, left to right. An empty entry is skipped (an empty entry means the current directory). `.` is skipped. A relative entry is skipped. A duplicate of directory 1 is skipped.

The current directory is not added. A `PATH` the user wrote to include it is still not honoured: the skip is the rule, not an accident of the default `PATH`.

The candidate is that directory plus `odk-<name>`. Symlinks are followed for the file-type check. A directory, socket or other non-regular file is skipped, and the search continues. The first regular file is the one that is executed, even if its mode is not executable: the exec error is reported, and a later copy is not tried in its place.

`execvp` of a bare `odk-<name>` is not used. That would search `PATH` again, and it would search the current directory on a `PATH` that contains `.`. The executed path is the absolute path that was selected.

If `current_exe` fails, directory 1 is omitted and the `PATH` walk stands. The not-found message says the dispatcher's own directory could not be read.

### Arguments, process, signals

Arguments after the name are passed through unchanged, as raw arguments, including `--`, empty arguments, and bytes that are not UTF-8. The name itself is not forwarded. `odk oma run --wait ./App.oma` executes `odk-oma` with `run`, `--wait`, `./App.oma`.

`argv[0]` is the absolute path of the file that was selected. It is not rewritten to `oma`, to `odk`, or to the dashed short name. A program that wants its own path reads that argument or `current_exe`. Both point at `odk-<name>`.

On Linux the dispatcher replaces itself (`exec`). There is no spawn-and-wait. The exit status of a successful dispatch is the external program's status, because it is the same process. A signal delivered to the command is delivered to that program. Standard input, output and error are the descriptors already open. The working directory, the environment, the process group and the session are unchanged. The dispatcher installs no signal handlers and does not call `setsid`.

Spawn-and-wait was rejected. It would make `odk` the parent of `odk-oma` for the whole command, it would have to forward signals and to invent an exit code when the child died by signal, and during `odk oma run` it would make `odk` an ancestor of the application until `odk-oma` returned. ADR 0034 keeps `oma` from being that ancestor. The dispatcher must not add a new one. `exec` does not.

Before the exec, the environment variable `ODK` is set to the absolute path of the dispatcher. A value the caller already had is replaced, so a subcommand sees the `odk` that launched it. No other variable is added or removed. Subcommands may ignore `ODK`. `odk-oma` does not need it to find `oma-consent`.

If the real user or group id differs from the effective id, the dispatcher refuses and exits 126. It is not a setuid program. A `PATH` search from a setuid binary is a different program, and this one will not be that.

### Help, version, list

Dispatcher help names the usage, the built-ins, the own-directory rule, and every external name discovered, in byte order. It tells the reader that Bundle commands are `odk oma` and that `odk oma --help` prints them. It does not execute any `odk-<name>` to ask for a description.

`odk help <name>` is the same exec as `odk <name> --help`. The exit status is whatever that program's `--help` returns. A missing name uses the not-found path below, including suggestions.

`odk --version` prints `odk` and the dispatcher's version. `odk oma --version` is passthrough and prints `odk-oma` and that program's version. The two lines are not the same line. They carry the same release version when both files came from one install (the layout section). The dispatcher does not run `odk-oma` to check, and a mismatch does not change dispatch.

`odk --list` prints one external command per line, `name`, a tab, the absolute path selected, in byte order of the name. Built-ins are not listed; they are not files. A name found in an earlier directory hides the same name later, and the hidden path is not printed. Files whose suffix is not a valid name are ignored, so a stray `odk-Foo` or `odk-foo_bar` is neither listed nor runnable.

### When the program is missing

No regular file in the search list:

```
odk: no such command: '<name>'
```

If any discovered name is within a Levenshtein distance of 2, up to three of them are printed, closest first, as `Did you mean: odk <name>?`. Distance is computed on the name, not the file name. When `oma` itself was discovered, the not-found text also says that Bundle commands are `odk oma <command>` and that `odk oma --help` lists them. That sentence is omitted when `odk-oma` is not installed, so the dispatcher does not advertise a program it did not find. A second line points at `odk --list`.

Exit 127. This is distinct from a usage error (2) and from a failure of the program itself, which returns its own status through `exec`.

The file was found and `exec` fails: exit 126, standard error names the absolute path and the system error. `ENOENT` at that moment (the file disappeared) is exit 127 with the same path. The search is not resumed.

A missing `odk-oma` is this path with `<name>` equal to `oma`. There is no special exit code for it.

## Crates and binaries

The dispatcher and the Bundle program are two binaries in two crates. The library applications already depend on stays a library.

| Crate | Binary | What it is |
|---|---|---|
| `odk-cli` (new), `crates/odk-cli` | `odk` | The dispatcher. Its code uses only the standard library. The package depends on the workspace `deps` crate, as every member does, and on nothing else: not `oma`, not `odk`, not clap. |
| `oma` (existing), `crates/oma` | `odk-oma` | Today's library and today's CLI, with the binary target renamed from `oma` to `odk-oma`. Clap, the subcommands, the sandbox and the init stay here. |
| `odk` (existing) | none | The facade library (ADR 0028). It does not gain a binary. |
| `oma-consent` (existing) | `oma-consent` | Unchanged. Still the dialog helper (ADR 0025, ADR 0033). |

The `odk` library crate does not host the command. Applications depend on that crate. A binary in the same package would put the command's dependencies on the graph of every application, and it would invite the dispatcher to call into ODK. `odk-cli` is a workspace member and is not published as a library. `oma` does not depend on `odk-cli` or on `odk`. ADR 0013 stands: the OMA crates and the ODK crates do not depend on each other. `odk-cli` is on the ODK side of that line and depends on neither.

What moves out of `crates/oma` is the file name of the binary, nothing else. `main.rs` stays the command implementation. Subcommand arguments stay in that clap definition. The dispatcher does not import them and does not keep a second copy. Sharing is the passthrough: the words after `oma` are the arguments `odk-oma` already defines.

What the two binaries share about version is the release, not a Rust module. ADR 0054's `current.json` field `oma_version` remains the version of the runtime as a whole. A release builds `odk`, `odk-oma` and `oma-consent` from the same tag and installs them together. Each `--version` prints its own file name and that same version. When those crates are first given a release version, they take one workspace `version` so the two strings are not edited apart. There is no `odk-version` crate. This record does not bump the `0.0.0` the crates have today.

`odk-oma` still finds `oma-consent` beside its own executable (`current_exe`), as ADR 0025 says. The dispatcher is not on that search. Install puts `oma-consent` in the same directory as the `odk-oma` file. A symlink used for a short name, if the owner keeps one, is not the directory the helper is looked up in; the resolved file is.

The init stays a re-exec of that same file. The launcher already holds the binary open and starts the init with `execveat` and `AT_EMPTY_PATH`, passing `--init` (ADR 0024). That open file is `odk-oma`. The init does not search `PATH` and it does not start the dispatcher. The source today passes the string `oma` as that exec's `argv[0]`. That string is a label inside the sandbox, not a lookup. Acceptance may leave it or set it to `odk-oma`. It does not follow the caller's symlink: `current_exe` is the resolved `odk-oma` file, and the user-facing prefix belongs to the outer process, which is the short-name section below. `--init` remains a private flag of `odk-oma`, not of `odk`.

`cargo run -p odk-cli -- oma …` works once both binaries have been built, because Cargo places them in the same target directory and the own-directory rule finds `odk-oma` there. Tests that name `CARGO_BIN_EXE_oma` would name `CARGO_BIN_EXE_odk_oma` when the target is renamed. That rename is part of accepting this record, and it is not done here.

## The runtime this command has to fit

ADR 0052, ADR 0053, ADR 0054 and ADR 0055, and `docs/spec/oma-cli.md`, describe a program called `oma` at `/usr/bin/oma`. The program they describe is `odk-oma`. The dispatcher is a second file. The paths the kernel and systemd already use as absolute paths name `odk-oma` directly, so they do not depend on the short name the owner has not chosen.

### Desktop entry and generated entries

The system handler ADR 0053 writes, and the per-Bundle entries ADR 0014 generates, run the Bundle program by absolute path. The system file's `Exec` is `/usr/bin/odk-oma run %f`, and its Show-contents action is `/usr/bin/odk-oma mount --open %f`. A per-user install writes the absolute path it actually installed, `~/.local/bin/odk-oma`, into the user desktop file and into the units, which is the rewrite ADR 0054 already does for `ExecStart`.

`--register` writes that same absolute path, the `current_exe` of the `odk-oma` that is registering, into each generated entry. It does not write a bare `oma`. A bare name is a `PATH` search at click time, and `~/.local/bin` ahead of `/usr/bin` would then be the verifier (ADR 0054 already warns about that shadow; ADR 0055 says a per-user copy is not the unwritable verifier). The absolute path is the fix, and it is the same path under every option of the short-name question.

`NoDisplay=true` stays. The launcher entries people click are still the per-Bundle ones. Flea, which is not in this repository, should call the same absolute program rather than a bare `oma` on `PATH`. Until it does, a Flea that searches `PATH` has the shadow the desktop file is here forbidden to have.

### `binfmt_misc`

The kernel's interpreter path has to be absolute. A relative path is rejected, and the kernel does not search `PATH`. ADR 0053's registration stays `/usr/lib/oma/binfmt` as that interpreter: a root-owned script, magic `OMA1` at offset 0. The script's only job becomes:

```
#!/bin/sh
exec /usr/bin/odk-oma run -- "$1"
```

The script does not exec `odk`, and it does not contain `odk oma`. The dispatcher would treat the Bundle path as a command name (a slash, so a usage error), and a `PATH` lookup inside the script would be the shadow the absolute path exists to avoid. A per-user install still cannot register `binfmt_misc` (ADR 0054). The executable bit is still required.

The script path `/usr/lib/oma/binfmt` does not change. The file it execs does: `odk-oma`, not a name that might only exist as a symlink.

### Units

`ExecStart` is `/usr/bin/odk-oma` plus the same arguments as today (`omarchy sync`, and `--register` for the oneshot). Per-user, the rewritten path is `~/.local/bin/odk-oma`. The units do not call `odk oma`. systemd's environment is not a login `PATH`, and the dispatcher would be an extra process image for no decision.

### `self-upgrade` and `self-uninstall`

Both commands live in `odk-oma`, as `oma self-upgrade` and `oma self-uninstall` do in ADR 0054. The spelling on a shell is `odk oma self-upgrade` or `odk-oma self-upgrade`, plus a short `oma self-upgrade` only if the owner keeps that name. `install.sh` hands off by the absolute path below, not by a `PATH` search for `oma`.

They upgrade or remove the runtime as one release. The set is:

| System path | Per-user path |
|---|---|
| `/usr/bin/odk` | `~/.local/bin/odk` |
| `/usr/bin/odk-oma` | `~/.local/bin/odk-oma` |
| `/usr/bin/oma-consent` | `~/.local/bin/oma-consent` |
| `/usr/lib/oma/binfmt` | not installed |
| `/usr/lib/binfmt.d/oma.conf` | not installed |
| the MIME, desktop and unit files ADR 0054 already lists | the per-user copies ADR 0054 already lists |
| `/var/lib/oma/runtime.toml` | `~/.local/share/oma/runtime.toml` |

A short `oma` entry, if the owner keeps one, is in the set too. It is not in the set until that decision.

Replacing `odk-oma` and leaving the dispatcher, or the reverse, is not an upgrade. Both binaries are in the tarball, both are renamed into place, and both are checked afterwards. The check ADR 0054 runs, the new program's `--version`, is run twice, by absolute path: `odk --version` and `odk-oma --version`. Each must exit 0, and each line must contain `oma_version` from `current.json` as a word. The file names in the two lines differ. A tarball whose two binaries were not built as that version is refused before any destination path is changed, the same way a mismatch between `oma` and `oma-consent` is refused today. Rollback copies the whole set, including `odk`, into `previous-runtime/`.

`self-upgrade` and `self-uninstall` refuse when pacman owns any of `/usr/bin/odk`, `/usr/bin/odk-oma`, `/usr/bin/oma-consent`, or `/usr/bin/oma` when that path exists. They only replace or delete the absolute paths in the table for the mode that wrote the receipt. They do not walk `PATH` and they do not remove a Cargo target directory.

The receipt's fields are unchanged. `oma_version` means this set. `method = "script"` means the script installed it. There is no second pin for the dispatcher. ADR 0055's rules for the runtime key apply to the tarball as they do today: one key, a dark domain installs nothing, a different key is a re-verification.

### `install.sh`

ADR 0054's script is the bootstrap. The OS check, the key fetch, the pause, the signature on `current.json`, and the hash of the tarball are unchanged. The tarball gains `odk` and carries `odk-oma` where it carried `oma`. It still carries `oma-consent`, the binfmt script, the units, `oma.xml` and `oma.desktop`.

The handoff, when a receipt for this domain is already present, execs the absolute `odk-oma` of that mode (`/usr/bin/odk-oma` or `~/.local/bin/odk-oma`) with `self-upgrade`. It does not search `PATH` for `oma`. A script that only looked for the short name would skip a machine whose owner had dropped it, and a `PATH` hit in a user-writable directory would be the wrong program to trust with the upgrade.

The desktop file and the binfmt script shipped in the tarball already contain the absolute `odk-oma` paths above. Per-user install rewrites those paths to `~/.local/bin/odk-oma` as it rewrites `ExecStart` today.

The package name stays `oma` until the owner says otherwise. ADR 0054 gave Omarchy's repository that name. The package installs the set in the table. Whether the short command is part of the package is the open question.

### Trust

ADR 0055 is unchanged in its three layers. The bytes that verify a Bundle are `odk-oma`. The system copy at `/usr/bin/odk-oma` is the unwritable verifier that ADR talks about; the sentence there that names `/usr/bin/oma` names this file once the record is accepted. A symlink called `oma`, if it exists, is not a second verifier. It is another directory entry for the same file, and only when someone invokes that entry.

The dispatcher is not a verifier. It never reads a Bundle. A replaced `odk` can only do harm by running a different program, which the own-directory rule blocks for the `odk` that lives beside the system `odk-oma`. An attacker who can write `/usr/bin` can already replace the verifier. An attacker who can write `~/.local/bin` can still plant an `odk` there, and a user who runs that `odk` gets that dispatcher's choice of program. The desktop file, the units and `binfmt_misc` do not go through that `odk`. They name `/usr/bin/odk-oma`.

External `odk-<name>` programs other than `oma` are as trusted as `PATH`. There is no signature check on them. That is the same rule cargo has for `cargo-<name>`, and it is accepted here. They are not Bundles and they are not this runtime's tarball.

### `docs/spec/oma-cli.md`

The spec stays the surface of the Bundle program. Acceptance adds an invocation section: the binary is `odk-oma`; `odk oma` passes every following argument through and is the same program; absolute paths in desktop files, units and the binfmt script call `odk-oma`; `self-upgrade` and `self-uninstall` cover the set in the table. The command tables keep one spelling. Which spelling, `oma` or `odk-oma`, waits on the owner. The spec does not gain `update`, `list` or `uninstall` of a Bundle (ADR 0010). The dispatcher does not gain a spec file of its own; this record is the normative text for dispatch until it outgrows one.

## The short command `oma`

**Not decided.** ADR 0026 made the command `oma` because `omalaunch` was long to type and because the state directory, the units and the format already used the prefix. Desktop files, the binfmt script, the units, the glossary and `docs/spec/oma-cli.md` all say `oma`. This facade adds `odk` and renames the file to `odk-oma`. Whether a person can still type `oma` is the owner's call. The three options below all keep `odk` and `odk-oma`. They differ on the extra name.

The integration paths in the previous section do not wait on the call. binfmt, systemd and the desktop `Exec` key name `/usr/bin/odk-oma` either way. The short name is how a person invokes the program from a shell, and how existing notes and ADRs spell it.

### Option A — `oma` is a symlink to `odk-oma`, for everyone

`install.sh` and the package create `oma` as a symlink to the `odk-oma` file in the same directory (`/usr/bin/oma` → `odk-oma`, and `~/.local/bin/oma` → `odk-oma` for `--user`). `cargo build` does not create that symlink. It produces `odk` and `odk-oma`, and `odk oma` finds the second because they share a directory. A developer who wants the short name on `PATH` makes the same symlink the package makes. The workspace does not grow a third binary to do it.

Invoking the symlink sets `argv[0]` to the path the user typed. The file name is `oma`, and `odk-oma` uses that file name for its prefix and its help title. Invoking `odk oma` or `odk-oma` sets `argv[0]` to the `odk-oma` path, and the prefix is `odk-oma`. `current_exe` follows the symlink to `odk-oma`, so `oma-consent` is still found beside the real file. `self-upgrade` replaces `odk-oma` by the usual temporary-name-and-rename; the symlink points at the name, so it sees the new file. No hardlink: a rename leaves the old inode under the other name, and ADR 0054's upgrade would keep running the previous binary as `oma`.

A shell shim (`exec /usr/bin/odk-oma "$@"`) is the worse form of the same option. It is a second program, `argv[0]` becomes `odk-oma` unless the shim uses `exec -a`, and pointing binfmt at a script that points at a script adds a failure the symlink does not have. A tiny extra binary that only execs `odk-oma` is a third artifact in the tarball. The symlink is the mechanism, if this option is taken.

| | |
|---|---|
| End-user ergonomics | `oma run` still works. `odk oma run` also works. The short command ADR 0026 chose survives. |
| binfmt and desktop | Already absolute `odk-oma` in this record, so they work if the symlink is deleted. Old notes that say `oma run` still match the shell. |
| Existing docs | The command tables can keep saying `oma`, with one line that the file is `odk-oma` and that `odk oma` is the same. |
| Discoverability | Two spellings. `odk --list` shows `oma`. A person who only learns `oma` may never see the umbrella. |
| Collision | The AOSC package manager named `oma` is the collision ADR 0026 already accepted. The symlink does not add a new one. `odk` is a separate, unchecked name (open questions). |

### Option B — `oma` only on the end-user install

`install.sh` and the `oma` package create the symlink. A developer build does not: `cargo build` produces `odk` and `odk-oma` only. Developers type `odk oma` or `odk-oma`. End users type `oma`.

| | |
|---|---|
| End-user ergonomics | The same short command as today, on a machine that ran the script or pacman. |
| binfmt and desktop | Absolute `odk-oma`, so an end-user machine works. A developer machine that never ran `install.sh` has no `oma` and no `binfmt` registration, which is already true: binfmt is the system install. |
| Existing docs | Have to say which audience types which word. Easy to write the wrong one in a unit file or a test. |
| Discoverability | End users do not meet `odk` unless they read past the symlink. Developers do not have the command the docs use, unless the docs are split. |
| Collision | Same `oma` collision on end-user machines only. A developer `PATH` does not gain `oma`. |

The variant where end users do not get `odk` at all (the package ships `oma` and `oma-consent` only) is this option pushed further. `self-upgrade` would then carry two shapes of tarball, and `odk oma` would not exist on the machines the script is for. That variant is not recommended.

### Option C — no `oma` command

Nothing is installed under the name `oma`. Shell, docs, and any remaining bare names say `odk oma` or `odk-oma`. Desktop, binfmt and units are already on the absolute path in this record.

| | |
|---|---|
| End-user ergonomics | `odk oma run` is longer than `oma run`, which is the length ADR 0026 removed. `odk-oma run` is a different, easy-to-mistype name. |
| binfmt and desktop | One file, one path, no symlink to explain. The strongest of the three for "the path in the unit is the path you type", if people type `odk-oma`. They will not; they will type `odk oma`, which is a different string from the absolute path. |
| Existing docs | Every `oma run`, `oma bundle`, `oma self-upgrade` and generated `Exec` example changes. ADR 0026's decision is reversed. |
| Discoverability | One umbrella. `odk --list` is how the Bundle program is found. Nothing answers `command -v oma`. |
| Collision | The `oma` collision goes away. The unchecked `odk` collision remains, and it becomes the only command. |

### Recommendation, pending the owner's decision

**Option A, with the symlink and not a shim or a hardlink.**

End users keep the command ADR 0026 settled. The kernel, systemd and the desktop file do not use that name: they use `/usr/bin/odk-oma`, so a `PATH` shadow cannot become the verifier and a missing symlink cannot break `binfmt_misc`. Developers get `odk oma` from the same pair of files, including from `target/debug` without a separate install. Docs keep the short spelling and gain one sentence for the facade. Option B splits the audience across two spellings. Option C spends the short name ADR 0026 exists to protect, for a cleanliness the absolute paths already provide.

This is a recommendation. The record is not accepted on the strength of it, and the other two options stay open.

## If this is accepted, what else is edited

Not edited in the proposal. On acceptance, these gain the changes named here.

| Document | Change |
|---|---|
| ADR 0026 | The binary's file name is `odk-oma`, and `odk oma` runs it. The short command `oma` is kept, dropped, or limited to the end-user install, per the owner. The init line stays a re-exec of the Bundle program. |
| ADR 0031 | The `oma` crate remains one library and one binary; the binary target is `odk-oma`. The dispatcher is a different crate, `odk-cli`, and is not a reason to split the format code out. |
| ADR 0028 | One sentence: the `odk` crate stays a library, and the `odk` command is `odk-cli`. |
| ADR 0025, ADR 0033 | "Beside its own executable" means beside the `odk-oma` file. `oma consent` is that program's subcommand. |
| ADR 0024, ADR 0034 | The init is a re-exec of `odk-oma`. The dispatcher `exec`s and is not an ancestor of the application. |
| ADR 0014 | Generated `Exec` lines are the absolute path of the `odk-oma` that ran `--register`, not a bare `oma`. |
| ADR 0044 | The unit `ExecStart` paths become `/usr/bin/odk-oma`. The copy the sandbox shows is unchanged. |
| ADR 0053 | The handler `Exec`, the Show-contents action, and the binfmt script's exec target are the absolute `odk-oma` paths in this record. The public shell command is whatever the owner decided. |
| ADR 0054 | The tarball, the path table, the handoff, the `--version` check, pacman ownership and uninstall cover `odk` and `odk-oma` as one set. `oma_version` stays the field name. The package name stays `oma` unless the owner changes it with the short-name decision. |
| ADR 0055 | The unwritable verifier is `/usr/bin/odk-oma`. The trust rules are the same. |
| ADR 0032 | The assembly command's spelling follows the short-name decision. The behaviour does not change. |
| `docs/spec/oma-cli.md` | The invocation section in this record. Command-table spelling follows the owner. |
| `GLOSSARY.md` | `oma` stays the program that runs a Bundle. Add the file name `odk-oma`, the dispatcher command `odk`, and that the `odk` crate is not that command. Avoid calling the dispatcher an OMA runtime. |
| `packaging/README.md`, `packaging/systemd/user/oma-omarchy.service` | `ExecStart` names `/usr/bin/odk-oma`. |
| `CLAUDE.md` | The command examples. The rule that OMA and ODK crates do not depend on each other now names `odk-cli` on the ODK side. |

ADR 0010 does not change: the Bundle program still verifies, installs, registers, sandboxes and starts, and still does not update applications or compile them. The dispatcher adds no network and no build.

ADR 0050 and ADR 0051 live on `origin/atmos-v1.0.0-rc` and are not in this branch. If they land, `oma dev` and the daemon unit names are subcommands and strings of the Bundle program. The dispatcher does not gain a built-in `dev`. Their command spelling would follow the short-name decision. They are not edited from here.

## Open questions

1. **The short command `oma`.** Options A, B and C above. The recommendation is A, and it is not a decision.
2. **The package name**, if the owner rejects A. ADR 0054 named the package `oma`. Option C makes that name a package with no command of the same spelling. Recommendation with A: leave the package name `oma`.
3. **The prefix on standard error.** `odk-oma` today would inherit `oma:` from the current binary. Recommendation: the prefix and the help title follow the file name of `argv[0]`, so the symlink says `oma:` and `odk oma` says `odk-oma:`. Confirming that is part of accepting A.
4. **Whether `odk` is already a command on Omarchy or Arch.** Unchecked. A collision would mean the umbrella needs a different file name, and the `odk-<name>` prefix with it. This record does not pick a fallback.
5. **The environment variable `ODK`.** It names the dispatcher that just ran. If that name is already used by something a Bundle or a developer sets, the variable needs a different spelling. Nothing in this repository reads `ODK` today.
