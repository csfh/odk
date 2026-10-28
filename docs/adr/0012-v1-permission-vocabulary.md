---
status: amended by ADR 0050 and ADR 0068
---

# v1 permissions: a fixed baseline plus six declared capabilities, deny by default

Every sandboxed application gets a fixed baseline: its own data directory, its own Bundle contents read-only, the host's `/usr` and the needed parts of `/etc` read-only (a dynamically linked ELF cannot run without the host's glibc and loader), the Wayland display socket, and Omarchy's current theme read-only (ADR 0019). The exact `/etc` subset is in `docs/spec/sandbox-baseline.md`. Everything else must be declared in the Manifest, and only these capabilities exist in v1; anything else is added later through a new ADR.

- **Filesystem:** named user directories only, each read or write: `documents`, `downloads`, `pictures`, `videos`, `music` and `desktop`. There are no arbitrary paths in v1.
- **Network:** on or off, with no per-host rules. Off gives the application a fresh network namespace with only loopback; on shares the host's network namespace, so the application can reach everything the host can, including services listening on `127.0.0.1`. The consent dialog says so. We rejected a separate namespace behind a userspace proxy (`pasta`, `slirp4netns`), which would hide localhost services but adds a dependency and a lot of complexity.
- **GPU:** access to DRI devices.
- **Audio:** playing and recording sound through PipeWire.
- **Session D-Bus:** off by default, with a list of named services the application may talk to.
- **Updates:** write access to the application's own inbox only, for staging a new version of itself (ADR 0015). It does not apply to Run once (ADR 0018).

omalaunch resolves each named directory from the user's XDG user-directory settings (`user-dirs.dirs`) every time it launches the application, and never falls back to a conventional location: a name with no entry is unavailable, and the application launches without it, with the consent dialog noting that the user has no such directory. After following symlinks, a named directory must lie strictly inside the home directory, and its first component under home must be neither `Applications` nor a hidden name (one starting with `.`); otherwise that directory is unavailable, exactly like a missing entry: the application still launches without it and the consent dialog says so. That rejects the home directory itself and any ancestor of it such as `/` or `/home`, so no grant can reach `~/.config`, shell startup files, omalaunch's own state or any other place that runs code outside the sandbox.

We chose a fixed set of named directories over arbitrary path grants because every path-based design failed the same way. A deny list kept growing (Omarchy alone runs Hyprland's Lua config, uwsm files, `~/.local/bin`, `~/.bashrc` and hooks outside any sandbox) and would drift as Omarchy changes. Masking denied paths with empty bind mounts depends on seccomp blocking `mount` and cannot mask a path that does not exist yet. The cost is that an application needing wide access, such as a general file explorer, cannot get whole-home access in v1; a later ADR can add further named grants.

Audio includes recording. A PipeWire client with access to the socket can capture as well as play by default, and no documented way to restrict a client to playback without a portal was found, so a playback-only permission is not offered in v1: it would promise a limit nobody has shown the sandbox can enforce. A separate Microphone permission can be added later through a new ADR once that enforcement is verified.

Cameras, raw sound devices (`/dev/snd`), USB, per-host network rules and portals are deliberately out of v1. This keeps the permission prompt short enough to read and the sandbox in ADR 0005 small enough to audit.
