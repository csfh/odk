---
status: accepted
---

# The unsigned Bundle directory is retired, and a deleted Bundle file is cleaned up after

ADR 0098 makes criterion 2 of v1 "a Bundle is a signed `.oma` file, and nothing else". The code does not do that yet. ADR 0052 superseded ADR 0022 (a Bundle is a directory), but the directory path was never taken out, and it runs with no signature. This record says what is removed, what is kept, and what happens to a directory installed before v1. The owner added a second question on 2026-11-13: when a Bundle file is deleted from `~/Applications`, what it leaves behind should be cleaned up too. The owner accepted both answers on 2026-11-13.

## What is true (read from the code on 2026-11-13)

- **`odk oma bundle` already writes only a file.** `--sign`, `--root` and `--release` are required, and it always calls `bundle::write_file`. The directory it assembles is a staging step that is removed once the file is signed. Only its help text still says `<Name>.oma/`.
- **Four commands still take a directory**, and none of them checks a signature:
  - `inspect` takes a file or a directory.
  - `run` treats any path that is not a file as a directory and runs it in the sandbox from a bind mount (`App::Directory`).
  - `consent` takes *only* a directory or the Identifier of an installed directory.
  - `install` installs a file, or copies a directory to `~/Applications/<Name>.oma/`, with its consent in `state/grants/<id>.toml`, its data in `state/data/<id>` and its inbox in `state/inbox/<id>`.
- **An installed directory has its own launcher and layout.** `--register` writes a `.desktop` entry for it. `run <id>` finds it by Identifier, and its daemon runs from `<root>/bin/d`. Rollback does not know it. `purge` refuses while it is installed and, once it is gone, removes only the publisher's pair, never the directory-era state.
- **The two kinds do not see each other.** A file install does not notice a directory with the same Identifier under another name, and a directory install does not notice a file.
- **One conversion exists.** `odk oma dev` replaces an installed directory of the same Identifier with the signed file it builds, and moves `state/data/<id>` into the pair. It is tested, including putting everything back when a step fails.
- **What deleting a Bundle file leaves behind.** `oma-applications.path` runs `odk oma --register` whenever `~/Applications` changes, and it reconciles (ADR 0009): a deleted file loses its `.desktop` entry and icon, and its `oma-inbox@` watch is disabled. Everything under its pair stays: the data, the previous file kept for rollback (ADR 0011), the inbox, the daemon program written out of the image (ADR 0084), any held file, and the record holding the pin, the consent and the high-water mark (ADR 0060). Only `odk oma purge` removes them.
- **Who has an installed directory.** Only a machine that ran `odk oma install <dir>` before v1: the owner's development machine, and anyone who installed `v0.1.0-rc.1` to `rc.3`.

## Decision

1. **Every command takes a signed Bundle file, or the Identifier of an installed Bundle file, and nothing else.** A directory given to `inspect`, `run`, `consent` or `install` is refused before anything is read, with the message `odk oma: <path> is a directory; a Bundle is a signed .oma file (odk oma bundle --sign, or odk oma dev)`.
2. **`consent` works on installed Bundle files.** `odk oma consent <id>`, or the path of the installed file, asks for and records the consent of the installed copy's pair, as `dev` does. A file that is not installed is refused with a pointer to `odk oma install`: recording consent for it would pin its publisher's root, which only the first-install flow does (ADR 0060).
3. **Directory installing and running are removed from the code.** That covers `install::install`, the directory branch of `launch::resolve`, `App::Directory` and its bind, the daemon from `<root>/bin/d`, `list_directory`, the directory entries of `--register`, the `state/inbox/<id>` binding, and the directory consent record `state/grants/<id>.toml`. A Bundle file's sandbox, daemon (ADR 0084) and layout are unchanged. One read-only helper stays: it finds a `*.oma` directory in `~/Applications` by its Identifier, for decision 5 and nothing else, and nothing it finds is opened to be run. The directory-era state paths (`state/data/<id>`, `state/inbox/<id>`, `state/grants/<id>.toml`) stay in the layout for the same reason.
4. **Assembly stays a private step of signing.** `bundle::assemble` and the directory validation in `oma::fmt` stay, used only by `bundle::write_file` (and so by `bundle --sign` and `dev`). They are not public and not reachable from the command line. The help text of `bundle` says it writes `<Name>.oma`.
5. **A directory installed before v1 is never run again, and its data is never deleted without being asked.**
   - `odk oma run <id>` refuses it with the same line `--register` prints (below).
   - `--register` no longer lists it, so its `.desktop` entry is removed with the other stale entries, as `desktop::remove_others` already does. For each `*.oma` *directory* in `~/Applications` it prints one line on standard error, `odk oma: <path> is a Bundle directory from before v1 and is not run; odk oma purge <id> removes it and its data`. It does not change or delete the directory.
   - `odk oma purge <id>` removes that directory and its directory-era state (`state/data/<id>`, `state/inbox/<id>`, `state/grants/<id>.toml`), with any pair the Identifier has. An Identifier that only ever had a directory, and so has no pair, is purged too, where today the command says there is nothing to purge. This changes purge's contract: for a Bundle file it still refuses while the file is in `~/Applications`, but for a directory it removes the directory itself, because a directory can no longer be run or upgraded.
   - `odk oma dev` keeps its replacement of a directory, moving its data into the pair. It is the one path that carries a directory's data forward, and it is already tested.
