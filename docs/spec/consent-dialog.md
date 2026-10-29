# Consent dialog specification, version 1

Normative. This spec defines the dialog of ADR 0016, shown by the helper of ADR 0025, that asks the user to grant permissions. It must not change a decision recorded in an ADR; if it needs to, write a new ADR first. Where the two disagree, the ADR wins.

## When it is shown

`odk oma` shows the dialog whenever a Bundle requests a permission missing from its consent record (ADR 0023), and whenever a Bundle is signed under a publisher root that is not pinned for that app id (ADR 0060). Consent is the pair (root key id, app id). That covers:

- opening a Bundle from outside `~/Applications`, where it offers Install, Run once or Cancel (ADR 0018);
- the first launch of a Bundle already in `~/Applications` that has no consent record for that pair;
- launching an upgraded Bundle that requests anything new (ADR 0015);
- a new publisher root. A release key the pinned root has authorized, a higher root-signed `oma-keys`, a revocation and a rotation are not this case. A new root uses the first-install buttons, not the upgrade buttons.

It is not shown when everything a Bundle requests is already in its consent record and the file verifies under the pinned root. In non-interactive mode there is no dialog: a missing permission, or a root that is not the pin, makes `odk oma` exit non-zero and launch nothing. `odk oma install --accept-permissions` grants permissions without a dialog (ADR 0016). It does not accept a new root; pointed at one, it installs nothing.

A Cancel for a (root key id, app id) pair is stored. A later launch, `--register`, or another pass over the same file does not open the dialog again for that pair. `odk oma install` of the file can ask again.

## Layout

One screen per consent, in this order:

1. The publisher identity from the signed manifest (ADR 0060): the root fingerprint, the app id, the monotonic version, and the publisher host in Unicode and in punycode. The image's display name and icon are not read for this screen.
2. One line saying whether the application is new or an upgrade that adds permissions.
3. One line per requested permission, worded as in the table below.
4. The buttons.

For an upgrade, only the **new** permissions are listed. The ones already in the consent record are collapsed behind a single "already allowed" line, so a long list does not bury the change.

## Buttons

| Situation | Buttons |
|---|---|
| First open of a Bundle outside `~/Applications`, under a root already pinned for that app id | Install, Run once, Cancel |
| A publisher root that is not pinned for that app id (ADR 0060) | Install, Run once, Cancel |
| `odk oma install` of a Bundle file under a publisher root that is not pinned for that app id | Install, Cancel: the command installs or does nothing, as `docs/spec/oma-cli.md` says |
| Before that, when its host is confusable with a host pinned for a different root | Confirm, Cancel: the note alone, with nothing that installs. Install and Run once are offered only after Confirm, in their own dialog |
| An upgrade that verifies under the pinned root and requests something new, with a previous version kept | Allow, Keep previous version, Cancel |
| Any other installed Bundle that requests something new | Allow, Cancel |
| `odk oma install` of a Bundle that requests something new, under the pinned root | Install, Cancel |

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
| `[daemon]` | Runs its own program ({executable}) OUTSIDE the sandbox with full access to your account, and keeps it running after it closes (ADR 0050). Listed first, since it is the one that takes the sandbox away |

A named directory that is unavailable (ADR 0012) is listed as "{Desktop}: you have no such folder, so this will not be available".

The trust lines are signed fields, plus the URL that was fetched (ADR 0060). They are `notes` entries, not permission lines. The dialog shows the root fingerprint in groups of eight, the app id, the monotonic version, and the publisher host in Unicode and in punycode. It does not read the image for a name or an icon.

| Situation | Note |
|---|---|
| A new publisher root | This pins the publisher key {fingerprint} for {app id} {version}. The publisher host is {unicode host} ({punycode}). |
| The root is already pinned for another app id | This publisher key is already pinned for {other app id}. {app id} is another application under it. |
| The file was fetched | It was fetched from {url}. |
| The publisher host is confusable with a host already pinned for a different root | {unicode host} looks like {existing host}, which is already pinned for a different publisher. Confirm that this is a different publisher before installing. |
| `user.xdg.origin.url` or `user.xdg.referrer.url` is shown at all | Unverified download note: {value}. This is not the publisher. |

Install and Run once stay disabled while the confusable-host note is up, until the person confirms that note on its own. `user.xdg.origin.url` and `user.xdg.referrer.url` are never a decision. A missing attribute adds no note. The host of a URL `odk-oma` fetched must be the signed publisher; a mismatch does not reach this dialog. `odk oma install --accept-permissions` does not accept a new root and does not skip the confusable-host confirmation.

## Interface

`odk-oma` runs `oma-consent`, found next to its own executable, and talks to it through its standard streams. The helper never sees the Bundle (ADR 0025).

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

`lines` are the permission lines, already worded; `notes` are the unavailable-folder notes. `buttons` are the offered answers in the order shown: `install`, `run-once`, `allow`, `keep-previous`, `confirm` and `cancel`. For a signed file (ADR 0060), `name` and `identifier` are both the signed app id and `version` is the monotonic version: the image is not read for them. `lines` are the permissions in the image's `manifest.toml`, read with `odk-oma`'s own reader once the envelope has held (ADR 0067, which settles `docs/oma-trust-plan.md`'s question 12).

**Answer**, one line on standard output: exactly one of the offered words, and exit status 0. Closing the window or pressing Escape is `cancel`.

**`odk-oma` treats everything else as Cancel and fails closed**: the helper is missing or cannot start, there is no display, it exits non-zero, its output is not exactly one line, or the word is not one that was offered. This is the non-interactive rule of ADR 0016: with no usable dialog, a missing permission launches nothing, and `odk oma install --accept-permissions` is the only way to grant without one.

## Behaviour

- No button is the default: Enter grants nothing. Escape and closing the window are Cancel.
- The window is small and fixed in size, with the application id `oma-consent`. GPUI 0.2.2 cannot ask a Wayland compositor for a floating dialog, so under Hyprland the helper asks `hyprctl` to float, size and centre it, and ignores any failure; on another compositor it may be tiled.
- The helper follows the Omarchy theme by reading `colors.toml` itself, with fallback colours; it does not depend on ODK.
- **Not in the dialog yet:** the "upgrade" screen, with its collapsed "already allowed" line and the "Keep previous version" button, because upgrades are not built. The icon stays out of the dialog (ADR 0060): the identity lines are the signed fields, and the image is not mounted to fill them.

## Left out of v1

Remembering a yes that would suppress a later permission prompt, revoking a permission from a settings UI, and a permissions viewer. A denial of a new publisher root is remembered, and that pair is not asked again until `odk oma install` (ADR 0060). Revoking a permission means deleting the consent record, which prompts again on the next launch (ADR 0023). A management UI can come later.
