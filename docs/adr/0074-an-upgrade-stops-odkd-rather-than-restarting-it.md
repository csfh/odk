---
status: accepted
---

# An upgrade stops `odkd` rather than restarting it

ADR 0063's script ends an install with `systemctl --user enable --now odkd.socket` and `try-restart odkd.service`, so that a new install starts `odkd` and an upgrade runs the new program. ADR 0068 gave the same service a second socket, `odkd-files.socket`. On 2026-10-28 the first upgrade from a release with one socket to one with two failed on the development machine. `enable --now` enabled the new socket and could not start it ("odkd-files.socket: Socket service odkd.service already active, refusing"). The script stopped there, and the old `odkd` went on running without the file chooser's socket. systemd does not start a socket unit while the service it activates is running, because it cannot hand the new descriptor to a process that is already running. A fresh install never meets this, because nothing is running yet, and that is all the installer's test covered.

**Decision.** After `daemon-reload` the script runs `systemctl --user stop odkd.service`, then `enable --now` on the sockets. There is no `try-restart`. The sockets listen across the stop, so nothing is refused. The next connection starts the new program with every socket. `odkd.service` wants `odkd-files.socket` and is ordered after it, so a connection that comes between the two commands also starts it with both sockets. An application that was connected loses its connection and reconnects, as it did on a restart, and that reconnection starts the new `odkd`. A running sandbox is bound the `odk` runtime directory, not each socket file, so it sees the new `files.sock` too.

**What it costs.** After an upgrade no `odkd` runs until something connects, where `try-restart` started one at once if one had been running. Nothing is lost that a restart kept, since both end the old process. `systemctl` prints that the stopped service's socket is still active. That warning is the intended state, and the script leaves it in.
