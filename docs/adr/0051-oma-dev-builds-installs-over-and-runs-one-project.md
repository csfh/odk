---
status: amended by ADR 0073
---

# `oma dev` builds, installs over and runs one project

The user asked on 2026-10-25 for `oma dev <target>`: one command that takes a project from source to a running window, in place of building, bundling, installing and running by hand, and which has to "work" from a single target. This record is the choices made on their behalf, and is open to change.

**The command.** `oma dev [<project-dir>] [--wait]` (the directory defaults to the current one). It reads `dev.toml` beside the Manifest, runs the project's build commands, assembles the Bundle as `oma bundle` does, asks for what the consent record lacks, stops the daemon of the copy it is about to replace, installs over that copy, and runs the installed copy as `oma run <Identifier>` would. `--wait` is `oma run`'s.

**The project file.** `oma` compiles nothing (ADR 0010, ADR 0032), so how to build is the project's to say, in a file of its own and not in the Manifest: the Manifest is copied into the Bundle and refuses any key it does not know, and a build command has no place in what is distributed. `dev.toml` is never put in the Bundle. Its keys are `build` (a list of commands, each a list of strings, run in order in the project's directory and stopping at the first that fails), `executable` (the built program), `manifest` (default `manifest.toml`) and `include` (as `oma bundle --include`). Every path in it is taken from the project's directory, never from where `oma` was started, and may leave it (`../backend/...`): the file is the project's own and the user pointed `oma` at it. An unknown key, an empty command and an include the Bundle would refuse are errors. A directory without `dev.toml` is an error that names the file; nothing is guessed.

**What the build commands are.** Programs the project names, run as the user with the user's environment and no sandbox, exactly as `make` or `cargo build` would be. Running `oma dev` on a directory is trusting its `dev.toml` as much as running its build. The user said "work" and does not run untrusted projects this way; a project from elsewhere should be read first.

**It replaces the installed copy.** `oma install` refuses an Identifier that is installed (ADR 0009, ADR 0015), and still does. `oma dev` is the one exception, for the copy a developer is iterating on: `install::reinstall` moves the old copy aside under a name that listing skips, installs the new one, deletes the old on success and puts it back on failure. The consent record is the Identifier's, so a copy that asks for nothing new asks nothing; one that asks for more shows the same dialog `oma install` would (Install or Cancel), and a Cancel changes nothing.

**Order.** Load `dev.toml`, build, assemble in a directory of its own under `~/.local/share/oma/dev/`, ask, stop the daemon, install, run. A build that fails or a Cancel therefore changes nothing and stops nothing, as `Cancelled` already says. The assembled directory is removed whichever way the command ends.

**The daemon.** A Bundle with a daemon runs it from a transient unit that `oma run` starts only when nothing accepts connections on its socket (ADR 0050), so a daemon left from the previous build would keep serving the old program. `oma dev` stops the unit it would start (`systemctl --user stop oma-<Identifier>.socket` and `.service`, the same name `oma run` gives it; not loaded is not an error) before it installs. If something else still accepts connections on the socket afterwards, such as the application's own installed user unit, `oma` says so on standard error and goes on: it does not stop a unit it did not start, and the application will then talk to that daemon.

**Rejected.** A `[dev]` table in the Manifest, which ships the build with the Bundle and loosens the rule that the Manifest holds no key it does not use; a separate identifier or install location for dev copies, which would leave a second copy of the application's data and consent record to explain; and having `oma` guess the build from the project (`Cargo.toml`, `package.json`), which is the build system `oma` does not have.

This amends ADR 0009's and ADR 0015's "an installed Identifier is not replaced" for `oma dev` alone; it does not change ADRs 0010, 0023, 0032, 0034 or 0050.
