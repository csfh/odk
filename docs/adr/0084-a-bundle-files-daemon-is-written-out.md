---
status: accepted
---

# A Bundle file's daemon is written out

A Manifest's `[daemon]` is a program of the Bundle's that runs outside the sandbox, as a socket-activated user unit (ADR 0050). A directory Bundle has that program on disk, and the unit runs it from there. A Bundle file does not. ADR 0067 unpacks its image only into the sandbox's tmpfs, and nothing reaches the host. So a Bundle file with a daemon could not run, and Atmos, whose `atmosd` is one, could not be a Bundle file. On 2026-10-29 the owner chose to write the daemon out.

## Decision

- **What is written.** When an installed Bundle file is launched and declares a daemon, `odk-oma` writes that daemon's program, the Manifest's `daemon.executable`, from the verified image. It goes to `apps/<root key id>/<app id>/daemon/<file name>` in the state directory, mode `0755`, in a directory only the user can enter (`0700`). The unit is started from there. It is the one file of a Bundle's image that is written to the host.
- **When.** At the launch, before the daemon is started and before the sandbox is planned. That is the one moment `odk-oma` has just verified the file it runs. Installing, upgrading, `rollback` and `--register` therefore need no step of their own. The file is replaced in one step, and only when its bytes differ. A daemon already running keeps the program it started from.
- **Run once.** It still does not run a daemon, since the daemon would outlive the launch (ADR 0050).
- **Purge.** `odk oma purge` removes it with the rest of the pair's directory.

## Consequences

- ADR 0067's "nothing reaches a host filesystem" now has this one exception: a daemon's program, which ADR 0050 already runs outside the sandbox.
- A daemon gets only its own program. Any other file of the Bundle stays inside the image. A daemon that needs more is not supported.
