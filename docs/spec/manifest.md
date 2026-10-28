# Manifest specification, version 1

Normative. This spec defines the Manifest described in ADR 0006. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

## Location and format

A Bundle (ADR 0052) contains one Manifest at `manifest.toml` on the root of the image a mount shows. The file on disk is one `<App>.oma`; the tree inside it is the tree this spec describes. The code still validates a Bundle directory until it follows ADR 0052. The Manifest is declarative; oma reads it without executing anything from the Bundle. The file name is cosmetic, and identity comes only from `identifier`.

Any key not listed here is an error. No text in the Manifest that oma later writes into a `.desktop` entry (`name`, `description`, `categories`, `icon`) may contain a control character, so a newline cannot add a line such as `Exec=` to it (ADR 0014); the same holds for `executable` and for D-Bus service names. Any invalid Manifest makes oma reject the Bundle: it is not run, installed or upgraded.

## Top-level keys

| Key | Type | Required | Rule |
|---|---|---|---|
| `manifest_version` | integer | yes | Must be `1`. |
| `identifier` | string | yes | See [Identifier](#identifier). Permanent. |
| `name` | string | yes | Display name. Non-empty UTF-8 with no control characters. |
| `version` | string | yes | Semver 2.0.0. See [Version](#version). |
| `executable` | string | yes | See [Executable](#executable). |
| `permissions` | table | yes | See [Permissions](#permissions). May be empty, meaning everything beyond the baseline is denied. |
| `daemon` | table | no | A program of the Bundle's that runs outside the sandbox (ADR 0050). See [Daemon](#daemon). |
| `description` | string | no | One-line summary, with no control characters. |
| `icon` | string | no | Path inside the Bundle to a PNG or SVG file, under the same containment rules as `executable`. |
| `categories` | array of strings | no | freedesktop registered category names, used for the `.desktop` entry (ADR 0014). Each is non-empty and contains only letters, digits and `-`. |
| `publisher` | string | yes | The publisher host in the signed manifest (ADR 0060). A hostname: lowercase letters, digits, hyphens and dots, no scheme, no userinfo, no port, no path, and not an IP address. At most 253 characters. An internationalized name is the punycode form. It must equal the signed manifest's `publisher`. It is display text and the first-contact host of `https://<publisher>/.well-known/oma-keys`. It is not the root of trust. The code still accepts a Bundle with no `publisher` until it follows ADR 0060. |

## Identifier

Two or more segments separated by `.`. Each segment starts with a lowercase letter `a`–`z` and contains only lowercase letters, digits and `-`. The whole string is at most 128 characters. Example: `com.example.foo`. Underscores are not allowed. oma escapes the Identifier with `systemd-escape` when it builds unit names (ADR 0015).

Two Bundles with the same Identifier and the same pinned publisher root are the same application at different versions (ADR 0060). The same Identifier under a different root is a different publisher's application and does not share data or permissions.

## Version

Semver 2.0.0 precedence applies to this string: a pre-release such as `1.2.0-beta.1` sorts below `1.2.0`, and build metadata is ignored when comparing. It is the version people read, and the payload digest covers it (ADR 0060). The upgrade check is the signed monotonic integer in the prefix manifest. A file whose integer is lower than the stored high-water mark is refused, including when this string looks newer. The same integer is accepted only when the payload digest matches the one already stored. A higher integer is an upgrade when ADR 0060 accepts the file. The only way to run an older installed copy is `odk oma rollback` (ADR 0011), which does not lower the mark.

## Executable

`executable` is a path relative to the Bundle root. It must not be absolute, must not contain `..`, and must not resolve (through a symlink) outside the Bundle directory. At validation the target must be a regular file with the executable bit set and must parse as an x86_64 ELF (ADR 0002). The Manifest carries no arguments and no environment variables in v1. The executable must be the application's real long-running process: when it exits, oma's init ends everything in the application's PID namespace (ADR 0024), so a launcher script that starts the real program in the background and exits is killed immediately.

## Permissions

The `[permissions]` table has these keys. Anything absent is denied, and any other key is an error. These are the declared capabilities of ADR 0012; the baseline is always granted and is not written here.

| Key | Type | Meaning |
|---|---|---|
| `network` | boolean | Network access, with no per-host rules. `true` shares the host's network namespace, including services on `127.0.0.1` (ADR 0012). Default `false`. |
| `gpu` | boolean | Access to DRI devices. Default `false`. |
| `audio` | boolean | Playing and recording sound through PipeWire. Default `false`. |
| `updates` | boolean | Write access to the application's own inbox (ADR 0015). Default `false`. Has no effect under Run once (ADR 0018). |
| `filesystem` | array of tables | See below. |
| `dbus` | table | `talk`: array of session-bus service names the application may talk to. Default none. |

### Filesystem entries

Each `[[permissions.filesystem]]` block adds one entry:

| Key | Type | Required | Rule |
|---|---|---|---|
| `dir` | string | yes | One of `documents`, `downloads`, `pictures`, `videos`, `music`, `desktop`. |
| `access` | string | yes | `"read"` or `"write"`. Write includes read. |

Each `dir` may appear at most once. There are no arbitrary paths in v1. oma resolves each named directory from the user's XDG user-directory settings (`user-dirs.dirs`) every time it launches the application. There is no fallback to a conventional location: a name with no entry is unavailable, and the application launches without it, with the consent dialog noting that the user has no such directory. After following symlinks, a directory must lie strictly inside the home directory, and its first component under home must be neither `Applications` nor a hidden name (one starting with `.`); otherwise that directory is unavailable, exactly like a missing entry (ADR 0012). That rejects the home directory itself and any ancestor of it, such as `/` or `/home`. On a default Omarchy install `XDG_DESKTOP_DIR` is the home directory itself, so `desktop` is unavailable there.

## Daemon

An optional `[daemon]` table declares one program of the Bundle's that runs **outside the sandbox**, with the user's full access, and keeps running after the application closes (ADR 0050). It is a permission, the largest one: the consent dialog says so, and the consent record keeps all three keys together, so a change to any of them asks again. Any other key is an error.

| Key | Type | Required | Rule |
|---|---|---|---|
| `executable` | string | yes | A path relative to the Bundle root, with no `..`. At validation, and again at every launch, it must be a regular file with the executable bit set and an x86_64 ELF, and **no symlink may be on its path**, not even one inside the Bundle: the application cannot choose what `oma` runs outside the sandbox. |
| `args` | array of strings | no | Its arguments. No control characters. |
| `socket` | string | yes | Where it listens, relative to the user's runtime directory (`$XDG_RUNTIME_DIR`), made of plain names with no `..`. The first component may not begin with `hypr`, `wayland`, `pipewire`, `pulse`, `bus`, `dbus`, `systemd`, `gnupg`, `sway`, `niri` or `ssh`, which are other things'. |

`odk oma run` makes sure the daemon is there before it starts the application: if something accepts connections on `socket` nothing is started, and otherwise one transient systemd user unit named `oma-<escaped Identifier>` is, with `socket` as a socket unit of its own and the allowlisted part of the session environment (`PATH`, `WAYLAND_DISPLAY`, `HYPRLAND_INSTANCE_SIGNATURE`, `DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`, `XDG_CURRENT_DESKTOP`, `XDG_SESSION_TYPE`, `LANG`). The application is given that one socket file in its private runtime directory, at the same relative path, and nothing else of the host's. A Bundle with a daemon cannot be run once (ADR 0018), since the daemon outlives the launch; it must be installed. And only the installed copy runs a daemon: `odk oma run` refuses any other copy of the Bundle, since the consent covers the program the user installed and a copy could hold another one.

## Example

```toml
manifest_version = 1
identifier = "com.example.notes"
name = "Notes"
version = "1.4.0"
description = "A small notes app"
executable = "bin/notes"
icon = "share/notes.svg"
categories = ["Utility", "TextEditor"]
publisher = "example.com"

[permissions]
network = false
gpu = true
audio = false
updates = true

[[permissions.filesystem]]
dir = "documents"
access = "write"

[[permissions.filesystem]]
dir = "downloads"
access = "read"

[permissions.dbus]
talk = ["org.freedesktop.Notifications"]

[daemon]
executable = "bin/notesd"
args = ["serve"]
socket = "notes/notesd.sock"
```

## Not covered here

What the sandbox exposes beyond these permissions is in `sandbox-baseline.md`, how the consent dialog presents them is in `consent-dialog.md`, and the Plugin manifest (ADR 0020) is in `plugin-manifest.md`.