6. **A deleted Bundle file is cleaned up after by `--register`.** A pair whose app id has no Bundle file in `~/Applications` under that root any more is uninstalled, whatever removed the file (a person, or `--register` itself deleting a copy that no longer verifies). `--register` then, besides what it does today:
   - stops the application's daemon unit and socket, if it has one, and removes the daemon program written out of the image;
   - deletes the previous file, the inbox and anything staged in it, and any held file for that pair;
   - **moves the data directory to the user's Trash**, following the freedesktop.org Trash specification: into `$XDG_DATA_HOME/Trash/files/` under a name such as `<Name> data`, with a `.trashinfo` naming its original path, so a file manager can show it and restore it to where it was. The Trash and the data are both under `$XDG_DATA_HOME`, so this is a rename on one filesystem and needs no tool. If the move fails, the data is left where it is and the reason goes to the journal. `--register` never deletes data;
   - **keeps the record**, which holds the pin, the consent and the high-water mark. It is small, the high-water mark is what refuses an older file installed later (ADR 0060), and a reinstall from the same publisher finds its pin and its consent and asks no question it was already answered.
   - Each of these is logged with what was removed or moved. The same happens at login if the file was deleted while the session was not running, through the one-shot `oma-register.service`.
   - `odk oma purge <id>` stays the command that removes everything, the record included, and data still under the pair is deleted by it, not moved to the Trash.

7. **The tests move to signed files.** The tests that install or run a directory (`tests/consent.rs`, `tests/sandbox.rs`, `tests/daemon.rs`, `tests/inspect.rs`, and the unit tests of `install.rs`, `consent.rs`, `daemon.rs` and `flow.rs`) use the signed-file fixtures that exist (`World::install_file`, `flow.rs`'s `file` and `bundle_file`, `image::fixture`). Each refused directory gets a test of its own.

## Why

For the cleanup: deleting the file is how a person uninstalls an application (ADR 0009). Leaving its data, its previous file and its daemon behind is surprising, and those files are never read again. Moving the data to the Trash makes the cleanup something a person can undo, and keeping the record keeps the high-water mark that protects them.

For the directory:

A Bundle that runs with no signature is a second trust model beside ADR 0060's, and it verifies nothing. As long as `run <dir>` works, ADR 0060's statement that an unverifiable Bundle fails closed is false. Development does not need the directory: `odk oma dev` signs with the dev key (ADR 0085). Keeping `assemble` private costs nothing, because signing needs a tree to build the image from.

An installed directory is left in place, and not converted or deleted, for two reasons. Nothing can sign it on the user's behalf, since the publisher's key is not on that machine. And deleting it unasked would delete the application's data. The two machines that have one get a line telling them how to remove it.

## Considered

- **Convert an installed directory to a file automatically.** Rejected: the result would need a signature, and a dev-key signature on someone else's application would make this computer its publisher.
- **Delete installed directories on upgrade.** Rejected: the data would be gone with no question asked.
- **Delete a deleted file's data outright.** Rejected: a file leaves `~/Applications` by accident too (moved to another folder, dragged over by a re-download, tidied away), and the deletion could not be undone. The Trash is where a person already looks for something they deleted.
- **Keep the data, and clean only what is derived from the file.** Rejected: data that nothing will ever read again is what the owner asked to have cleaned up.
- **Remove the record too.** Rejected: it would drop the high-water mark, so an older file with a known flaw could then be installed with no refusal.
- **Keep `run <dir>` for development only, behind a flag.** Rejected: `dev` already covers it with a signature, and a flag that turns verification off is the hole this record closes.

## Consequences

- ADR 0009 is amended: removing an application by deleting its file now also removes its state, its data going to the Trash. ADR 0060, section 7 (what `--register` does) is amended the same way. Its statement that uninstalling keeps the high-water mark still holds.
- `docs/spec/oma-cli.md` describes the cleanup under `--register`, and `crates/oma/tests` gets a test for each step, on a throwaway `$XDG_DATA_HOME` and a fake `systemctl`, including a move to the Trash that fails and leaves the data where it was.
- ADR 0032 is amended: `bundle` writes only a signed file, and its directory is a staging step. Its status line becomes "amended by ADR 0050, ADR 0052 and ADR 0099". ADR 0022 stays superseded by ADR 0052. The "still installs a directory" lines in ADR 0052, ADR 0060 and ADR 0085 are context and are left as written.
- `docs/spec/oma-cli.md`, `docs/spec/manifest.md`, `docs/spec/sandbox-baseline.md` (the data path), `docs/oma-trust-plan.md`, `GLOSSARY.md`, `README.md` and `CLAUDE.md` lose their "the code still installs a directory" wording when the code lands.
- The data directory has been named by its directory-era path, `~/.local/share/oma/data/<identifier>`, in `docs/spec/sandbox-baseline.md`, `docs/spec/plugin-manifest.md` (where Plugins are loaded from), `docs/spec/plugin-api.md`, the doc comment of `odk-plugin`'s `Host::new` and `examples/notes`. A Bundle file's data directory is the pair's, `~/.local/share/oma/apps/<root key id>/<identifier>/data`, and inside the sandbox it is `$HOME` for both kinds. Those documents name it that way. `examples/notes`'s `data_dir()` uses `$HOME` when it runs under `odk oma run`, so its Plugins are found where ADR 0098's criterion 4 needs them. `odk-plugin` takes the directory as an argument and needs no change in code.
- `docs/checks.md`'s row about moving a Bundle directory to the previous-version store is dropped as no longer applicable.
