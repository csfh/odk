---
status: superseded by ADR 0048
---

# The user's `shell.toml` asks for reduced motion

Superseded by ADR 0048. The `[motion] reduced` key in the user's own `~/.config/omarchy/shell.toml` is not a source of reduced motion, including when Hyprland gives no answer. What remains of this decision is oma's mirror: `oma omarchy sync` writes Hyprland's answer into the sandbox's copy of `shell.toml`, and a sandboxed application reads that copy (ADR 0044). The text below is the decision as it was made, before ADR 0048 replaced it.

ODK had a reduced-motion switch for the application and a stub for the system's, which was `false`. The brief for Phase 10 asked for the system's preference, so the installed environment was probed on 2026-10-23: the desktop portal's `org.freedesktop.appearance` answers only `contrast` and `color-scheme`, Omarchy has no such setting, and GNOME's `org.gnome.desktop.interface enable-animations` exists (it reads `true` here) but nothing in Omarchy sets or documents it. The user's own `~/.config/omarchy/shell.toml` is already how ODK learns the text size (ADR 0043), is already watched live, and is already copied into the sandbox (ADR 0044). So `[motion] reduced = true` there asks for reduced motion: `ThemeSnapshot::reduced_motion` carries it, the foundation is built again when it changes, and every window redraws with the new tokens, open overlays included. Only the user's file counts, never a theme's `shell.toml`, since a theme is not the user's accessibility setting; anything but a boolean says nothing. The application's own choice (`FoundationConfig::reduced_motion`, `odk::ui::set_reduced_motion`) is unchanged and both add: either one reduces motion, and neither turns the other off. `oma omarchy sync` copies `[motion] reduced = true` into the sandbox's copy beside `[font] base-size`.

The key is ODK's own convention in Omarchy's file, since Omarchy defines none. If Omarchy gains a setting, it replaces this reader. This widens ADR 0035 (only colours, the text size and the font come from Omarchy), ADR 0043 (the user's `shell.toml` is read for the text size) and ADR 0044 (the sandbox's copy holds `[font] base-size` and nothing else). It stays `proposed` until the user confirms the key and the widening; the code follows it meanwhile.

## Considered Options

- **GNOME's `enable-animations` through `gsettings`.** The only existing key, but a subprocess at startup, no live change (the theme watcher does not see dconf), and unreachable from the sandbox without the session bus and dconf, so the setting would silently read as `false` exactly where applications run. Making `oma` copy it in needs a resident watcher or a poll.
- **The freedesktop portal.** Has no reduced-motion key here.
- **An environment variable.** Not live, and a second source beside `shell.toml`.

## Consequences

Turning it on or off in `shell.toml` reaches running applications through the watcher that follows the text size. A user who set `enable-animations = false` for GTK gets no reduced motion in ODK until they also set the key. The gallery's diagnostics say whether the user's file asked.
