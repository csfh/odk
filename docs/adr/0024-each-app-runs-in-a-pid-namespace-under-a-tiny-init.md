---
status: amended by ADR 0034
---

# Each application runs in its own PID namespace under a tiny init

The application runs in its own PID namespace, so it sees only its own processes in a fresh `/proc` and cannot read or signal the user's other programs. This reverses ADR 0017's plain `exec`: a PID 1 without signal handlers ignores SIGTERM from outside and does not reap orphans, so an application run as PID 1 directly could not be stopped with an ordinary `kill`. Without a PID namespace the alternative was leaving other processes' command lines and environments readable, with no confirmed way to prevent it.

A process that creates a PID namespace never enters it; only its child does. So omalaunch builds the sandbox and forks, the child becomes PID 1 of the namespace by re-executing omalaunch in an init mode, and the init runs the application as its child. The init is the same binary re-executed, not a second program. By default the outer omalaunch then exits and the application lives under `systemd --user` like any other desktop application, so omalaunch is not the parent of the applications it launches. From the host the application is an ordinary process with an ordinary PID; the namespace limits what it can see, not who its parent is. `omalaunch --wait <app>` is the opt-in alternative for terminals and scripts: the outer process stays as a thin parent, relays terminal signals such as Ctrl-C and Ctrl-Z, and exits with the application's status.

The init does exactly these things:

1. Reaps orphaned processes.
2. Forwards signals to the application: every catchable one.
3. Exits with the application's exit code, or 128 plus the signal number if it was killed, which is reported to whoever waits on it: `systemd --user`, or omalaunch under `--wait`.
4. Exits when the application's main process exits, so the kernel tears down anything left in the namespace. The Bundle's executable must therefore be the real long-running process, not a launcher that starts the application in the background and exits.
5. Mounts the fresh `/proc` and finishes the sandbox, which can only be done from inside the new PID namespace: it applies the last Landlock and seccomp restrictions and drops capabilities for the application, then drops its own.

It never restarts an application, imposes resource limits, checks for updates, opens a control socket, or decides anything about permissions or consent; that is policy and stays in omalaunch before the init starts (ADR 0010). It does not write to the journal: applications launched through `systemd --user` already have stdout and stderr connected to it, and the init passes those through to the application.

## Consequences

Each running application has one extra small process, named `omalaunch --init`. An unprivileged process can mount a fresh `/proc` in its own user, mount and PID namespaces: this was tested on the development machine (Linux 7.2.8, `unshare --user --map-root-user --mount --pid --fork --mount-proc`), where the process saw only its own four processes. A kernel rule requires an existing fully visible procfs mount before a user namespace may mount a new one, so the init must mount `/proc` before it covers or masks anything under it. This has not been tested on other kernels or inside containers. Re-executing omalaunch as the init should use a file descriptor opened beforehand (`fexecve`), so it does not depend on omalaunch's install path being visible inside the sandbox; no ADR says where omalaunch is installed.

The application keeps the user's own uid inside the sandbox; it is not mapped to root. A process with a non-zero uid loses all capabilities across `execve`, so the re-executed init would have none and could not mount `/proc` or finish the sandbox. Before the re-exec, omalaunch therefore makes its capabilities inheritable and raises them in the ambient set, which survives the exec. This was tested (`spikes/fexecve-init`, `docs/checks.md`). We rejected mapping the user to uid 0 inside, because Chromium and Electron refuse to run as root.
