# Sandbox baseline specification, version 1

Normative. This spec fills in the baseline of ADR 0012 for the sandbox of ADR 0005. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

Access is read-only unless a row says otherwise, and private tmpfs mounts are writable. This applies to every sandboxed application. Things this spec lists are visible; everything else is not.

## Baseline (always)

| What | Access |
|---|---|
| The application's own data directory, `~/.local/share/oma/data/<identifier>/`, appearing at the real home directory's path, so `$HOME` is that path (see below) | read-write |
| `~/.local/share/oma/inbox/<identifier>/`, the application's own inbox (ADR 0015), at `/inbox` with `$OMA_INBOX=/inbox` | read-write, only with `updates` |
| The application's own Bundle contents, at `/app` | read-only |
| `/usr` | read-only |
| `/bin`, `/lib`, `/lib64`, `/sbin` | as symlinks into `/usr` |
| `/` | a private tmpfs holding only the mount points listed here; read-only |
| `/proc` | a fresh procfs of the application's own PID namespace, showing only its own processes (ADR 0024) |
| `/dev` | a minimal private set: `null`, `zero`, `full`, `random`, `urandom`, `tty`, and a private `/dev/shm` tmpfs not shared with other applications, and the usual `fd`, `stdin`, `stdout` and `stderr` links; `/dev/dri` only with `gpu` |
| `/tmp` | a private tmpfs, per application and per launch |
| `/run` and `$XDG_RUNTIME_DIR` | a private tmpfs holding only what the application's permissions bring: the Wayland socket always, the PipeWire socket with `audio`, and `/run/systemd/resolve/stub-resolv.conf` with `network`. The session bus socket is not provided: see [Session D-Bus](#session-d-bus) |
| `/var` | not visible |
| `/sys` | not visible, except with `gpu` the slice described below |
| Omarchy's current theme (ADR 0019) | read-only |
| `~/.config/omarchy/` and `~/.config/fontconfig/`: oma's copy of the user's text size, reduced-motion request and font (ADR 0044, ADR 0048) | read-only |
| The `/etc` entries below | read-only |

Under Run once (ADR 0018) the data directory is a private tmpfs that disappears when the application exits, not the path above.

`/bin`, `/lib`, `/lib64` and `/sbin` are symlinks into `/usr` on a merged-`/usr` system such as Omarchy. Every x86_64 ELF names `/lib64/ld-linux-x86-64.so.2` as its interpreter, so without `/lib64` no application starts. oma exposes these as the same symlinks, which resolve inside `/usr`.

The theme is the directory `~/.local/state/omarchy/current/theme/` (`colors.toml`, `shell.toml` and files for other programs), which Omarchy's shell reads, with its name in `~/.local/state/omarchy/current/theme.name`. Omarchy replaces that directory on every theme change (it deletes `theme/` and moves a new one into place), so a mount or Landlock rule on `theme/` itself would go stale. oma exposes its parent, `~/.local/state/omarchy/current/`, read-only instead; that also shows the application the theme name and the current background link, which are harmless. How a running application learns of the change is for the ODK UI layer (ADR 0019).

The user's text size and font (ADR 0043, ADR 0044) are not the real files, which the application never sees: `omarchy display text size` replaces `~/.config/omarchy/shell.toml` by renaming over it, so a mount of that file would go stale, and `~/.config/omarchy/` holds more than an application should read. `oma run` copies what is needed into `~/.local/share/oma/omarchy/` (`oma::omarchy::refresh`), and the sandbox mounts its `omarchy/` at `~/.config/omarchy` and its `fontconfig/` at `~/.config/fontconfig`, each read-only and only if it exists. `omarchy/shell.toml` holds `[font] base-size` and, when Hyprland's animations are off, `[motion] reduced = true` (ADR 0048), and nothing else. `oma` asks Hyprland itself and does not copy a `[motion] reduced` key from the user's own file; when Hyprland gives no answer the key is left out. `fontconfig/fonts.conf` is the user's own, when it is a regular file of at most 64 KiB. Copies are replaced by renaming inside those directories, which stay, so a mount and a watch on them follow. The copy is made at every launch and never stops one. Nothing of `$XDG_RUNTIME_DIR/hypr` is ever mounted or granted: its socket accepts commands, so an application that could open it would not be sandboxed (a test in `sandbox/plan.rs` holds this). A running application sees the compositor's setting as of the launch or the last `oma omarchy sync`.

### `/etc` baseline

