# Consent dialog specification, version 1

Normative. This spec defines the dialog of ADR 0016, shown by the helper of ADR 0025, that asks the user to grant permissions. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

## When it is shown

oma shows the dialog whenever a Bundle requests a permission missing from its consent record (ADR 0023), and whenever a signed Bundle is signed by a key that is not the one pinned for its Identifier (ADR 0055). That covers:

- opening a Bundle from outside `~/Applications`, where it offers Install, Run once or Cancel (ADR 0018);
- the first launch of a Bundle already in `~/Applications` that has no consent record;
- launching an upgraded Bundle that requests anything new (ADR 0015);
- a file signed by any key other than the pin, including a key the publisher's domain has newly listed and a key from a domain that was unreachable and came back with a different key. That case uses the first-install buttons, not the upgrade buttons.

It is not shown when everything a Bundle requests is already in its consent record and the file verifies under the pinned key. In non-interactive mode there is no dialog: a missing permission, or a key that is not the pin, makes oma exit non-zero and launch nothing. `odk oma install --accept-permissions` grants permissions without a dialog (ADR 0016). It does not accept a key other than the pin; pointed at one, it installs nothing.

## Layout

One screen per consent, in this order:

1. The application's name, icon and version, with its Identifier in small text.
2. One line saying whether the application is new or an upgrade that adds permissions.
3. One line per requested permission, worded as in the table below.
4. The buttons.

For an upgrade, only the **new** permissions are listed. The ones already in the consent record are collapsed behind a single "already allowed" line, so a long list does not bury the change.

## Buttons

| Situation | Buttons |
|---|---|
| First open of a Bundle outside `~/Applications` | Install, Run once, Cancel |
| A file signed by a key that is not the pin, including a new publisher after the domain was unreachable (ADR 0055) | Install, Run once, Cancel |
| An upgrade that verifies under the pinned key and requests something new, with a previous version kept | Allow, Keep previous version, Cancel |
| Any other installed Bundle that requests something new | Allow, Cancel |
| `odk oma install` of a Bundle that requests something new, under the pinned key or with no key pinned yet | Install, Cancel |

Keep previous version rolls back to the previous version (ADR 0011) and launches it, so declining an upgrade never leaves an application that will not start (ADR 0015). Cancel launches nothing.

Consent is all-or-nothing: the user grants everything the Bundle requests or cancels. There is no way to turn individual permissions off, because the Manifest cannot mark a permission optional and an application would have to cope with it missing. Optional permissions can be added later through an ADR.

## Permission lines

Each line states the consequence in plain language, not the permission's name.

| Permission | Line |
|---|---|
| `network = true` | Can use the network, including services running on this computer |
| `gpu = true` | Can use the graphics card directly |
| `audio = true` | Can play and record sound |
| `updates = true` | Can update itself; you'll be asked again if an update wants more access |
| filesystem `read` | Can read files in your {Documents} folder |
| filesystem `write` | Can read and change files in your {Documents} folder |
| `dbus` | Can talk to these desktop services: {names} |

A named directory that is unavailable (ADR 0012) is listed as "{Desktop}: you have no such folder, so this will not be available".

A trust note (ADR 0055) is a `notes` entry, not a permission line. At most one of the key notes applies, and the origin note is separate:

| Situation | Note |
|---|---|
| No signature, and no key pinned | This bundle is unsigned. |
| The domain could not be reached, so the key in the file is not confirmed | The publisher's domain could not be reached, so this signing key is not confirmed. |
| The file's key is not the pin, and the domain stayed reachable | This file is signed by a different key from the one pinned for this application. Installing it asks you to accept that key, as on a first install. |
| The domain was unreachable and now signs with a different key | This domain was unreachable and now signs with a different key. This is a new publisher, and it needs your consent as on a first install. |
| `user.xdg.origin.url` names a host other than the publisher | The download origin ({host}) is not the publisher's domain ({domain}). |

A missing origin attribute adds no note and never blocks. An origin whose host is the publisher adds no note. `odk oma install --accept-permissions` does not skip a different key.

## Interface

`oma` runs `oma-consent`, found next to its own executable, and talks to it through its standard streams. The helper never sees the Bundle (ADR 0025).

**Request**, TOML on standard input:

```toml
name = "Hello"                      # the Manifest's name
identifier = "com.example.hello"    # shown in small text
version = "0.1.0"
situation = "new"                   # "new", or "upgrade" (defined, not sent yet)
buttons = ["install", "run-once", "cancel"]
lines = ["Can use the graphics card directly"]
notes = ["Desktop: you have no such folder, so this will not be available"]
```

`lines` are the permission lines, already worded; `notes` are the unavailable-folder notes. `buttons` are the offered answers in the order shown: `install`, `run-once`, `allow`, `keep-previous` and `cancel`.

**Answer**, one line on standard output: exactly one of the offered words, and exit status 0. Closing the window or pressing Escape is `cancel`.

**`oma` treats everything else as Cancel and fails closed**: the helper is missing or cannot start, there is no display, it exits non-zero, its output is not exactly one line, or the word is not one that was offered. This is the non-interactive rule of ADR 0016: with no usable dialog, a missing permission launches nothing, and `odk oma install --accept-permissions` is the only way to grant without one.

## Behaviour

- No button is the default: Enter grants nothing. Escape and closing the window are Cancel.
- The window is small and fixed in size, with the application id `oma-consent`. GPUI 0.2.2 cannot ask a Wayland compositor for a floating dialog, so under Hyprland the helper asks `hyprctl` to float, size and centre it, and ignores any failure; on another compositor it may be tiled.
- The helper follows the Omarchy theme by reading `colors.toml` itself, with fallback colours; it does not depend on ODK.
- **Not in the dialog yet:** the application's icon, because showing it would have an unsandboxed process parse an untrusted Bundle's image; the "upgrade" screen, with its collapsed "already allowed" line and the "Keep previous version" button, because upgrades are not built.

## Left out of v1

Remembering "don't ask again", revoking a permission from a settings UI, and a permissions viewer. Revoking a permission means deleting the consent record, which prompts again on the next launch (ADR 0023). A management UI can come later.
