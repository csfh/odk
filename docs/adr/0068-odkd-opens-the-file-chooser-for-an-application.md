---
status: accepted
---

# `odkd` opens the file chooser for an application

An application needs the user to pick a file or a folder: Atmos picks an avatar, a wallpaper, a boot logo, a hook, a WireGuard profile, a file to share, and where to save an export or a diagnostic report, twelve dialogs on eight pages. The desktop's file chooser is the freedesktop portal (`org.freedesktop.portal.FileChooser`), which Omarchy answers with Flea's backend and GTK's after it. A sandboxed application cannot ask it: the sandbox has no session bus, a `dbus` permission is refused until a filtering proxy exists (`docs/spec/sandbox-baseline.md`), and ADR 0012 leaves portals out of v1. GPUI's own chooser calls the portal from the application's process, so it works only outside a sandbox. On 2026-10-28 the user said that `odkd` should do this.

## Decision

**`odkd` asks the portal on the application's behalf.** It already runs as the user, outside every sandbox, and its directory `$XDG_RUNTIME_DIR/odk/` is bound into every sandbox (ADR 0062). It listens on a second socket there, `files.sock`, beside `odkd.sock`. An application connects, writes one request as one line of JSON, and reads one answer as one line: the paths the user chose, that the user cancelled, or why there is no chooser. `odkd` calls the portal's `OpenFile` (a file, several files, or a folder) or `SaveFile` on the session bus as itself, so the portal treats it as a host application and answers with the real paths, not the document portal's.

**The application gains a name, not access.** The answer is a path. The sandbox's rules are fixed at launch (ADR 0005, ADR 0012), so a path outside the application's granted directories stays unreadable to it. What a Bundle does with the path is its own: Atmos hands it to its daemon (ADR 0050), which runs outside the sandbox and reads or writes the file there. Choosing in the dialog is the user's consent to that one name, at that moment, so there is no permission to declare and no consent row: the application cannot see anything of the filesystem until the user picks, and only what the user picked.

**What a request may say.** The kind (`open`, `save`, `folder`), a title, the label of the accept button, whether several may be chosen, name filters (a label and glob patterns), a suggested file name for `save`, and where to start: one of the named folders `home`, `desktop`, `documents`, `downloads`, `music`, `pictures` and `videos`, which `odkd` looks up outside the sandbox in `user-dirs.dirs` (its home for `home`, and its home when a named one is missing), or an absolute path the application was given, such as one its daemon read. Inside the sandbox `$HOME` is the application's data directory, so it cannot name the user's folders itself.

**Bounds.** A request is one line of at most 16 KiB, and must arrive within five seconds of connecting; a title, a label or a name is at most 256 characters; at most 16 filters of at most 32 patterns. One connection is one request. At most four dialogs are open at once for all applications together; a fifth request is answered at once that the chooser is busy. Anything else is answered with an error and the connection closed. The dialog itself has no time limit, since the user may take as long as they like.

**What stays as it was.** `odkd.sock` still reads nothing from a client: the chooser is another socket, so ADR 0062's guarantee that a client of the settings socket can only hear what every application hears holds as written. The application still has no bus, and nothing in the sandbox changes: `files.sock` is in the directory already bound read-only, and connecting to a socket needs no write to that directory.

**Starting it.** A second socket unit, `odkd-files.socket`, listens on `%t/odk/files.sock` and starts the same `odkd.service`. `odkd` tells the two apart by the names systemd passes (`LISTEN_FDNAMES`), and binds the file chooser's socket itself when it was started without it (by hand, or by an older `odkd.socket`).

**No parent window.** The dialog is not attached to the application's window in v1: the portal would need a Wayland handle exported from inside the application, which ODK cannot reach through GPUI 0.2.2. The compositor places the dialog as a window of its own.

This amends ADR 0062 (a second socket and the requests on it) and ADR 0012 (the application still asks no portal; `odkd` does, outside the sandbox).

## Considered Options

- **GPUI's chooser in the application.** It works outside a sandbox only, has no name filters in 0.2.2, and the applications that most need a chooser, Bundles like Atmos, are sandboxed.
- **Bind the session bus, or a filtering proxy.** The bus reaches every service; a proxy such as `xdg-dbus-proxy` is not in a stock Omarchy and is a component of its own to audit.
- **The Bundle's own daemon asks the portal.** Every Bundle would carry a D-Bus client and a portal flow; a Bundle without a daemon could not pick a file at all.
- **The document portal.** It would give the application read access to the chosen file through a FUSE mount, which the sandbox would have to bind and the application to understand. Nothing needs that yet; a path handed to a daemon covers Atmos.
- **Reuse `odkd.sock`.** It would end the rule that `odkd` never reads from a client, which is what lets every sandbox have it with no permission.

## Consequences

- A hostile application can open file choosers. It learns only what the user then picks, and at most four dialogs are open at a time.
- A dialog stays open if the application that asked goes away; its answer is written to a closed socket and dropped.
- `odkd` links a D-Bus client (`ashpd` 0.11 with its `zbus`, the versions GPUI already brings).
- Tests never reach the user's portal, which would open a window on their desktop: `odkd`'s test runs a private `dbus-daemon` with no service directories and a fake portal on it.
- `packaging/`, `install.sh` and the installer's test gain the second unit. `docs/spec/sandbox-baseline.md` says what the directory now holds.