`ld.so.cache`, `ld.so.conf`, `ld.so.conf.d/`, `fonts/`, `localtime`, `os-release`, `passwd`, `group`, `nsswitch.conf` and `xdg/`.

## Where things appear inside

`$HOME` inside is the same path as the real home directory (for example `/home/hallas`), but what is there is the application's own data directory, not the real home. Programs that compute `~/.config`, `~/.local/share` or `~/.local/state` from `$HOME` therefore get the usual paths, and what they write persists in the data directory. The named directories the application was granted appear under it at their usual names (`~/Documents`), and the theme at `~/.local/state/omarchy/current`, the text size and font at `~/.config/omarchy` and `~/.config/fontconfig`; mount points for them are created inside the data directory. The real home is not otherwise visible. The Bundle is at `/app`, and the working directory is `$HOME`.

The environment is cleared and rebuilt: `HOME`, `PATH` (`/usr/local/bin:/usr/bin`), `XDG_RUNTIME_DIR` (`/run/user/<uid>`), `WAYLAND_DISPLAY`, `XDG_SESSION_TYPE=wayland`, `OMA_SANDBOX=1` (so the application knows `~/.config/omarchy/shell.toml` is oma's copy and may read `[motion] reduced` there, ADR 0048), `OMA_INBOX` with `updates`, and copies of `LANG`, `LANGUAGE`, `LC_ALL`, `LC_CTYPE`, `LC_MESSAGES`, `TERM`, `USER`, `LOGNAME` and `TZ` when set. No `XDG_*` directory variable is set, so the defaults under `$HOME` apply.

The application keeps the user's own uid and gid inside the sandbox.

## `/sys` with `gpu`

Graphics drivers read `/sys` to identify a GPU, and without it Vulkan fails to initialise (`docs/checks.md`). The whole of `/sys` would show hardware serials and network addresses, so with `gpu` oma shows only, for each node in `/dev/dri`, the `/sys/dev/char/<major>:<minor>` link and the PCI device directory it leads to, read-only. This was enough for GPUI on the development machine, which has an integrated and an external GPU.

## Session D-Bus

The baseline of ADR 0012 gives the session bus socket to an application with a non-empty `dbus` list. A raw socket reaches every service on the bus, which makes the list of named services meaningless, and no filtering proxy is designed. Until one is, `oma run` refuses a Bundle that requests `dbus` and says so; the permission stays in the Manifest and the consent record, and nothing else in the sandbox depends on it.

## `/etc` entries tied to permissions

An entry is visible only while the Manifest declares the permission that justifies it (see `manifest.md`).

| Permission | Entries |
|---|---|
| `network = true` | `hosts`, `resolv.conf` and the file it links to, `ssl/`, `ca-certificates/` |
| `audio = true` | `pipewire/`, `alsa/` |
| `gpu = true` | `vulkan/` and `drirc.d/`, where they exist |
| `dbus` with at least one service | `machine-id` |

Nothing else under `/etc` is visible. In particular `machine-id` is hidden from an application that does not use D-Bus, because it is a stable host identifier.

## Symlinks

oma follows a symlink inside `/etc` and exposes it only if its target lies inside something already exposed. A link whose target lies outside is exposed together with that one target and nothing else. On Omarchy `resolv.conf` links to `/run/systemd/resolve/stub-resolv.conf`, so exactly that file is made visible with the `network` permission, and `/etc/localtime` needs nothing extra because it links into `/usr`.

## Network

Without `network = true` the application gets a fresh network namespace containing only loopback. With it, the application shares the host's network namespace, which is what lets the systemd stub resolver at `127.0.0.53` answer. This also exposes any service listening on the host's `127.0.0.1` to the application, and the consent dialog says so (ADR 0012).

## Abstract sockets and IPC

Every application gets a private IPC namespace, so SysV shared memory and message queues are not shared. An application with `network = true` shares the host's network namespace, which also holds abstract unix sockets such as D-Bus's abstract address, so oma scopes abstract unix sockets with Landlock (ABI 6) for such applications and refuses to launch them if the kernel does not support it (ADR 0005). Applications without `network` have a fresh network namespace and need no scoping.

## Not yet verified

- Whether the fresh `/proc` mount also works on kernels other than the one tested and inside containers (ADR 0024).
- Whether common applications other than the GPUI test application run with the minimal `/dev` and `/run` above, for example ones that need a pseudo-terminal (`/dev/pts`) or shared memory beyond their own `/dev/shm`.
- Whether the entries tied to `audio` and `network` are complete. `gpu` was checked with GPUI.
