# `oma` is never the parent of an application, not even under `--wait`

An application's process tree is entirely separate from `oma`'s. `oma run` starts it through an intermediate process that creates the namespaces, tells `oma` the init's pid over a pipe and exits at once, so the init is an orphan of the host from the first moment and leads its own session. `oma` is never its parent or an ancestor of it, whether or not it stays.

This amends ADR 0024, which let `oma run --wait` stay as "a thin parent". Under `--wait`, `oma` stays, relays signals by sending them to the init's pid (a process of the same user may signal it), and learns the outcome from a second pipe: the init writes the application's exit status to it just before it exits, and the status is `128` plus the signal number if the application was killed. `oma` returns that status, or `128 + 9` if the pipe closes with nothing in it because the init was killed outright. Without `--wait`, `oma` returns as soon as the application has started. A setup failure still reaches `oma` over the pipe that closes when the application starts, as in ADR 0024.

What this does not do: the application stays in whatever control group started `oma`, such as a terminal's or the launcher's. Moving it into its own transient `systemd --user` scope is a separate step and is not done.
